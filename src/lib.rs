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
    pub use crate::renderer::capture::{
        CaptureAvatar, CaptureConfig, CapturePalette, CaptureQuality, CaptureReport,
        capture_phase0_baseline, capture_phase2_shape_proof, capture_phase5_outfits,
        capture_phase6_report,
    };
    pub use crate::renderer::validation::capture_phase3;

    pub fn validate_phase5_catalog(
        path: impl AsRef<Path>,
    ) -> Result<CatalogValidationReport, String> {
        let source = std::fs::read_to_string(path.as_ref())
            .map_err(|error| format!("read {}: {error}", path.as_ref().display()))?;
        crate::character::catalog::validate_catalog(&source)
    }
}
mod scripting;
mod types;
mod ui;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
mod web_renderer;
mod world;

pub use engine::Engine;
