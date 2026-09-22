// Headless builds intentionally leave presentation-only CPU catalogs in the
// shared source tree while omitting their renderer consumers.
#![cfg_attr(not(feature = "rendering"), allow(dead_code, unused_imports))]

pub mod authority;
mod camera;
mod character;
pub mod data_model;
mod effects;
mod engine;
mod ffi;
mod game_package;
mod math;
mod npc;
mod player;
#[cfg(any(
    feature = "rendering",
    all(target_arch = "wasm32", feature = "web-renderer")
))]
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
    pub use crate::renderer::reference_capture::{
        RobloxReferenceCaptureConfig, RobloxReferenceCaptureReport, capture_roblox_reference,
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
mod static_collision;
pub use scripting::authority::LuauAuthorityRules;
pub mod server_runtime;
mod terrain;
mod types;
mod ui;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
mod web_renderer;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
pub use web_renderer::WebRenderer;
mod world;

pub use engine::Engine;
#[cfg(feature = "studio-ui")]
pub use engine::StudioUiNode;

#[cfg(feature = "studio-ui")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StudioCameraPreset {
    #[default]
    Gameplay,
    Overview,
    Showcase,
}
pub use engine::snapshot::{
    AgentSnapshot, BuildBlockSnapshot, CameraSnapshot, ENGINE_SNAPSHOT_FORMAT,
    ENGINE_SNAPSHOT_VERSION, EngineSnapshot, InputSnapshot, InteractionEventSnapshot,
    InteractionSnapshot, InteractionZoneSnapshot, LaunchPadSnapshot, MAX_ENGINE_SNAPSHOT_BYTES,
    PlayerEventSnapshot, PlayerRuntimeSnapshot, PlayerSnapshot, SnapshotError,
};

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "rendering"
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

        pub fn draw_with_overlay<F>(&mut self, overlay: F)
        where
            F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
        {
            self.inner.draw_with_overlay(overlay);
        }

        #[cfg(all(feature = "studio-ui", debug_assertions))]
        pub fn capture_studio_frame<F>(&mut self, overlay: F)
        where
            F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
        {
            self.inner.capture_studio_frame(overlay);
        }

        #[cfg(feature = "studio-ui")]
        pub fn set_studio_viewport(&mut self, viewport: Option<[f32; 4]>) {
            self.inner.set_studio_viewport(viewport);
        }

        #[cfg(feature = "studio-ui")]
        pub fn set_studio_camera_preset(&mut self, preset: crate::StudioCameraPreset) {
            self.inner.set_studio_camera_preset(preset);
        }

        /// Restores the selected review preset without changing gameplay state.
        #[cfg(feature = "studio-ui")]
        pub fn reset_studio_camera(&mut self) {
            self.inner.reset_studio_camera();
        }

        /// Navigates a review camera using logical-point drag deltas and viewport height.
        /// Positive zoom moves closer. Gameplay cameras ignore this input.
        #[cfg(feature = "studio-ui")]
        pub fn navigate_studio_camera(
            &mut self,
            orbit: [f32; 2],
            pan: [f32; 2],
            zoom: f32,
            viewport_height: f32,
        ) {
            self.inner
                .navigate_studio_camera(orbit, pan, zoom, viewport_height);
        }

        /// Frames one authored object in the current Studio camera.
        #[cfg(feature = "studio-ui")]
        pub fn focus_studio_camera(&mut self, point: [f32; 3], radius: f32) {
            self.inner.focus_studio_camera(point, radius);
        }

        /// Returns visible-camera right and away axes projected onto the ground plane.
        #[cfg(feature = "studio-ui")]
        pub fn studio_camera_ground_axes(&self) -> ([f32; 2], [f32; 2]) {
            self.inner.studio_camera_ground_axes()
        }

        pub fn device(&self) -> &wgpu::Device {
            &self.inner.device
        }

        pub fn studio_overlay_format(&self) -> wgpu::TextureFormat {
            self.inner.studio_overlay_format()
        }

        #[cfg(feature = "studio-ui")]
        pub fn studio_project_world_point(&self, point: [f32; 3]) -> Option<[f32; 2]> {
            self.inner.studio_project_world_point(point)
        }

        #[cfg(feature = "studio-ui")]
        pub fn studio_project_world_points(&self, points: &[[f32; 3]]) -> Vec<Option<[f32; 2]>> {
            self.inner.studio_project_world_points(points)
        }

        #[cfg(feature = "studio-ui")]
        pub fn studio_world_point_on_horizontal_plane(
            &self,
            screen: [f32; 2],
            plane_y: f32,
        ) -> Option<[f32; 3]> {
            self.inner
                .studio_world_point_on_horizontal_plane(screen, plane_y)
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

        pub fn register_world_mesh(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
            self.inner.register_world_mesh(id, bytes)
        }

        pub fn clear_world_meshes(&mut self) {
            self.inner.clear_world_meshes();
        }

        pub fn replace_world_meshes(&mut self, models: &[(&str, &[u8])]) -> Result<(), String> {
            self.inner.replace_world_meshes(models)
        }

        #[cfg(feature = "studio-ui")]
        pub fn set_avatar_preview_mode(&mut self, enabled: bool) {
            self.inner.set_avatar_preview_mode(enabled);
        }

        /// Hides runtime-only players and HUD while Studio is authoring.
        #[cfg(feature = "studio-ui")]
        pub fn set_studio_edit_mode(&mut self, enabled: bool) {
            self.inner.set_studio_edit_mode(enabled);
        }

        /// Enables or disables Studio's directional static-shadow pass.
        /// This is a presentation-only diagnostic control.
        #[cfg(feature = "studio-ui")]
        pub fn set_studio_shadows_enabled(&mut self, enabled: bool) {
            self.inner.set_studio_shadows_enabled(enabled);
        }

        /// Returns the last Studio renderer draw sub-timings in milliseconds:
        /// encode, presenter/composite, queue submit, and frame present.
        #[cfg(feature = "studio-ui")]
        pub fn studio_draw_timings_ms(&self) -> [f32; 4] {
            self.inner.studio_draw_timings_ms
        }
    }
}
