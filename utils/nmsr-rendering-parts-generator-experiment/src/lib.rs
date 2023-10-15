use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Ok, Result};
use image::{GenericImage, ImageBuffer, Rgba, RgbaImage};
use itertools::Itertools;
use nmsr_rendering::high_level::{
    camera::Camera,
    model::PlayerModel,
    parts::provider::PlayerPartProviderContext,
    pipeline::{
        scene::{Scene, Size, SunInformation},
        Backends, BlendState, Features, GraphicsContext, GraphicsContextDescriptor, SceneContext,
        SceneContextWrapper, ShaderSource,
    },
    types::{PlayerBodyPartType, PlayerPartTextureType},
};

pub use nmsr_rendering;

pub enum PartsGroupLogic {
    SplitArmsFromBody,
    MergeArmsWithBody,
    MergeEverything,
}

struct PartGroupSpec {
    pub(crate) parts: Vec<PlayerBodyPartType>,
    pub(crate) toggle_slim: bool,
    name: &'static str,
}

impl PartGroupSpec {
    fn new(parts: Vec<PlayerBodyPartType>, toggle_slim: bool, name: &'static str) -> Self {
        Self {
            parts,
            toggle_slim,
            name,
        }
    }
}

impl PartsGroupLogic {
    pub(crate) fn get_groups(&self) -> Vec<PartGroupSpec> {
        match self {
            PartsGroupLogic::SplitArmsFromBody => {
                vec![
                    PartGroupSpec::new(
                        vec![
                            PlayerBodyPartType::Head,
                            PlayerBodyPartType::Body,
                            PlayerBodyPartType::LeftLeg,
                            PlayerBodyPartType::RightLeg,
                        ],
                        /* toggle slim */ false,
                        /* name */ "Body.qoi",
                    ),
                    PartGroupSpec::new(
                        vec![
                            PlayerBodyPartType::HeadLayer,
                            PlayerBodyPartType::BodyLayer,
                            PlayerBodyPartType::LeftLegLayer,
                            PlayerBodyPartType::RightLegLayer,
                        ],
                        /* toggle slim */ false,
                        /* name */ "Body Layer.qoi",
                    ),
                    PartGroupSpec::new(
                        vec![PlayerBodyPartType::LeftArm, PlayerBodyPartType::RightArm],
                        /* toggle slim */ true,
                        /* name */ "{model}/Arms.qoi",
                    ),
                    PartGroupSpec::new(
                        vec![
                            PlayerBodyPartType::LeftArmLayer,
                            PlayerBodyPartType::RightArmLayer,
                        ],
                        /* toggle slim */ true,
                        /* name */ "{model}/Arms Layer.qoi",
                    ),
                ]
            }
            PartsGroupLogic::MergeArmsWithBody => {
                vec![
                    PartGroupSpec::new(
                        vec![
                            PlayerBodyPartType::Head,
                            PlayerBodyPartType::Body,
                            PlayerBodyPartType::LeftLeg,
                            PlayerBodyPartType::RightLeg,
                            PlayerBodyPartType::LeftArm,
                            PlayerBodyPartType::RightArm,
                        ],
                        /* toggle slim */ true,
                        /* name */ "{model}/Body.qoi",
                    ),
                    PartGroupSpec::new(
                        vec![
                            PlayerBodyPartType::HeadLayer,
                            PlayerBodyPartType::BodyLayer,
                            PlayerBodyPartType::LeftLegLayer,
                            PlayerBodyPartType::RightLegLayer,
                            PlayerBodyPartType::LeftArmLayer,
                            PlayerBodyPartType::RightArmLayer,
                        ],
                        /* toggle slim */ true,
                        /* name */ "{model}/Body Layer.qoi",
                    ),
                ]
            },
            PartsGroupLogic::MergeEverything => vec![
                PartGroupSpec::new(
                    vec![
                        PlayerBodyPartType::Head,
                        PlayerBodyPartType::Body,
                        PlayerBodyPartType::LeftLeg,
                        PlayerBodyPartType::RightLeg,
                        PlayerBodyPartType::LeftArm,
                        PlayerBodyPartType::RightArm,
                    ],
                    /* toggle slim */ true,
                    /* name */ "{model}/Body.qoi",
                ),
                PartGroupSpec::new(
                    vec![
                        PlayerBodyPartType::Head,
                        PlayerBodyPartType::Body,
                        PlayerBodyPartType::LeftLeg,
                        PlayerBodyPartType::RightLeg,
                        PlayerBodyPartType::LeftArm,
                        PlayerBodyPartType::RightArm,
                        PlayerBodyPartType::HeadLayer,
                        PlayerBodyPartType::BodyLayer,
                        PlayerBodyPartType::LeftLegLayer,
                        PlayerBodyPartType::RightLegLayer,
                        PlayerBodyPartType::LeftArmLayer,
                        PlayerBodyPartType::RightArmLayer,
                    ],
                    /* toggle slim */ true,
                    /* name */ "{model}/Body Layer.qoi",
                ),
            ],
        }
    }
}

