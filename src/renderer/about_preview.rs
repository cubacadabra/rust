//! An About surface renders a real, isolated game session.
//!
//! The session and its scripted demo driver are shared by the native hosts.
//! This module provides the second render target and temporarily projects that
//! session through the normal renderer. Keeping the scene conversion here
//! means every About surface uses the same world, effects, interaction states,
//! and player renderer as every other host.

use crate::{engine::Engine, renderer::Scene};

use super::Renderer;

pub(super) const ABOUT_WIDTH: u32 = 1_000;
pub(super) const ABOUT_HEIGHT: u32 = 560;

impl Renderer {
    pub(crate) fn about_preview_texture(&self) -> &wgpu::TextureView {
        &self.about_targets.color
    }

    pub(crate) fn render_about_preview(&mut self, engine: &Engine) {
        let saved_scene = std::mem::replace(&mut self.scene, Scene::default());
        let saved_worlds = std::mem::take(&mut self.worlds);
        let saved_active_world = self.active_world;
        let saved_package_generation = self.package_generation;
        let saved_ui_frame = std::mem::take(&mut self.ui_frame);
        let saved_avatar_preview_mode = self.avatar_preview_mode;
        let saved_width = self.width;
        let saved_height = self.height;

        // Force sync_engine to resolve the About package into the renderer's
        // normal Scene representation. Static world geometry is uploaded into
        // the ordinary buffers for this pass, then rebuilt for Studio's live
        // package before returning to the main surface.
        self.worlds = Vec::new();
        self.active_world = usize::MAX;
        self.package_generation = u32::MAX;
        self.avatar_preview_mode = false;
        self.sync_engine(engine);

        self.width = ABOUT_WIDTH as f32;
        self.height = ABOUT_HEIGHT as f32;
        self.about_rendering = true;
        #[cfg(feature = "studio-ui")]
        let saved_viewport = self.studio_viewport.take();
        #[cfg(feature = "studio-ui")]
        let saved_camera_preset = std::mem::replace(
            &mut self.studio_camera_preset,
            crate::StudioCameraPreset::Gameplay,
        );
        #[cfg(feature = "studio-ui")]
        let saved_edit_mode = std::mem::replace(&mut self.studio_edit_mode, false);
        std::mem::swap(&mut self.targets, &mut self.about_targets);

        if let Some((_, encoder, _)) = self.encode_frame(true) {
            self.queue.submit(Some(encoder.finish()));
        }

        std::mem::swap(&mut self.targets, &mut self.about_targets);
        self.about_rendering = false;
        self.scene = saved_scene;
        self.worlds = saved_worlds;
        self.active_world = saved_active_world;
        self.package_generation = saved_package_generation;
        self.ui_frame = saved_ui_frame;
        self.avatar_preview_mode = saved_avatar_preview_mode;
        self.width = saved_width;
        self.height = saved_height;
        #[cfg(feature = "studio-ui")]
        {
            self.studio_viewport = saved_viewport;
            self.studio_camera_preset = saved_camera_preset;
            self.studio_edit_mode = saved_edit_mode;
        }
        self.rebuild_static_vertices();
    }
}
