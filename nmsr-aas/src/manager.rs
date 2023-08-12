use nmsr_rendering::{
    high_level::pipeline::Backends,
    high_level::{
        camera::{Camera, CameraRotation, ProjectionParameters},
        pipeline::{GraphicsContext, GraphicsContextDescriptor, scene::SunInformation},
        types::PlayerBodyPartType,
    },
};

use strum::{Display, EnumCount, EnumIter, EnumString, IntoEnumIterator};
#[cfg(feature = "uv")]
use {
    crate::utils::errors::NMSRaaSError::MissingPartManager, std::borrow::Cow,
    std::collections::HashMap, std::path::Path, strum::IntoEnumIterator, tracing::instrument,
};

#[cfg(feature = "lazy_parts")]
use tracing::debug;

#[cfg(feature = "uv")]
use nmsr_lib::{
    parts::manager::PartsManager,
    vfs::{PhysicalFS, VfsPath},
};
use tracing::{info, instrument};
#[cfg(feature = "lazy_parts")]
use {
    crate::utils::errors::NMSRaaSError,
    rayon::prelude::*,
    std::io::{BufReader, BufWriter, Write},
};

use crate::utils::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, EnumString, EnumIter, EnumCount, Display)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum RenderMode {
    FullBody,
    FrontFull,
    #[cfg(feature = "wgpu")]
    Bust,
    FullBodyIso,
    Head,
    HeadIso,
    Face,
}

#[derive(Debug, Clone)]
#[cfg(feature = "uv")]
pub(crate) struct NMSRaaSManager {
    #[cfg(feature = "lazy_parts")]
    part_root: VfsPath,
    #[cfg(not(feature = "lazy_parts"))]
    part_managers: HashMap<RenderMode, PartsManager>,
}

#[cfg(feature = "wgpu")]
impl RenderMode {
    pub(crate) fn get_camera(&self) -> Camera {
        match self {
            RenderMode::FullBody => Camera::new_orbital(
                [0.2, 16.5, 0.5].into(),
                53.0,
                CameraRotation {
                    yaw: 24.28,
                    pitch: 11.83,
                },
                ProjectionParameters::Perspective { fov: 37.5 },
                1.0,
            ),
            _ => unimplemented!("wgpu rendering is not yet implemented"),
        }
    }
    
    pub(crate) fn get_lighting(&self) -> SunInformation {
        match self {
            RenderMode::FullBody | RenderMode::FullBodyIso => SunInformation::new([0.0, -1.0, 5.0].into(), 1.0, 0.25),
            _ => unimplemented!("wgpu rendering is not yet implemented"),
        }
    }
    
    pub(crate) fn get_arm_rotation(&self) -> f32 {
        match self {
            RenderMode::FullBody | RenderMode::FullBodyIso => 10.0,
            _ => 0.0
        }
    }
    

    #[instrument(level = "trace", skip(self))]
    pub(crate) fn get_body_parts(&self) -> Vec<PlayerBodyPartType> {
        match self {
            RenderMode::FullBody | RenderMode::FrontFull | RenderMode::FullBodyIso => {
                PlayerBodyPartType::iter().collect()
            }
            RenderMode::Head | RenderMode::HeadIso | RenderMode::Face => {
                vec![PlayerBodyPartType::Head, PlayerBodyPartType::HeadLayer]
            }
            RenderMode::Bust => {
                let excluded = vec![PlayerBodyPartType::LeftLeg, PlayerBodyPartType::RightLeg];
                
                PlayerBodyPartType::iter().filter(|m| excluded.contains(&m.get_non_layer_part())).collect()
            }
        }
    }
}

#[cfg(feature = "wgpu")]
#[derive(Debug)]
pub(crate) struct NMSRaaSManager {
    pub graphics_context: GraphicsContext,
}

#[cfg(feature = "wgpu")]
impl NMSRaaSManager {
    pub(crate) async fn new() -> Result<NMSRaaSManager> {
        // Setup an nmsr wgpu rendering pipeline.
        // Since we are not rendering to a surface (i.e. a window), we don't need to provide
        // a surface, nor a default size.
        let graphics_context = GraphicsContext::new(GraphicsContextDescriptor {
            backends: Some(Backends::all()),
            surface_provider: Box::new(|_| None),
            default_size: (0, 0),
            texture_format: Some(GraphicsContext::DEFAULT_TEXTURE_FORMAT),
        })
        .await?;

        info!(
            "Created graphics context with adapter {:?} and we're using {} MSAA samples.",
            &graphics_context.adapter.get_info(),
            graphics_context.sample_count
        );

        Ok(NMSRaaSManager { graphics_context })
    }
}