pub async fn generate_parts(
    camera: Camera,
    sun: SunInformation,
    viewport_size: Size,
    parts_group_logic: PartsGroupLogic,
    shadow_y_pos: Option<f32>,
    root: PathBuf,
) -> Result<()> {
    fs::create_dir_all(&root)?;

    let groups = parts_group_logic.get_groups();

    for PartGroupSpec { parts, toggle_slim, name } in groups {
        process_group(parts, toggle_slim, camera, sun, viewport_size, name, &root).await?;
    }

    let mut env_shadow = Vec::with_capacity(1);
    process_group_logic(
        vec![PlayerBodyPartType::Head],
        false,
        false,
        &mut env_shadow,
        camera,
        sun,
        viewport_size,
        shadow_y_pos.or(Some(0.0)),
    )
    .await?;

    if let Some(PartRenderOutput { image }) = env_shadow.first() {
        save(image, "renders/environment_background.qoi")?;
    }

    Ok(())
}

async fn save_group(
    to_process: Vec<PartRenderOutput>,
    viewport_size: Size,
    name: String,
    renders_path: &Path,
) -> Result<()> {
    let processed = process_render_outputs(to_process);

    let layer_count = processed
        .values()
        .max_by_key(|layers| layers.len())
        .map(|layers| layers.len())
        .unwrap_or_default();

    println!("Saving group {} with {} layers", name, layer_count);

    let mut layers: HashMap<usize, _> = HashMap::new();

    for (point, pixels) in processed {
        for (index, pixel) in pixels.iter().enumerate() {
            let img = layers
                .entry(index)
                .or_insert_with(|| RgbaImage::new(viewport_size.width, viewport_size.height));

            unsafe {
                img.unsafe_put_pixel(point.x, point.y, *pixel);
            }
        }
    }

    for (index, img) in &layers {
        let mut file = renders_path.join::<PathBuf>(name.clone().into());
        if layer_count > 1 {
            file = file
                .with_file_name(format!(
                    "{}-{}",
                    file.file_stem().unwrap().to_str().unwrap(),
                    index
                ))
                .with_extension("qoi");
        }

        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }

        save(img, file)?;
    }

    Ok(())
}

