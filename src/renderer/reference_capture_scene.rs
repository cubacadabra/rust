use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReferenceScene {
    pub(super) format_version: u32,
    pub(super) kind: String,
    pub(super) summary: SourceSummary,
    pub(super) geometry: Vec<GeometryInstance>,
    pub(super) cameras: Vec<PathRecord>,
    pub(super) lights: Vec<PathRecord>,
    pub(super) textures: Vec<PathRecord>,
    pub(super) texts: Vec<PathRecord>,
    pub(super) project_lighting: Option<ProjectLighting>,
    pub(super) terrain: Option<TerrainSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSummary {
    pub(super) instance_count: usize,
    pub(super) geometry_count: usize,
    pub(super) visible_geometry_count: usize,
    pub(super) camera_count: usize,
    pub(super) light_count: usize,
    pub(super) texture_count: usize,
    pub(super) text_count: usize,
    pub(super) spawn_count: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct PathRecord {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GeometryInstance {
    pub(super) path: String,
    pub(super) class: String,
    pub(super) transform: Transform,
    pub(super) size: [f32; 3],
    pub(super) color: [f32; 3],
    pub(super) material: Material,
    pub(super) transparency: f32,
    pub(super) cast_shadow: bool,
    pub(super) mesh: Option<MeshReference>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Transform {
    pub(super) position: [f32; 3],
    pub(super) rotation: [[f32; 3]; 3],
}

#[derive(Debug, Deserialize)]
pub(super) struct Material {
    pub(super) value: u32,
    pub(super) name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MeshReference {
    pub(super) kind: String,
    #[serde(rename = "meshId")]
    pub(super) mesh_id: Option<String>,
    #[serde(rename = "meshType")]
    pub(super) mesh_type: Option<u32>,
    pub(super) scale: Option<[f32; 3]>,
    pub(super) offset: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectLighting {
    pub(super) properties: BTreeMap<String, Value>,
    pub(super) effects: Vec<ProjectEffect>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectEffect {
    pub(super) class: String,
    pub(super) properties: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerrainSource {
    pub(super) requires_voxel_decoder: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ReferenceVertex {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) color: [f32; 4],
}

impl ReferenceVertex {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 24,
                shader_location: 2,
            },
        ],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ReferenceGlobals {
    pub(super) view_projection: [[f32; 4]; 4],
    pub(super) sun_direction_brightness: [f32; 4],
    pub(super) ambient: [f32; 4],
    pub(super) color_correction: [f32; 4],
}

#[derive(Clone, Copy)]
pub(super) struct SceneBounds {
    pub(super) minimum: Vec3,
    pub(super) maximum: Vec3,
}

impl SceneBounds {
    pub(super) fn empty() -> Self {
        Self {
            minimum: Vec3::splat(f32::INFINITY),
            maximum: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub(super) fn include(&mut self, point: Vec3) {
        self.minimum = self.minimum.min(point);
        self.maximum = self.maximum.max(point);
    }

    pub(super) fn center(self) -> Vec3 {
        (self.minimum + self.maximum) * 0.5
    }

    pub(super) fn size(self) -> Vec3 {
        self.maximum - self.minimum
    }
}

#[derive(Clone, Copy)]
pub(super) struct LightingValues {
    pub(super) brightness: f32,
    pub(super) ambient: Vec3,
    pub(super) outdoor_ambient: Vec3,
    pub(super) correction_brightness: f32,
    pub(super) correction_contrast: f32,
    pub(super) correction_saturation: f32,
}