#[cfg(feature = "uv")]
impl NMSRaaSManager {
    #[instrument(level = "trace", skip(part_root))]
    async fn create_part_manager_for_mode(
        part_root: &VfsPath,
        render_type: &RenderMode,
    ) -> Result<PartsManager> {
        let path = part_root.join(render_type.to_string())?;

        Ok(PartsManager::new(&path)?)
    }
}

#[cfg(not(feature = "lazy_parts"))]
#[cfg(feature = "uv")]
impl NMSRaaSManager {
    pub(crate) fn get_manager(&self, render_type: &RenderMode) -> Result<Cow<PartsManager>> {
        self.part_managers
            .get(render_type)
            .map(Cow::Borrowed)
            .ok_or_else(|| MissingPartManager(render_type.clone()))
    }

    #[cfg(feature = "uv")]
    #[instrument(level = "trace", skip(part_root))]
    pub(crate) async fn new(part_root: impl AsRef<Path>) -> Result<NMSRaaSManager> {
        let part_root: VfsPath = PhysicalFS::new(part_root).into();
        let mut map = HashMap::with_capacity(RenderMode::COUNT);

        for render_type in RenderMode::iter() {
            let manager = Self::create_part_manager_for_mode(&part_root, &render_type)?;
            map.insert(render_type, manager);
        }

        Ok(NMSRaaSManager { part_managers: map })
    }
}

#[cfg(feature = "lazy_parts")]
impl NMSRaaSManager {
    pub(crate) fn get_manager(&self, render_type: &RenderMode) -> Result<Cow<PartsManager>> {
        let lazy_parts_dir = Self::get_lazy_parts_directory(&self.part_root)?;
        let part_path = Self::get_render_mode_part_manager_path(&lazy_parts_dir, render_type)?;

        if part_path.exists()? {
            let reader = BufReader::new(part_path.open_file()?);

            let start = std::time::Instant::now();
            let manager = bincode::deserialize_from(reader)?;
            debug!(
                "Deserialized part manager for {:?} in {:?}",
                render_type,
                start.elapsed()
            );

            Ok(Cow::Owned(manager))
        } else {
            Err(MissingPartManager(render_type.clone()))
        }
    }

    fn get_lazy_parts_directory(part_root: &VfsPath) -> Result<VfsPath> {
        Ok(part_root.join("lazy_parts")?)
    }

    fn get_render_mode_part_manager_path(
        lazy_parts_dir: &VfsPath,
        render_type: &RenderMode,
    ) -> Result<VfsPath> {
        Ok(lazy_parts_dir.join(render_type.to_string())?)
    }

    #[instrument(level = "trace", skip(part_root))]
    pub(crate) fn new(part_root: impl AsRef<Path>) -> Result<NMSRaaSManager> {
        let part_root = PhysicalFS::new(part_root).into();
        let lazy_parts_dir = Self::get_lazy_parts_directory(&part_root)?;

        // Yeet all the old parts we made just in case.
        // It's a one time action so it's fine™
        lazy_parts_dir.remove_dir_all()?;
        lazy_parts_dir.create_dir_all()?;

        let serialized_parts: Vec<_> = RenderMode::iter()
            .par_bridge()
            .map(|render_type| {
                let manager = Self::create_part_manager_for_mode(&part_root, &render_type);
                let serialized = manager.and_then(|manager| {
                    bincode::serialize(&manager).map_err(NMSRaaSError::BincodeError)
                });

                (render_type, serialized)
            })
            .collect();

        for (mode, serialized_part) in serialized_parts {
            let file = Self::get_render_mode_part_manager_path(&lazy_parts_dir, &mode)?;
            let mut writer = BufWriter::new(file.create_file()?);
            let data = serialized_part?;

            writer.write_all(data.as_slice())?;
        }

        Ok(NMSRaaSManager { part_root })
    }
}
