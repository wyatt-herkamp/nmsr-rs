use base64::{engine::general_purpose::STANDARD, Engine};
use glam::{Vec2, Vec3};
use nmsr_rendering::{
    high_level::{
        parts::{
            part::Part,
            uv::{CubeFaceUvs, FaceUv},
        },
        types::PlayerPartTextureType, utils::parts::primitive_convert,
    },
    low_level::primitives::mesh::PrimitiveDispatch,
};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_128;

use crate::generator::ModelGenerationProject;

#[derive(Debug, Copy, Clone, Serialize)]
pub struct ProjectMeta {
    format_version: &'static str,
    model_format: &'static str,
    box_uv: bool,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            format_version: "4.5",
            model_format: "free",
            box_uv: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct RawProjectElement(Value);

impl RawProjectElement {
    pub fn new_cube(
        name: String,
        box_uv: bool,
        from: Vec3,
        to: Vec3,
        origin: Vec3,
        rotation: Vec3,
        faces: RawProjectElementFaces,
    ) -> Self {
        Self(json!({
            "uuid": str_to_uuid(&name),
            "name": name,
            "box_uv": box_uv,
            "type": "cube",
            "from": from,
            "to": to,
            "origin": origin,
            "rotation": rotation,
            "faces": faces,
        }))
    }