async fn process_group(
    parts: Vec<PlayerBodyPartType>,
    toggle_slim: bool,
    camera: Camera,
    sun: SunInformation,
    viewport_size: Size,
    name: &'static str,
    renders_path: &Path,
) -> Result<()> {
    let toggle_backface = parts.iter().any(|p| p.is_hat_layer() || p.is_layer());

    let backface = if toggle_backface {
        vec![false, true]
    } else {
        vec![false]
    };

    let slim = if toggle_slim {
        vec![false, true]
    } else {
        vec![false]
    };

    for slim in slim {
        let mut result = Vec::new();

        for is_back_face in &backface {
            println!(
                "Processing group with parts {:?} (slim: {}, backface: {})",
                &parts, slim, is_back_face
            );

            if toggle_backface {
                for part in &parts {
                    if *is_back_face && !(part.is_layer() || part.is_hat_layer()) {
                        continue;
                    }
                    
                    process_group_logic(
                        vec![*part],
                        slim,
                        *is_back_face,
                        &mut result,
                        camera,
                        sun,
                        viewport_size,
                        None,
                    )
                    .await?;
                }
            } else {
                process_group_logic(
                    parts.clone(),
                    slim,
                    *is_back_face,
                    &mut result,
                    camera,
                    sun,
                    viewport_size,
                    None,
                )
                .await?;
            }
        }

        let model_name = if slim { "Alex" } else { "Steve" };
        save_group(
            result,
            viewport_size,
            name.replace("{model}", model_name),
            &renders_path,
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_group_logic(
    parts: Vec<PlayerBodyPartType>,
    slim: bool,
    back_face: bool,
    to_process: &mut Vec<PartRenderOutput>,
    camera: Camera,
    sun: SunInformation,
    viewport_size: Size,
    shadow_y_pos: Option<f32>,
) -> Result<()> {
    println!("  // Processing group logic with parts {:?} (slim: {}, backface: {})", &parts, slim, back_face);
    
    let part_provider: PlayerPartProviderContext<()> = PlayerPartProviderContext {
        model: if slim {
            PlayerModel::Alex
        } else {
            PlayerModel::Steve
        },
        has_hat_layer: parts.iter().any(|p| p.is_hat_layer()),
        has_layers: parts.iter().any(|p| p.is_layer()),
        has_cape: false,
        arm_rotation: 10.0,
        shadow_y_pos,
        shadow_is_square: false,
        armor_slots: None,
        #[cfg(feature = "ears")]
        ears_features: None,
    };

    let mut shader: String = include_str!("nmsr-new-uvmap-shader.wgsl").into();
    if back_face {
        shader = shader.replace("//backingface:", "")
    } else {
        shader = shader.replace("//frontface:", "")
    }

    let descriptor = GraphicsContextDescriptor {
        backends: Some(Backends::all()),
        surface_provider: Box::new(|_| None),
        default_size: (0, 0),
        texture_format: None,
        features: Features::empty(),
        blend_state: Some(BlendState::REPLACE),
        sample_count: Some(1),
        use_smaa: Some(false),
    };

    let graphics_context = if shadow_y_pos.is_none() {
        GraphicsContext::new_with_shader(descriptor, ShaderSource::Wgsl(shader.into())).await?
    } else {
        GraphicsContext::new(descriptor).await?
    };

    let scene_context = SceneContext::new(&graphics_context);

    let mut scene: Scene<SceneContextWrapper> = Scene::new(
        &graphics_context,
        scene_context.into(),
        camera,
        sun,
        viewport_size,
        &part_provider,
        &[],
    );

    scene.set_texture(
        &graphics_context,
        PlayerPartTextureType::Skin,
        &RgbaImage::new(64, 64),
    );

    scene.rebuild_parts(&part_provider, parts);

    scene.render(&graphics_context)?;

    let render = scene.copy_output_texture(&graphics_context, false).await?;

    let render_image: RgbaImage =
        ImageBuffer::from_raw(viewport_size.width, viewport_size.height, render)
            .ok_or(anyhow!("Unable to convert render to image"))?;

    to_process.push(PartRenderOutput {
        image: render_image,
    });

    Ok(())
}

fn process_render_outputs(to_process: Vec<PartRenderOutput>) -> HashMap<Point, Vec<Rgba<u8>>> {
    let pixels: HashMap<_, Vec<_>> = to_process
        .into_iter()
        .flat_map(|PartRenderOutput { image }| {
            image
                .enumerate_pixels()
                .map(move |(x, y, pixel)| (x, y, *pixel))
                .filter(|(_, _, pixel)| pixel[3] != 0)
                .collect::<Vec<_>>()
        })
        .sorted_by_cached_key(|(x, y, _)| (*x, *y))
        .group_by(|(x, y, _)| (*x, *y))
        .into_iter()
        .flat_map(|(_, group)| {
            group
                .map(|(x, y, pixel)| (Point::from((x, y)), pixel))
                .sorted_by_key(|(_, pixel)| (get_depth(pixel) as i32))
                .collect::<Vec<_>>()
        })
        .into_group_map();

    pixels
}

fn get_depth(pixel: &Rgba<u8>) -> u16 {
    let r = pixel[0] as u32;
    let g = pixel[1] as u32;
    let b = pixel[2] as u32;
    let a = pixel[3] as u32;

    let rgba: u32 = r | (g << 8) | (b << 16) | (a << 24);
    // Our Blue channel is composed of the 4 remaining bits of the shading + 4 bits from the depth
    // 1   2   3   4   5   6   7   8
    // [  -- s --  ]   [  -- d --  ]
    // Our Alpha channel is composed of the 8 remaining bits of the depth
    // 1   2   3   4   5   6   7   8
    // [          -- d --          ]
    ((rgba >> 20) & 0x1FFF) as u16
}

fn save<P: AsRef<Path>>(img: &RgbaImage, name: P) -> Result<()> {
    let encoded = qoi::encode_to_vec(&img.as_raw(), img.width(), img.height())?;
    fs::write(name, encoded)?;

    Ok(())
}

struct PartRenderOutput {
    image: RgbaImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Point {
    x: u32,
    y: u32,
}

impl From<(u32, u32)> for Point {
    fn from(value: (u32, u32)) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}
