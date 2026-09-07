//! Opt-in, offscreen review fixtures for the character program.
//!
//! Legacy and magic rollout captures exercise the two renderer paths. Rounded
//! and CPU shape-proof captures remain historical before-images. Phase 2 now
//! reviews shapes with the production GPU materials. The feature is
//! kept out of normal client builds so fixtures cannot change simulation
//! capacity, public snapshots, or runtime resource lifetime.

use super::{
    DEPTH_FORMAT, Globals, RenderEntity, RenderPalette, Vertex, add_avatar, add_cuboid,
    add_legacy_avatar, color,
};
use super::character_quality::{
    CHARACTER_FAR_PLANE, LOD_FAR_PIXELS, LOD_HYSTERESIS_PIXELS, LOD_NEAR_PIXELS, MAX_EFFECTS,
    MAX_EFFECTS_PER_CHARACTER, CharacterLod,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const FORMAT_VERSION: u32 = 2;
const WORLD_ASPECT: f32 = 16.0 / 9.0;
const DEFAULT_SEED: u64 = 0xC0BA_CAFE;
const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 360;

#[path = "capture_motion.rs"]
mod motion;
pub use motion::{MotionCaptureMode, capture_phase4_motion, capture_phase4_motion_with_mode};
#[path = "capture_hero.rs"]
mod hero;
pub use hero::{HeroCaptureSet, capture_phase9_hero, capture_phase9_hero_with_set};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CaptureAvatar {
    Legacy,
    Rounded,
    ShapeProof,
    Wardrobe,
    /// Uses the same indexed/instanced CharacterRenderer as the live draw
    /// path. Kept separate from Wardrobe for rollout reports.
    Magic,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CaptureQuality {
    Full,
    Half,
}

impl CaptureQuality {
    fn scale(self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CapturePalette {
    Current,
    HighContrast,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct CaptureConfig {
    pub seed: u64,
    pub pose_time: f32,
    pub width: u32,
    pub height: u32,
    pub portrait_width: u32,
    pub portrait_height: u32,
    pub quality: CaptureQuality,
    pub palette: CapturePalette,
    pub avatar: CaptureAvatar,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            pose_time: 0.35,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            portrait_width: 390,
            portrait_height: 844,
            quality: CaptureQuality::Full,
            palette: CapturePalette::Current,
            avatar: CaptureAvatar::Legacy,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CaptureRecord {
    pub name: String,
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub world_viewport: [u32; 4],
    pub actor_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub render_mode: &'static str,
    pub character_draws: usize,
    pub character_instances: usize,
    pub character_mesh_uploads: usize,
    pub character_resident_bytes: usize,
    pub estimated_vertex_upload_bytes: usize,
    pub estimated_resource_bytes: usize,
    pub cpu_build_ms: f64,
    pub gpu_submit_and_readback_ms: f64,
    pub gpu_timestamp_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CaptureReport {
    pub format_version: u32,
    pub fixture: String,
    pub config: CaptureConfig,
    pub adapter: AdapterRecord,
    pub captures: Vec<CaptureRecord>,
    pub engine_capacity_characters: usize,
    pub render_only_stress_characters: usize,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Phase8RolloutReport {
    pub format_version: u32,
    pub fixture: &'static str,
    pub default_mode: &'static str,
    pub rollback_mode: &'static str,
    pub legacy_capture: CaptureReport,
    pub magic_capture: CaptureReport,
    pub wardrobe_capture: CaptureReport,
    pub compatibility_checks: Vec<&'static str>,
    pub retirement_checks: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Phase6QualityReport {
    pub format_version: u32,
    pub fixture: &'static str,
    pub lod: LodPolicyReport,
    pub effect_budget: EffectBudgetReport,
    pub cache: CachePolicyReport,
    pub catalog: CatalogPolicyReport,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct LodPolicyReport {
    pub near_pixels: f32,
    pub mid_pixels: f32,
    pub hysteresis_pixels: f32,
    pub near_subdivisions: u32,
    pub mid_subdivisions: u32,
    pub far_subdivisions: u32,
    pub far_plane: f32,
}

#[derive(Debug, Serialize)]
pub struct EffectBudgetReport {
    pub max_live_effects: usize,
    pub max_per_character: usize,
    pub reduced_effects_disables_seams: bool,
}

#[derive(Debug, Serialize)]
pub struct CachePolicyReport {
    pub max_meshes: usize,
    pub max_resident_bytes: usize,
    pub render_only_stress_characters: usize,
    pub engine_capacity_characters: usize,
}

#[derive(Debug, Serialize)]
pub struct CatalogPolicyReport {
    pub outfits: usize,
    pub materials: usize,
    pub texture_bytes: u64,
    pub licenses: usize,
}

/// Write the deterministic Phase 6 policy report. Runtime measurements remain
/// backend/device evidence from the production validation harness; this report
/// records the bounded decisions that make those runs comparable and prevents
/// a quality pass from silently changing simulation capacity.
pub fn capture_phase6_report(output_dir: impl AsRef<Path>) -> Result<Phase6QualityReport, String> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;
    let catalog = crate::character::catalog::validate_catalog(include_str!(
        "../../assets/characters/catalog.json"
    ))
    .map_err(|error| format!("validate Phase 5 catalog: {error}"))?;
    let report = Phase6QualityReport {
        format_version: 1,
        fixture: "phase-6-fidelity-and-sustained-performance-policy",
        lod: LodPolicyReport {
            near_pixels: LOD_NEAR_PIXELS,
            mid_pixels: LOD_FAR_PIXELS,
            hysteresis_pixels: LOD_HYSTERESIS_PIXELS,
            near_subdivisions: CharacterLod::Near.subdivisions(),
            mid_subdivisions: CharacterLod::Mid.subdivisions(),
            far_subdivisions: CharacterLod::Far.subdivisions(),
            far_plane: CHARACTER_FAR_PLANE,
        },
        effect_budget: EffectBudgetReport {
            max_live_effects: MAX_EFFECTS,
            max_per_character: MAX_EFFECTS_PER_CHARACTER,
            reduced_effects_disables_seams: true,
        },
        cache: CachePolicyReport {
            max_meshes: super::character_gpu::MAX_MESHES,
            max_resident_bytes: 32 * 1024 * 1024,
            render_only_stress_characters: 50,
            engine_capacity_characters: 18,
        },
        catalog: CatalogPolicyReport {
            outfits: catalog.outfit_count,
            materials: catalog.material_count,
            texture_bytes: catalog.texture_bytes,
            licenses: catalog.license_count,
        },
        notes: vec![
            "Near/mid/far selection uses projected height with 180px and 70px boundaries.",
            "Animated bounds are conservative and culling is presentation-only.",
            "The report records policy; sustained native/mobile/browser timings belong to the corresponding capture artifact.",
            "Directional shadows, AO and bloom remain opt-in alternatives until a device budget ratifies them.",
        ],
    };
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize Phase 6 report: {error}"))?;
    fs::write(output_dir.join("phase6_report.json"), bytes)
        .map_err(|error| format!("write Phase 6 report: {error}"))?;
    Ok(report)
}

#[derive(Debug, Serialize)]
pub struct AdapterRecord {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub gpu_timestamps: bool,
}

#[derive(Clone, Copy)]
enum Pose {
    Idle,
    Walk,
    Sprint,
    Jump,
}

#[derive(Clone, Copy)]
enum Camera {
    Third,
    First,
}

#[derive(Clone, Copy)]
enum Scenario {
    Single {
        name: &'static str,
        remote: bool,
        pose: Pose,
        camera: Camera,
    },
    Raised,
    Crowd {
        name: &'static str,
        count: usize,
        portrait: bool,
    },
    ShapeLineup {
        name: &'static str,
        camera_yaw: f32,
        silhouette: bool,
    },
    WardrobeLineup { name: &'static str, camera_yaw: f32 },
    MotionLineup,
    MotionMoving,
    MotionMovingRaised,
    HairReview,
    Hero { name: &'static str, yaw: f32, pitch: f32, distance: f32,
        study: super::hero_character::Study, silhouette: bool, motion: bool },
    Orbit { name: &'static str, yaw: f32, pitch: f32, distance: f32 },
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Single { name, .. } => name,
            Self::Raised => "raised-platform-third",
            Self::Crowd { name, .. } => name,
            Self::ShapeLineup { name, .. } => name,
            Self::WardrobeLineup { name, .. } => name,
            Self::MotionLineup => "motion",
            Self::MotionMoving => "motion-moving",
            Self::MotionMovingRaised => "motion-moving-raised",
            Self::HairReview => "hair-review",
            Self::Hero {name,..} => name,
            Self::Orbit { name, .. } => name,
        }
    }
}

const PHASE2_SCENARIOS: [Scenario; 4] = [
    Scenario::ShapeLineup {
        name: "shape-lineup-front",
        camera_yaw: std::f32::consts::PI,
        silhouette: false,
    },
    Scenario::ShapeLineup {
        name: "shape-lineup-side",
        camera_yaw: std::f32::consts::FRAC_PI_2,
        silhouette: false,
    },
    Scenario::ShapeLineup {
        name: "shape-lineup-back",
        camera_yaw: 0.0,
        silhouette: false,
    },
    Scenario::ShapeLineup {
        name: "shape-lineup-silhouette",
        camera_yaw: std::f32::consts::PI,
        silhouette: true,
    },
];

const PHASE5_SCENARIOS: [Scenario; 10] = [
    Scenario::WardrobeLineup { name: "wardrobe-front", camera_yaw: std::f32::consts::PI },
    Scenario::WardrobeLineup { name: "wardrobe-three-quarter", camera_yaw: 2.55 },
    Scenario::WardrobeLineup { name: "wardrobe-side", camera_yaw: std::f32::consts::FRAC_PI_2 },
    Scenario::WardrobeLineup { name: "wardrobe-back", camera_yaw: 0.0 },
    Scenario::Orbit { name: "orbit-front-close", yaw: std::f32::consts::PI, pitch: 0.0, distance: 2.3 },
    Scenario::Orbit { name: "orbit-side-close", yaw: std::f32::consts::FRAC_PI_2, pitch: 0.0, distance: 2.3 },
    Scenario::Orbit { name: "orbit-front-default", yaw: std::f32::consts::PI, pitch: 0.2, distance: 8.0 },
    Scenario::Orbit { name: "orbit-overhead-wide", yaw: 2.3, pitch: 1.3, distance: 120.0 },
    Scenario::Orbit { name: "orbit-first-person-entry", yaw: 1.0, pitch: 0.2, distance: 1.7 },
    Scenario::Orbit { name: "orbit-first-person", yaw: 1.0, pitch: 0.2, distance: 0.0 },
];

const PHASE0_SCENARIOS: [Scenario; 15] = [
    Scenario::Single {
        name: "local-idle-third",
        remote: false,
        pose: Pose::Idle,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-idle-third",
        remote: true,
        pose: Pose::Idle,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-walk-third",
        remote: false,
        pose: Pose::Walk,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-walk-third",
        remote: true,
        pose: Pose::Walk,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-sprint-third",
        remote: false,
        pose: Pose::Sprint,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-sprint-third",
        remote: true,
        pose: Pose::Sprint,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-jump-third",
        remote: false,
        pose: Pose::Jump,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-jump-third",
        remote: true,
        pose: Pose::Jump,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-idle-first",
        remote: false,
        pose: Pose::Idle,
        camera: Camera::First,
    },
    Scenario::Single {
        name: "remote-idle-first",
        remote: true,
        pose: Pose::Idle,
        camera: Camera::First,
    },
    Scenario::Raised,
    Scenario::Crowd {
        name: "crowd-18-landscape",
        count: 18,
        portrait: false,
    },
    Scenario::Crowd {
        name: "crowd-50-landscape",
        count: 50,
        portrait: false,
    },
    Scenario::Crowd {
        name: "crowd-18-portrait-letterboxed",
        count: 18,
        portrait: true,
    },
    Scenario::Crowd {
        name: "crowd-50-portrait-letterboxed",
        count: 50,
        portrait: true,
    },
];

/// Render the complete Phase 0 baseline suite to PNGs and a JSON measurement
/// report. The report is deterministic except for adapter/device metadata and
/// measured timings.
pub fn capture_phase0_baseline(
    output_dir: impl AsRef<Path>,
    config: CaptureConfig,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;

    let mut context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };

    let mut captures = Vec::with_capacity(PHASE0_SCENARIOS.len());
    for scenario in PHASE0_SCENARIOS {
        captures.push(context.capture(output_dir, config, scenario)?);
    }

    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: match config.avatar {
            CaptureAvatar::Legacy => "phase-0-legacy-avatar-offscreen",
            CaptureAvatar::Rounded => "phase-1-rounded-avatar-offscreen",
            CaptureAvatar::ShapeProof => "phase-2-three-body-shape-proof",
            CaptureAvatar::Wardrobe => "phase-5-six-outfit-procedural-review",
            CaptureAvatar::Magic => "phase-8-magic-instanced-rollout",
        }
        .to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "Legacy uses the compatibility hard-cuboid path; Magic/Wardrobe use the indexed instanced character path.",
            "The 50-character scene is render-only and never enters Engine simulation or the public snapshot.",
            "GPU timestamp queries are not requested by the production renderer; unavailable values are null.",
            "Portrait captures use the existing 16:9 world viewport centered inside the portrait target.",
            "Remote and local labels describe the source fixture; both use the current shared player appearance path.",
        ],
    };
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize capture report: {error}"))?;
    fs::write(output_dir.join("phase0_report.json"), report_bytes)
        .map_err(|error| format!("write capture report: {error}"))?;
    Ok(report)
}

/// Render the Phase 2 shape-proof review sheet. The lineup is isolated from
/// Engine simulation and the public snapshot, just like the Phase 0 stress
/// fixture.
pub fn capture_phase2_shape_proof(
    output_dir: impl AsRef<Path>,
    mut config: CaptureConfig,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    config.avatar = CaptureAvatar::Magic;
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;
    let mut context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };
    let mut captures = Vec::with_capacity(PHASE2_SCENARIOS.len());
    for scenario in PHASE2_SCENARIOS {
        captures.push(context.capture(output_dir, config, scenario)?);
    }
    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "phase-2-three-body-shape-proof".to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "The lineup contains person, cat and dragon recipes on the shared rigid-piece rig.",
            "Front, side, back and black-silhouette captures are deterministic review artifacts.",
            "The fixture is render-only and does not enable local NPC simulation or alter snapshots.",
            "Phase 0 legacy and Phase 1 rounded captures remain the before-images for comparison.",
        ],
    };
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize Phase 2 capture report: {error}"))?;
    fs::write(output_dir.join("phase2_report.json"), report_bytes)
        .map_err(|error| format!("write Phase 2 capture report: {error}"))?;
    Ok(report)
}

/// Render the six deterministic Phase 5 hero outfits without entering the
/// engine or changing the production renderer's simulation capacity.
pub fn capture_phase5_outfits(
    output_dir: impl AsRef<Path>,
    mut config: CaptureConfig,
) -> Result<CaptureReport, String> {
    crate::character::catalog::validate_catalog(include_str!(
        "../../assets/characters/catalog.json"
    ))
    .map_err(|error| format!("validate Phase 5 catalog: {error}"))?;
    let output_dir = output_dir.as_ref();
    config.avatar = CaptureAvatar::Wardrobe;
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;
    let mut context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };
    let captures = PHASE5_SCENARIOS
        .into_iter()
        .map(|scenario| context.capture(output_dir, config, scenario))
        .collect::<Result<Vec<_>, _>>()?;
    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "phase-5-six-outfit-procedural-review".to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "The six hero outfits are rendered from the bounded procedural catalog.",
            "Person hoodie/pajamas, cat puffer/raincoat, and dragon wizard/knight are shown together.",
            "The fixture is render-only and does not enable local NPC simulation or alter snapshots.",
            "Run validate_character_assets before reviewing capture output.",
        ],
    };
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize Phase 5 capture report: {error}"))?;
    fs::write(output_dir.join("phase5_report.json"), report_bytes)
        .map_err(|error| format!("write Phase 5 capture report: {error}"))?;
    Ok(report)
}

