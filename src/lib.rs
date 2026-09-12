mod character;
mod effects;
mod engine;
mod ffi;
mod game_package;
mod math;
mod npc;
mod player;
#[cfg(any(not(target_arch = "wasm32"), feature = "web-renderer"))]
mod renderer;
mod schema;
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub mod dev_showcase {
    use std::path::Path;

    pub use crate::character::catalog::CatalogValidationReport;
    pub use crate::character::catalog::StyleExamplesValidationReport;
    pub use crate::renderer::capture::{
        CaptureAvatar, CaptureConfig, CapturePalette, CaptureQuality, CaptureReport,
        HeroCaptureSet, MotionCaptureMode, Phase8RolloutReport, capture_phase0_baseline,
        capture_phase2_shape_proof, capture_phase4_motion, capture_phase4_motion_with_mode,
        capture_phase5_outfits, capture_phase6_report, capture_phase8_rollout, capture_phase9_hero,
        capture_phase9_hero_with_set,
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
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
pub use web_renderer::WebRenderer;
mod world;

pub use engine::Engine;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub mod native {
    use crate::Engine;
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

    /// The shared renderer presented by a native desktop host.
    pub struct Renderer {
        inner: crate::renderer::Renderer,
    }

    impl Renderer {
        /// Creates a renderer for a live native window. The host must keep the
        /// underlying window alive until this renderer is dropped.
        pub fn new(
            display_handle: RawDisplayHandle,
            window_handle: RawWindowHandle,
            width: f32,
            height: f32,
        ) -> Option<Self> {
            crate::renderer::Renderer::new_from_window_handles(
                display_handle,
                window_handle,
                width,
                height,
            )
            .map(|inner| Self { inner })
        }

        pub fn resize(&mut self, width: f32, height: f32) {
            self.inner.resize(width, height);
        }

        pub fn sync(&mut self, engine: &Engine) {
            self.inner.sync_engine(engine);
        }

        pub fn draw(&mut self) {
            self.inner.draw();
        }

        #[cfg(feature = "studio-ui")]
        pub fn draw_with_overlay<F>(&mut self, overlay: F)
        where
            F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
        {
            self.inner.draw_with_overlay(overlay);
        }

        #[cfg(feature = "studio-ui")]
        pub fn set_studio_viewport(&mut self, viewport: Option<[f32; 4]>) {
            self.inner.set_studio_viewport(viewport);
        }

        #[cfg(feature = "studio-ui")]
        pub fn device(&self) -> &wgpu::Device {
            &self.inner.device
        }

        #[cfg(feature = "studio-ui")]
        pub fn studio_overlay_format(&self) -> wgpu::TextureFormat {
            self.inner.studio_overlay_format()
        }

        pub fn set_package_image_atlas(
            &mut self,
            width: u32,
            height: u32,
            pixels: &[u8],
            regions: std::collections::BTreeMap<String, [f32; 4]>,
        ) -> bool {
            self.inner
                .set_package_image_atlas(width, height, pixels, regions)
        }

        pub fn register_morph_pack(
            &mut self,
            bytes: &[u8],
        ) -> Result<(), Vec<cubacadabra_morphs::MorphDiagnostic>> {
            self.inner.register_morph_pack(bytes)
        }

        #[cfg(feature = "studio-ui")]
        pub fn set_avatar_preview_mode(&mut self, enabled: bool) {
            self.inner.set_avatar_preview_mode(enabled);
        }
    }
}
