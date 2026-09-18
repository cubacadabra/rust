//! Development-only rendering of the tool-owned Roblox reference-scene artifact.
//!
//! This intentionally favors getting source-authored composition on screen over
//! broad Roblox compatibility. Unsupported data is counted in the report.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

const REFERENCE_SCENE_KIND: &str = "roblox-static-reference-scene";
const REFERENCE_SCENE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct RobloxReferenceCaptureConfig {
    pub scene_path: PathBuf,
    pub output_path: PathBuf,
    pub report_path: PathBuf,
    pub path_prefix: Option<String>,
    pub width: u32,
    pub height: u32,
    pub antialias: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxReferenceCaptureReport {
    pub schema_version: u32,
    pub scene_format_version: u32,
    pub source_scene: String,
    pub output_image: String,
    pub path_prefix: Option<String>,
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub adapter: String,
    pub backend: String,
    pub source_counts: BTreeMap<String, usize>,
    pub rendered_counts: BTreeMap<String, usize>,
    pub rendered_classes: BTreeMap<String, usize>,
    pub rendered_materials: BTreeMap<String, usize>,
    pub source_mesh_references: BTreeMap<String, usize>,
    pub approximated_as_oriented_boxes: BTreeMap<String, usize>,
    pub camera: CaptureCameraReport,
    pub lighting: CaptureLightingReport,
    pub unsupported: Vec<UnsupportedFeature>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCameraReport {
    pub source: &'static str,
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub vertical_field_of_view_degrees: f32,
    pub bounds_minimum: [f32; 3],
    pub bounds_maximum: [f32; 3],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLightingReport {
    pub source: &'static str,
    pub brightness: f32,
    pub ambient: [f32; 3],
    pub outdoor_ambient: [f32; 3],
    pub color_correction_brightness: f32,
    pub color_correction_contrast: f32,
    pub color_correction_saturation: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedFeature {
    pub feature: &'static str,
    pub count: usize,
    pub treatment: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceScene {
    format_version: u32,
    kind: String,
    summary: SourceSummary,
    geometry: Vec<GeometryInstance>,
    cameras: Vec<PathRecord>,
    lights: Vec<PathRecord>,
    textures: Vec<PathRecord>,
    texts: Vec<PathRecord>,
    project_lighting: Option<ProjectLighting>,
    terrain: Option<TerrainSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceSummary {
    instance_count: usize,
    geometry_count: usize,
    visible_geometry_count: usize,
    camera_count: usize,
    light_count: usize,
    texture_count: usize,
    text_count: usize,
    spawn_count: usize,
}

#[derive(Debug, Deserialize)]
struct PathRecord {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometryInstance {
    path: String,
    class: String,
    transform: Transform,
    size: [f32; 3],
    color: [f32; 3],
    material: Material,
    transparency: f32,
    cast_shadow: bool,
    mesh: Option<MeshReference>,
}

#[derive(Debug, Deserialize)]
struct Transform {
    position: [f32; 3],
    rotation: [[f32; 3]; 3],
}

#[derive(Debug, Deserialize)]
struct Material {
    value: u32,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MeshReference {
    kind: String,
    #[serde(rename = "meshId")]
    mesh_id: Option<String>,
    #[serde(rename = "meshType")]
    mesh_type: Option<u32>,
    scale: Option<[f32; 3]>,
    offset: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize)]
struct ProjectLighting {
    properties: BTreeMap<String, Value>,
    effects: Vec<ProjectEffect>,
}

#[derive(Debug, Deserialize)]
struct ProjectEffect {
    class: String,
    properties: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainSource {
    requires_voxel_decoder: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ReferenceVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

impl ReferenceVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
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
struct ReferenceGlobals {
    view_projection: [[f32; 4]; 4],
    sun_direction_brightness: [f32; 4],
    ambient: [f32; 4],
    color_correction: [f32; 4],
}

#[derive(Clone, Copy)]
struct SceneBounds {
    minimum: Vec3,
    maximum: Vec3,
}

impl SceneBounds {
    fn empty() -> Self {
        Self {
            minimum: Vec3::splat(f32::INFINITY),
            maximum: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    fn include(&mut self, point: Vec3) {
        self.minimum = self.minimum.min(point);
        self.maximum = self.maximum.max(point);
    }

    fn center(self) -> Vec3 {
        (self.minimum + self.maximum) * 0.5
    }

    fn size(self) -> Vec3 {
        self.maximum - self.minimum
    }
}

#[derive(Clone, Copy)]
struct LightingValues {
    brightness: f32,
    ambient: Vec3,
    outdoor_ambient: Vec3,
    correction_brightness: f32,
    correction_contrast: f32,
    correction_saturation: f32,
}

pub fn capture_roblox_reference(
    config: &RobloxReferenceCaptureConfig,
) -> Result<RobloxReferenceCaptureReport, String> {
    if config.width == 0 || config.height == 0 {
        return Err("capture dimensions must be greater than zero".to_owned());
    }
    let source = fs::read(&config.scene_path)
        .map_err(|error| format!("read {}: {error}", config.scene_path.display()))?;
    let scene: ReferenceScene = serde_json::from_slice(&source)
        .map_err(|error| format!("parse {}: {error}", config.scene_path.display()))?;
    validate_scene(&scene)?;

    let selected: Vec<&GeometryInstance> = scene
        .geometry
        .iter()
        .filter(|geometry| path_matches(&geometry.path, config.path_prefix.as_deref()))
        .filter(|geometry| {
            geometry.transparency < 0.99
                && geometry
                    .size
                    .iter()
                    .all(|value| value.is_finite() && *value > 0.0)
        })
        .collect();
    if selected.is_empty() {
        return Err(match config.path_prefix.as_deref() {
            Some(prefix) => format!("no visible geometry matched path prefix {prefix:?}"),
            None => "reference scene contains no visible geometry".to_owned(),
        });
    }

    let mut vertices = Vec::with_capacity(selected.len() * 36);
    let mut bounds = SceneBounds::empty();
    let mut rendered_classes = BTreeMap::new();
    let mut rendered_materials = BTreeMap::new();
    let mut source_mesh_references = BTreeMap::new();
    let mut approximated_as_oriented_boxes = BTreeMap::new();
    let mut external_meshes = 0;
    let mut source_shadow_casters = 0;
    for geometry in &selected {
        add_geometry(&mut vertices, &mut bounds, geometry);
        *rendered_classes.entry(geometry.class.clone()).or_insert(0) += 1;
        *rendered_materials
            .entry(
                geometry
                    .material
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Material({})", geometry.material.value)),
            )
            .or_insert(0) += 1;
        if geometry.class == "MeshPart" {
            *approximated_as_oriented_boxes
                .entry(geometry.class.clone())
                .or_insert(0) += 1;
        }
        if let Some(mesh) = &geometry.mesh {
            *source_mesh_references.entry(mesh.kind.clone()).or_insert(0) += 1;
        }
        if geometry
            .mesh
            .as_ref()
            .is_some_and(|mesh| mesh.mesh_id.is_some())
        {
            external_meshes += 1;
        }
        if geometry.cast_shadow {
            source_shadow_casters += 1;
        }
    }

    let lighting = lighting_values(scene.project_lighting.as_ref());
    let camera = fit_camera(bounds, config.width as f32 / config.height as f32);
    let render_result = render(
        &vertices,
        config.width,
        config.height,
        config.antialias,
        camera,
        lighting,
    )?;
    ensure_parent(&config.output_path)?;
    write_png(
        &config.output_path,
        config.width,
        config.height,
        &render_result.pixels,
    )?;

    let scoped_textures = count_paths(&scene.textures, config.path_prefix.as_deref());
    let scoped_lights = count_paths(&scene.lights, config.path_prefix.as_deref());
    let scoped_texts = count_paths(&scene.texts, config.path_prefix.as_deref());
    let mut source_counts = BTreeMap::new();
    source_counts.insert("instances".to_owned(), scene.summary.instance_count);
    source_counts.insert("geometry".to_owned(), scene.summary.geometry_count);
    source_counts.insert(
        "visibleGeometry".to_owned(),
        scene.summary.visible_geometry_count,
    );
    source_counts.insert("cameras".to_owned(), scene.summary.camera_count);
    source_counts.insert("lights".to_owned(), scene.summary.light_count);
    source_counts.insert("textures".to_owned(), scene.summary.texture_count);
    source_counts.insert("texts".to_owned(), scene.summary.text_count);
    source_counts.insert("spawns".to_owned(), scene.summary.spawn_count);

    let mut rendered_counts = BTreeMap::new();
    rendered_counts.insert("geometry".to_owned(), selected.len());
    rendered_counts.insert("vertices".to_owned(), vertices.len());
    rendered_counts.insert("triangles".to_owned(), vertices.len() / 3);

    let mut unsupported = vec![
        UnsupportedFeature {
            feature: "external mesh geometry",
            count: external_meshes,
            treatment: "rendered as the owning BasePart primitive or oriented size bounds",
        },
        UnsupportedFeature {
            feature: "mesh geometry and non-scale modifiers",
            count: source_mesh_references.values().sum(),
            treatment: "built-in wedge scale/offset is applied; external vertices, textures, and other mesh behavior are omitted",
        },
        UnsupportedFeature {
            feature: "Roblox material surface appearance",
            count: selected.len(),
            treatment: "material identities are counted, but surfaces use source color with one shared diffuse model",
        },
        UnsupportedFeature {
            feature: "surface textures and decals",
            count: scoped_textures,
            treatment: "omitted; source BasePart color is retained",
        },
        UnsupportedFeature {
            feature: "local lights",
            count: scoped_lights,
            treatment: "omitted; project ambient and directional light are used",
        },
        UnsupportedFeature {
            feature: "text and GUI signs",
            count: scoped_texts,
            treatment: "omitted",
        },
        UnsupportedFeature {
            feature: "source shadow casters",
            count: source_shadow_casters,
            treatment: "lit without cast shadows",
        },
        UnsupportedFeature {
            feature: "skybox and clouds",
            count: 1,
            treatment: "replaced by a flat cyan clear color",
        },
        UnsupportedFeature {
            feature: "imported cameras",
            count: scene.cameras.len(),
            treatment: "ignored because Maze World only exposes embedded thumbnail cameras; a deterministic fit camera is used",
        },
    ];
    if scene
        .terrain
        .as_ref()
        .is_some_and(|terrain| terrain.requires_voxel_decoder)
    {
        unsupported.push(UnsupportedFeature {
            feature: "Roblox SmoothGrid terrain",
            count: 1,
            treatment: "omitted; the imported payload still requires voxel decoding",
        });
    }
    let enabled_sun_rays = enabled_effect_count(&scene, "SunRaysEffect");
    if enabled_sun_rays > 0 {
        unsupported.push(UnsupportedFeature {
            feature: "SunRaysEffect",
            count: enabled_sun_rays,
            treatment: "omitted",
        });
    }
    let enabled_bloom = enabled_effect_count(&scene, "BloomEffect");
    if enabled_bloom > 0 {
        unsupported.push(UnsupportedFeature {
            feature: "BloomEffect",
            count: enabled_bloom,
            treatment: "omitted",
        });
    }

    let report = RobloxReferenceCaptureReport {
        schema_version: 1,
        scene_format_version: scene.format_version,
        source_scene: config.scene_path.display().to_string(),
        output_image: config.output_path.display().to_string(),
        path_prefix: config.path_prefix.clone(),
        width: config.width,
        height: config.height,
        sample_count: render_result.sample_count,
        adapter: render_result.adapter.name,
        backend: format!("{:?}", render_result.adapter.backend),
        source_counts,
        rendered_counts,
        rendered_classes,
        rendered_materials,
        source_mesh_references,
        approximated_as_oriented_boxes,
        camera: CaptureCameraReport {
            source: "deterministic bounds fit",
            position: camera.position.to_array(),
            target: camera.target.to_array(),
            vertical_field_of_view_degrees: camera.field_of_view_degrees,
            bounds_minimum: bounds.minimum.to_array(),
            bounds_maximum: bounds.maximum.to_array(),
        },
        lighting: CaptureLightingReport {
            source: if scene.project_lighting.is_some() {
                "project Lighting properties"
            } else {
                "capture defaults"
            },
            brightness: lighting.brightness,
            ambient: lighting.ambient.to_array(),
            outdoor_ambient: lighting.outdoor_ambient.to_array(),
            color_correction_brightness: lighting.correction_brightness,
            color_correction_contrast: lighting.correction_contrast,
            color_correction_saturation: lighting.correction_saturation,
        },
        unsupported,
    };
    ensure_parent(&config.report_path)?;
    let mut report_json = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("encode capture report: {error}"))?;
    report_json.push(b'\n');
    fs::write(&config.report_path, report_json)
        .map_err(|error| format!("write {}: {error}", config.report_path.display()))?;
    Ok(report)
}

fn validate_scene(scene: &ReferenceScene) -> Result<(), String> {
    if scene.kind != REFERENCE_SCENE_KIND {
        return Err(format!(
            "unsupported scene kind {:?}; expected {REFERENCE_SCENE_KIND:?}",
            scene.kind
        ));
    }
    if scene.format_version != REFERENCE_SCENE_VERSION {
        return Err(format!(
            "unsupported reference scene format version {}; expected {REFERENCE_SCENE_VERSION}",
            scene.format_version
        ));
    }
    Ok(())
}

fn path_matches(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|prefix| path.starts_with(prefix))
}

fn count_paths(records: &[PathRecord], prefix: Option<&str>) -> usize {
    records
        .iter()
        .filter(|record| path_matches(&record.path, prefix))
        .count()
}

fn add_geometry(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
) {
    let center = Vec3::from_array(geometry.transform.position);
    // Format 1 stores the three Roblox CFrame matrix rows. Reconstruct the
    // column vectors expected by glam before transforming local vertices.
    let basis = basis_from_rows(geometry.transform.rotation);
    let mut half = Vec3::from_array(geometry.size) * 0.5;
    let mut center = center;
    if let Some(mesh) = &geometry.mesh
        && mesh.kind == "SpecialMesh"
        && mesh.mesh_type == Some(2)
    {
        half *= Vec3::from_array(mesh.scale.unwrap_or([1.0, 1.0, 1.0]));
        center += basis * Vec3::from_array(mesh.offset.unwrap_or([0.0, 0.0, 0.0]));
    }
    if geometry.class == "WedgePart" {
        add_oriented_wedge(vertices, bounds, geometry, center, basis, half);
    } else {
        add_oriented_box(vertices, bounds, geometry, center, basis, half);
    }
}

fn basis_from_rows(rotation: [[f32; 3]; 3]) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(rotation[0][0], rotation[1][0], rotation[2][0]),
        Vec3::new(rotation[0][1], rotation[1][1], rotation[2][1]),
        Vec3::new(rotation[0][2], rotation[1][2], rotation[2][2]),
    )
}

fn add_oriented_box(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
    center: Vec3,
    basis: Mat3,
    half: Vec3,
) {
    let local = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ];
    let world = local.map(|point| center + basis * point);
    for point in world {
        bounds.include(point);
    }
    let color = [
        geometry.color[0],
        geometry.color[1],
        geometry.color[2],
        1.0 - geometry.transparency,
    ];
    for (indices, local_normal) in [
        ([0, 3, 2, 0, 2, 1], Vec3::NEG_Z),
        ([4, 5, 6, 4, 6, 7], Vec3::Z),
        ([0, 4, 7, 0, 7, 3], Vec3::NEG_X),
        ([1, 2, 6, 1, 6, 5], Vec3::X),
        ([0, 1, 5, 0, 5, 4], Vec3::NEG_Y),
        ([3, 7, 6, 3, 6, 2], Vec3::Y),
    ] {
        let normal = (basis * local_normal).normalize_or_zero().to_array();
        vertices.extend(indices.into_iter().map(|index| ReferenceVertex {
            position: world[index].to_array(),
            normal,
            color,
        }));
    }
}

fn add_oriented_wedge(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
    center: Vec3,
    basis: Mat3,
    half: Vec3,
) {
    let local = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
    ];
    let world = local.map(|point| center + basis * point);
    for point in world {
        bounds.include(point);
    }
    let color = [
        geometry.color[0],
        geometry.color[1],
        geometry.color[2],
        1.0 - geometry.transparency,
    ];
    for indices in [
        [0, 1, 3],
        [0, 3, 2],
        [0, 4, 5],
        [0, 5, 1],
        [2, 3, 5],
        [2, 5, 4],
        [0, 2, 4],
        [1, 5, 3],
    ] {
        add_triangle(vertices, world, indices, color);
    }
}

fn add_triangle(
    vertices: &mut Vec<ReferenceVertex>,
    points: [Vec3; 6],
    indices: [usize; 3],
    color: [f32; 4],
) {
    let first = points[indices[0]];
    let second = points[indices[1]];
    let third = points[indices[2]];
    let normal = (second - first)
        .cross(third - first)
        .normalize_or_zero()
        .to_array();
    vertices.extend(indices.into_iter().map(|index| ReferenceVertex {
        position: points[index].to_array(),
        normal,
        color,
    }));
}

#[derive(Clone, Copy)]
struct CaptureCamera {
    position: Vec3,
    target: Vec3,
    field_of_view_degrees: f32,
    near: f32,
    far: f32,
}

fn fit_camera(bounds: SceneBounds, aspect: f32) -> CaptureCamera {
    let size = bounds.size();
    let center = bounds.center();
    let field_of_view_degrees: f32 = 55.0;
    let vertical_fov = field_of_view_degrees.to_radians();
    let horizontal_fov = 2.0 * ((vertical_fov * 0.5).tan() * aspect).atan();
    let fit_height = size.y / (vertical_fov * 0.5).tan();
    let fit_width = size.x.max(size.z) / (horizontal_fov * 0.5).tan();
    let distance = fit_height.max(fit_width) * 0.58 + size.length() * 0.25;
    let direction = Vec3::new(0.08, 0.28, 1.0).normalize();
    let target = center + Vec3::new(0.0, size.y * 0.06, 0.0);
    let position = target + direction * distance;
    CaptureCamera {
        position,
        target,
        field_of_view_degrees,
        near: (distance * 0.01).max(0.05),
        far: distance + size.length() * 2.5,
    }
}

fn lighting_values(project: Option<&ProjectLighting>) -> LightingValues {
    let defaults = LightingValues {
        brightness: 2.0,
        ambient: Vec3::ZERO,
        outdoor_ambient: Vec3::splat(0.5),
        correction_brightness: 0.0,
        correction_contrast: 0.0,
        correction_saturation: 0.0,
    };
    let Some(project) = project else {
        return defaults;
    };
    let correction = project
        .effects
        .iter()
        .find(|effect| effect.class == "ColorCorrectionEffect" && effect_enabled(effect));
    LightingValues {
        brightness: number(&project.properties, "Brightness").unwrap_or(defaults.brightness),
        ambient: color(&project.properties, "Ambient").unwrap_or(defaults.ambient),
        outdoor_ambient: color(&project.properties, "OutdoorAmbient")
            .unwrap_or(defaults.outdoor_ambient),
        correction_brightness: correction
            .and_then(|effect| number(&effect.properties, "Brightness"))
            .unwrap_or(0.0),
        correction_contrast: correction
            .and_then(|effect| number(&effect.properties, "Contrast"))
            .unwrap_or(0.0),
        correction_saturation: correction
            .and_then(|effect| number(&effect.properties, "Saturation"))
            .unwrap_or(0.0),
    }
}

fn number(properties: &BTreeMap<String, Value>, key: &str) -> Option<f32> {
    properties.get(key)?.as_f64().map(|value| value as f32)
}

fn color(properties: &BTreeMap<String, Value>, key: &str) -> Option<Vec3> {
    let values = properties.get(key)?.as_array()?;
    (values.len() == 3).then(|| {
        Vec3::new(
            values[0].as_f64().unwrap_or_default() as f32,
            values[1].as_f64().unwrap_or_default() as f32,
            values[2].as_f64().unwrap_or_default() as f32,
        )
    })
}

fn effect_enabled(effect: &ProjectEffect) -> bool {
    effect
        .properties
        .get("Enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn enabled_effect_count(scene: &ReferenceScene, class: &str) -> usize {
    scene
        .project_lighting
        .iter()
        .flat_map(|lighting| &lighting.effects)
        .filter(|effect| effect.class == class && effect_enabled(effect))
        .count()
}

struct RenderResult {
    pixels: Vec<u8>,
    sample_count: u32,
    adapter: wgpu::AdapterInfo,
}

fn render(
    vertices: &[ReferenceVertex],
    width: u32,
    height: u32,
    antialias: bool,
    camera: CaptureCamera,
    lighting: LightingValues,
) -> Result<RenderResult, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .or_else(|_| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
    })
    .map_err(|error| format!("headless adapter unavailable: {error}"))?;
    let adapter_info = adapter.get_info();
    let sample_count = super::targets::select_samples(&adapter, antialias);
    let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Roblox reference capture device"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("headless device unavailable: {error}"))?;

    let projection = Mat4::perspective_rh(
        camera.field_of_view_degrees.to_radians(),
        width as f32 / height as f32,
        camera.near,
        camera.far,
    );
    let view = Mat4::look_at_rh(camera.position, camera.target, Vec3::Y);
    let ambient = lighting.ambient.max(lighting.outdoor_ambient * 0.72);
    let globals = ReferenceGlobals {
        view_projection: (projection * view).to_cols_array_2d(),
        sun_direction_brightness: [-0.52, 0.78, -0.35, lighting.brightness.max(0.0)],
        ambient: [ambient.x, ambient.y, ambient.z, 0.0],
        color_correction: [
            lighting.correction_brightness,
            lighting.correction_contrast,
            lighting.correction_saturation,
            0.0,
        ],
    };
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Roblox reference vertices"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Roblox reference globals"),
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Roblox reference globals layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Roblox reference globals bind group"),
        layout: &globals_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buffer.as_entire_binding(),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Roblox reference capture shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("reference_capture.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Roblox reference capture pipeline layout"),
        bind_group_layouts: &[Some(&globals_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Roblox reference capture pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[ReferenceVertex::LAYOUT],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: super::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let color_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Roblox reference capture color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let multisample_view = (sample_count > 1).then(|| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Roblox reference capture multisample color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    });
    let depth_view = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Roblox reference capture depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: super::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());

    let padded_row_bytes = align_to(width as u64 * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Roblox reference capture readback"),
        size: padded_row_bytes * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Roblox reference capture encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Roblox reference capture pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: multisample_view.as_ref().unwrap_or(&color_view),
                depth_slice: None,
                resolve_target: multisample_view.as_ref().map(|_| &color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.30,
                        g: 0.88,
                        b: 0.96,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &globals_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }
    encoder.copy_texture_to_buffer(
        color_texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes as u32),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("poll reference capture device: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("receive reference capture readback: {error}"))?
        .map_err(|error| format!("map reference capture readback: {error}"))?;
    let mapped = slice.get_mapped_range();
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let source = row * padded_row_bytes as usize;
        let target = row * width as usize * 4;
        pixels[target..target + width as usize * 4]
            .copy_from_slice(&mapped[source..source + width as usize * 4]);
    }
    drop(mapped);
    readback.unmap();
    Ok(RenderResult {
        pixels,
        sample_count,
        adapter: adapter_info,
    })
}

fn align_to(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file =
        fs::File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("write PNG header: {error}"))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("write PNG pixels: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_filter_accepts_whole_scene_or_prefix() {
        let path = "Folder:Place[1]/Model:MainIsland[1]/Part:Grass[1]";
        assert!(path_matches(path, None));
        assert!(path_matches(
            path,
            Some("Folder:Place[1]/Model:MainIsland[1]")
        ));
        assert!(!path_matches(path, Some("Folder:Place[1]/Model:Other[1]")));
    }

    #[test]
    fn reconstructs_glam_basis_from_format_one_matrix_rows() {
        let basis = basis_from_rows([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]);
        assert!((basis * Vec3::X - Vec3::NEG_Z).length() < 0.0001);
        assert!((basis * Vec3::Y - Vec3::Y).length() < 0.0001);
        assert!((basis * Vec3::Z - Vec3::X).length() < 0.0001);
    }
}