/// Produce the Phase 8 comparison artifact. The two suites intentionally use
/// separate output folders so reviewers can compare the same camera/viewport
/// evidence without overwriting either side of the rollback pair.
pub fn capture_phase8_rollout(
    output_dir: impl AsRef<Path>,
    config: CaptureConfig,
) -> Result<Phase8RolloutReport, String> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;

    let mut legacy_config = config;
    legacy_config.avatar = CaptureAvatar::Legacy;
    let legacy_capture = capture_phase0_baseline(output_dir.join("legacy"), legacy_config)?;

    let mut magic_config = config;
    magic_config.avatar = CaptureAvatar::Magic;
    let magic_capture = capture_phase0_baseline(output_dir.join("magic"), magic_config)?;
    let wardrobe_capture = capture_phase5_outfits(output_dir.join("wardrobe"), config)?;

    let report = Phase8RolloutReport {
        format_version: 1,
        fixture: "phase-8-reversible-renderer-rollout",
        // Magic remains the current default because the instanced path is the
        // committed production renderer. Hosts can select Legacy before sync
        // while a staged rollout or compatibility incident is investigated.
        default_mode: "magic",
        rollback_mode: "legacy",
        legacy_capture,
        magic_capture,
        wardrobe_capture,
        compatibility_checks: vec![
            "legacy hard-cuboid geometry is available through the renderer mode boundary",
            "legacy package colors remain siblings of the versioned character appearance",
            "public snapshot length, stride and suffix semantics are unchanged",
            "first-person hiding, third-person framing and portrait letterboxing use the existing camera rules",
            "the render-only 50-character suite does not alter the 18-character engine capacity",
        ],
        retirement_checks: vec![
            "live magic characters use immutable indexed meshes and instance uploads",
            "CPU-expanded magic geometry is restricted to the opt-in development capture fixture",
            "legacy rendering remains until host owners agree the compatibility window",
            "old-color migration remains independent from the visual rollback switch",
        ],
    };
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize Phase 8 rollout report: {error}"))?;
    fs::write(output_dir.join("phase8_report.json"), bytes)
        .map_err(|error| format!("write Phase 8 rollout report: {error}"))?;
    Ok(report)
}