    pub fn new_quad(
        name: String,
        part: Part,
        texture: PlayerPartTextureType,
        project: &ModelGenerationProject,
    ) -> Self {
        fn random_names(a: &str, b: &str) -> (String, String) {
            let (a_new, b_new) = Uuid::new_v4().as_u64_pair();

            (format!("{a}{a_new:x}"), format!("{b}{b_new:x}"))
        }

        let converted = primitive_convert(&part);

        let (top_left, top_right) = random_names("top_left", "top_right");
        let (bottom_left, bottom_right) = random_names("bottom_left", "bottom_right");

        let texture_id = project.get_texture_id(texture);

        let uv_size = texture.get_texture_size();
        let (uv_width, uv_height) = (uv_size.0 as f32, uv_size.1 as f32);
        
        let result = if let PrimitiveDispatch::Quad(quad) = converted {
            let uvs = FaceUv::from([
                (quad.top_left.uv.x * uv_width) as u16,
                (quad.top_left.uv.y * uv_height) as u16,
                (quad.top_right.uv.x * uv_width) as u16,
                (quad.top_right.uv.y * uv_height) as u16,
                (quad.bottom_left.uv.x * uv_width) as u16,
                (quad.bottom_left.uv.y * uv_height) as u16,
                (quad.bottom_right.uv.x * uv_width) as u16,
                (quad.bottom_right.uv.y * uv_height) as u16,
            ]);

            let uvs = project.handle_face(texture, uvs);

            let [top_left_uv, top_right_uv, bottom_right_uv, bottom_left_uv] = shrink_rectangle(
                [
                    [uvs.top_left.x, uvs.top_left.y],
                    [uvs.top_right.x, uvs.top_right.y],
                    [uvs.bottom_right.x, uvs.bottom_right.y],
                    [uvs.bottom_left.x, uvs.bottom_left.y],
                ],
                RawProjectElementFace::UV_OFFSET,
            );

            json!({
                "uuid": str_to_uuid(&name),
                "name": name,
                "box_uv": false,
                "type": "mesh",
                "origin": Vec3::ZERO,
                "rotation": Vec3::ZERO,
                "vertices": {
                    &top_left: [
                        quad.top_left.position.x,
                        quad.top_left.position.y,
                        quad.top_left.position.z,
                    ],
                    &top_right: [
                        quad.top_right.position.x,
                        quad.top_right.position.y,
                        quad.top_right.position.z,
                    ],
                    &bottom_right: [
                        quad.bottom_right.position.x,
                        quad.bottom_right.position.y,
                        quad.bottom_right.position.z,
                    ],
                    &bottom_left: [
                        quad.bottom_left.position.x,
                        quad.bottom_left.position.y,
                        quad.bottom_left.position.z,
                    ],
                },
                "faces": {
                    "face": {
                        "texture": texture_id,
                        "uv": {
                            &top_left: top_left_uv,
                            &top_right: top_right_uv,
                            &bottom_right: bottom_right_uv,
                            &bottom_left: bottom_left_uv,
                        },
                        "vertices": [
                            &top_left,
                            &top_right,
                            &bottom_right,
                            &bottom_left,
                        ]
                    }
                },
            })
        } else {
            unreachable!("Expected a quad primitive, got something else")
        };

        Self(result)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RawProjectElementFace {
    texture: Option<u32>,
    uv: [f32; 4],
}

impl RawProjectElementFace {
    pub const UV_OFFSET: f32 = 0.05;

    pub fn new(
        project: &ModelGenerationProject,
        texture: PlayerPartTextureType,
        uv: FaceUv,
    ) -> Self {
        let uv = project.handle_face(texture, uv);
        let texture_id = project.get_texture_id(texture);

        let [top_left_uv, _, bottom_right_uv, _] = shrink_rectangle(
            [
                [uv.top_left.x, uv.top_left.y],
                [uv.top_right.x, uv.top_right.y],
                [uv.bottom_right.x, uv.bottom_right.y],
                [uv.bottom_left.x, uv.bottom_left.y],
            ],
            RawProjectElementFace::UV_OFFSET,
        );

        let uv = [
            top_left_uv[0],
            top_left_uv[1],
            bottom_right_uv[0],
            bottom_right_uv[1],
        ];

        Self {
            texture: Some(texture_id),
            uv,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RawProjectElementFaces {
    north: RawProjectElementFace,
    south: RawProjectElementFace,
    east: RawProjectElementFace,
    west: RawProjectElementFace,
    up: RawProjectElementFace,
    down: RawProjectElementFace,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelFaceUv {
    pub top_left: Vec2,
    pub top_right: Vec2,
    pub bottom_right: Vec2,
    pub bottom_left: Vec2,
}

impl RawProjectElementFaces {
    pub fn new(
        project: &ModelGenerationProject,
        texture: PlayerPartTextureType,
        faces: CubeFaceUvs,
    ) -> Self {
        Self {
            north: RawProjectElementFace::new(project, texture, faces.north),
            south: RawProjectElementFace::new(project, texture, faces.south),
            east: RawProjectElementFace::new(project, texture, faces.east),
            west: RawProjectElementFace::new(project, texture, faces.west),
            up: RawProjectElementFace::new(
                project,
                texture,
                faces.up.flip_horizontally().flip_vertically(),
            ),
            down: RawProjectElementFace::new(project, texture, faces.down.flip_horizontally()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RawProject {
    meta: ProjectMeta,
    resolution: ProjectTextureResolution,
    elements: Vec<RawProjectElement>,
    textures: Vec<RawProjectTexture>,
    outliner: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectTextureResolution {
    width: f32,
    height: f32,
}

impl ProjectTextureResolution {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl RawProject {
    pub fn new(
        resolution: ProjectTextureResolution,
        elements: Vec<RawProjectElement>,
        textures: Vec<RawProjectTexture>,
        outliner: Vec<Value>,
    ) -> Self {
        Self {
            meta: ProjectMeta::default(),
            elements,
            textures,
            resolution,
            outliner
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RawProjectTexture {
    name: String,
    id: u32,
    path: &'static str,
    mode: &'static str,
    visible: bool,
    saved: bool,
    uuid: Uuid,
    source: String,
}

impl RawProjectTexture {
    pub fn new(name: String, id: u32, source: &[u8]) -> Self {
        Self {
            path: "",
            uuid: str_to_uuid(&name),
            name,
            id,
            mode: "bitmap",
            visible: true,
            saved: false,
            source: format!("data:image/png;base64,{}", STANDARD.encode(source)),
        }
    }
}

pub(crate) fn str_to_uuid(s: &str) -> Uuid {
    let mut bytes = xxh3_128(s.as_bytes()).to_be_bytes();
    // Set the version to 4 (random)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;

    Uuid::from_bytes(bytes)
}

pub fn shrink_rectangle(points: [[f32; 2]; 4], factor: f32) -> [[f32; 2]; 4] {
    let center = [
        (points[0][0] + points[1][0] + points[2][0] + points[3][0]) / 4.,
        (points[0][1] + points[1][1] + points[2][1] + points[3][1]) / 4.,
    ];

    fn distance_to(a: [f32; 2], other: [f32; 2]) -> f32 {
        ((a[0] - other[0]).powi(2) + (a[1] - other[1]).powi(2)).sqrt()
    }

    let mut new_points = [[0.0; 2]; 4];
    for (i, point) in points.iter().enumerate() {
        let distance_to_center = distance_to(*point, center);
        let new_distance_to_center = distance_to_center - factor;

        let new_point = [
            center[0] + (point[0] - center[0]) * new_distance_to_center / distance_to_center,
            center[1] + (point[1] - center[1]) * new_distance_to_center / distance_to_center,
        ];

        new_points[i] = new_point;
    }

    new_points
}
