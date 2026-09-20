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

#[path = "reference_capture_scene.rs"]
mod reference_capture_scene;
use reference_capture_scene::{GeometryInstance, PathRecord, ReferenceScene, SceneBounds};
#[path = "reference_capture_render.rs"]
mod reference_capture_render;
#[cfg(test)]
use reference_capture_render::basis_from_rows;
use reference_capture_render::{
    add_geometry, enabled_effect_count, ensure_parent, fit_camera, lighting_values, render,
    write_png,
};

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