#[path = "capture_context.rs"]
mod context;
#[path = "capture_scene.rs"]
mod scene;
use context::HeadlessContext;

mod tests {
    use super::*;
    use super::scene::{build_scene, crowd_actors, dimensions, world_viewport};

    #[test]
    fn phase0_dimensions_keep_portrait_world_letterbox() {
        let config = CaptureConfig::default();
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[0]), (640, 360));
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[13]), (390, 844));
        assert_eq!(world_viewport(390, 844), [0, 312, 390, 219]);
    }

    #[test]
    fn phase0_crowd_fixture_is_seeded_and_isolated() {
        let first = crowd_actors(50, DEFAULT_SEED);
        let second = crowd_actors(50, DEFAULT_SEED);
        assert_eq!(first.len(), 50);
        assert_eq!(first[17].position, second[17].position);
        assert_ne!(
            first[17].position,
            crowd_actors(50, DEFAULT_SEED + 1)[17].position
        );
    }

    #[test]
    fn phase0_quality_scales_capture_targets() {
        let config = CaptureConfig {
            quality: CaptureQuality::Half,
            ..CaptureConfig::default()
        };
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[0]), (320, 180));
    }

    #[test]
    fn phase0_local_sprint_preserves_the_legacy_snapshot_freeze() {
        let config = CaptureConfig::default();
        let (local, ..) = build_scene(config, PHASE0_SCENARIOS[4], 640, 360);
        let (remote, ..) = build_scene(config, PHASE0_SCENARIOS[5], 640, 360);
        assert!(
            local
                .iter()
                .zip(remote.iter())
                .any(|(local, remote)| local.position != remote.position)
        );
    }

    #[test]
    fn phase1_capture_switch_uses_the_rounded_mesh_path() {
        let rounded_config = CaptureConfig {
            avatar: CaptureAvatar::Rounded,
            ..CaptureConfig::default()
        };
        let (legacy, ..) = build_scene(CaptureConfig::default(), PHASE0_SCENARIOS[0], 640, 360);
        let (rounded, ..) = build_scene(rounded_config, PHASE0_SCENARIOS[0], 640, 360);
        assert!(rounded.len() > legacy.len());
    }

    #[test]
    fn phase2_shape_fixture_has_three_rooted_bodies() {
        let config = CaptureConfig::default();
        let (vertices, actors, ..) = build_scene(
            CaptureConfig {
                avatar: CaptureAvatar::ShapeProof,
                ..config
            },
            PHASE2_SCENARIOS[0],
            640,
            360,
        );
        assert_eq!(actors.len(), 3);
        assert!(vertices.len() > 3 * 15 * 36);
    }
}
