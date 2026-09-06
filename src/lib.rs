mod character;
mod engine;
mod ffi;
mod game_package;
mod math;
mod npc;
mod player;
#[cfg(any(not(target_arch = "wasm32"), feature = "web-renderer"))]
mod renderer;
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub mod dev_showcase {
    use std::path::Path;

    pub use crate::character::catalog::CatalogValidationReport;
    pub use crate::character::catalog::StyleExamplesValidationReport;
    pub use crate::renderer::capture::{
        CaptureAvatar, CaptureConfig, CapturePalette, CaptureQuality, CaptureReport,
        HeroCaptureSet, Phase8RolloutReport, capture_phase0_baseline, capture_phase2_shape_proof,
        capture_phase4_motion, capture_phase4_motion_with_mode, capture_phase5_outfits, capture_phase6_report,
        capture_phase8_rollout, MotionCaptureMode,
        capture_phase9_hero, capture_phase9_hero_with_set,
    };
    pub use crate::renderer::validation::capture_phase3;

    pub fn validate_phase5_catalog(
        path: impl AsRef<Path>,
    ) -> Result<CatalogValidationReport, String> {
        let source = std::fs::read_to_string(path.as_ref())
            .map_err(|error| format!("read {}: {error}", path.as_ref().display()))?;
        crate::character::catalog::validate_catalog(&source)
    }

    pub fn validate_style_examples(
        path: impl AsRef<Path>,
    ) -> Result<StyleExamplesValidationReport, String> {
        let source = std::fs::read_to_string(path.as_ref())
            .map_err(|error| format!("read {}: {error}", path.as_ref().display()))?;
        crate::character::catalog::validate_style_examples(&source)
    }
}
mod scripting;
mod types;
mod ui;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
mod web_renderer;
mod world;

pub use engine::Engine;
