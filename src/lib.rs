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
    pub use crate::renderer::capture::{
        CaptureConfig, CapturePalette, CaptureQuality, CaptureReport, capture_phase0_baseline,
    };
}
mod scripting;
mod types;
mod ui;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
mod web_renderer;
mod world;

pub use engine::Engine;
