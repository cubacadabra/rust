use super::*;

impl super::super::Renderer {
    pub(crate) fn studio_overlay_format(&self) -> wgpu::TextureFormat {
        super::super::targets::SCENE_FORMAT
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_project_world_point(&self, point: [f32; 3]) -> Option<[f32; 2]> {
        let (view_projection, viewport) = self.studio_view_projection();
        Self::project_world_point(view_projection, viewport, point)
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_project_world_points(&self, points: &[[f32; 3]]) -> Vec<Option<[f32; 2]>> {
        let (view_projection, viewport) = self.studio_view_projection();
        points
            .iter()
            .copied()
            .map(|point| Self::project_world_point(view_projection, viewport, point))
            .collect()
    }

    #[cfg(feature = "studio-ui")]
    fn project_world_point(
        view_projection: Mat4,
        viewport: (f32, f32, f32, f32),
        point: [f32; 3],
    ) -> Option<[f32; 2]> {
        let clip = view_projection * Vec3::from_array(point).extend(1.0);
        if !clip.is_finite() || clip.w <= 0.0001 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        let (x, y, width, height) = viewport;
        Some([
            x + (ndc.x * 0.5 + 0.5) * width,
            y + (0.5 - ndc.y * 0.5) * height,
        ])
        .filter(|point| point.iter().all(|value| value.is_finite()))
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_world_point_on_horizontal_plane(
        &self,
        screen: [f32; 2],
        plane_y: f32,
    ) -> Option<[f32; 3]> {
        let (view_projection, (x, y, width, height)) = self.studio_view_projection();
        if width <= 0.0 || height <= 0.0 || !plane_y.is_finite() {
            return None;
        }
        let ndc_x = ((screen[0] - x) / width) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((screen[1] - y) / height) * 2.0;
        let inverse = view_projection.inverse();
        let near = inverse * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
        let far = inverse * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        if !near.is_finite() || !far.is_finite() || near.w.abs() <= 0.0001 || far.w.abs() <= 0.0001
        {
            return None;
        }
        let near = near.truncate() / near.w;
        let far = far.truncate() / far.w;
        let direction = far - near;
        if direction.y.abs() <= 0.0001 {
            return None;
        }
        let distance = (plane_y - near.y) / direction.y;
        if !distance.is_finite() || distance < 0.0 {
            return None;
        }
        let point = near + direction * distance;
        point.is_finite().then(|| point.to_array())
    }

    #[cfg(feature = "studio-ui")]
    fn studio_view_projection(&self) -> (Mat4, (f32, f32, f32, f32)) {
        let viewport = self.world_viewport();
        let aspect = (viewport.2 / viewport.3.max(1.0)).max(0.1);
        let (camera_position, target) = self.camera_view(aspect);
        let view = Mat4::look_at_rh(camera_position, target, Vec3::Y);
        let projection = Mat4::perspective_rh(
            62.0_f32.to_radians(),
            aspect,
            0.05,
            self.camera_far_plane(camera_position, target),
        );
        (projection * view, viewport)
    }

    pub(super) fn camera_view(&self, aspect: f32) -> (Vec3, Vec3) {
        #[cfg(not(feature = "studio-ui"))]
        let _ = aspect;
        let player = Vec3::from_array(self.scene.player.position);
        let [yaw, pitch, distance] = self.scene.camera;
        let gameplay =
            || crate::camera::orbit(player, self.scene.player.body, yaw, pitch, distance);

        #[cfg(feature = "studio-ui")]
        if self.studio_camera_preset != crate::StudioCameraPreset::Gameplay {
            let (minimum, maximum) = self.presentation_bounds();
            return self
                .studio_camera
                .view(self.studio_camera_preset, minimum, maximum, aspect);
        }

        gameplay()
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn navigate_studio_camera(
        &mut self,
        orbit: [f32; 2],
        pan: [f32; 2],
        zoom: f32,
        viewport_height: f32,
    ) {
        if self.studio_camera_preset == crate::StudioCameraPreset::Gameplay {
            return;
        }
        let (_, _, width, height) = self.world_viewport();
        let (camera, target) = self.camera_view(width / height.max(1.0));
        self.studio_camera
            .navigate(orbit, pan, zoom, camera, target, viewport_height);
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn focus_studio_camera(&mut self, point: [f32; 3], radius: f32) {
        if self.studio_camera_preset == crate::StudioCameraPreset::Gameplay {
            return;
        }
        self.studio_camera.focus(Vec3::from_array(point), radius);
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_camera_ground_axes(&self) -> ([f32; 2], [f32; 2]) {
        let (_, _, width, height) = self.world_viewport();
        let (minimum, maximum) = self.presentation_bounds();
        self.studio_camera.ground_axes(
            self.studio_camera_preset,
            minimum,
            maximum,
            width / height.max(1.0),
        )
    }

    pub(super) fn camera_far_plane(&self, camera: Vec3, target: Vec3) -> f32 {
        #[cfg(not(feature = "studio-ui"))]
        let _ = (camera, target);
        #[cfg(feature = "studio-ui")]
        if self.studio_camera_preset != crate::StudioCameraPreset::Gameplay {
            let (minimum, maximum) = self.presentation_bounds();
            let radius = ((maximum - minimum) * 0.5).length();
            return (camera.distance(target) + radius * 2.0 + 32.0).max(240.0);
        }
        240.0
    }

    #[cfg(feature = "studio-ui")]
    fn presentation_bounds(&self) -> (Vec3, Vec3) {
        if let Some((minimum, maximum)) = self.scene.world.presentation_bounds {
            return (Vec3::from_array(minimum), Vec3::from_array(maximum));
        }
        let mut minimum = Vec3::splat(f32::INFINITY);
        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
        let mut has_authored_geometry = false;
        let mut include = |low: Vec3, high: Vec3| {
            has_authored_geometry = true;
            minimum = minimum.min(low);
            maximum = maximum.max(high);
        };
        if let Some((low, high)) = self
            .scene
            .world
            .terrain
            .as_ref()
            .and_then(crate::terrain::TerrainGrid::allocated_bounds)
        {
            include(Vec3::from_array(low), Vec3::from_array(high));
        }
        if !self.scene.world.hide_default_ground {
            let half_ground = self.scene.world.ground_size * 0.5;
            include(
                Vec3::new(-half_ground, self.scene.world.ground_y - 0.1, -half_ground),
                Vec3::new(half_ground, self.scene.world.ground_y + 0.1, half_ground),
            );
        }
        for block in &self.scene.world.blocks {
            let half = Vec3::from_array(block.size) * 0.5;
            let position = Vec3::from_array(block.position);
            include(position - half, position + half);
        }
        for decoration in &self.scene.world.decorations {
            let position = Vec3::from_array(decoration.position);
            let extent = Vec3::splat(decoration.scale.max(0.5) * 2.0);
            include(position - extent, position + extent);
        }
        for mesh in &self.scene.world.mesh_instances {
            let position = Vec3::from_array(mesh.position);
            let extent = Vec3::from_array(mesh.scale.map(|scale| scale.max(0.5))) * 2.0;
            include(position - extent, position + extent);
        }
        for ladder in &self.scene.world.ladders {
            let half = Vec3::from_array(ladder.size) * 0.5;
            let position = Vec3::from_array(ladder.position);
            include(position - half, position + half);
        }
        for pad in &self.scene.world.pads {
            let position = Vec3::new(pad.x, self.scene.world.ground_y, pad.z);
            let extent = Vec3::splat(pad.radius.max(0.5));
            include(position - extent, position + extent);
        }
        for sign in &self.scene.world.signs {
            let position = Vec3::from_array(sign.position);
            include(position - Vec3::splat(1.0), position + Vec3::splat(1.0));
        }
        for billboard in &self.scene.world.billboards {
            let position = Vec3::from_array(billboard.position);
            let extent = Vec3::new(billboard.width, billboard.height, billboard.width);
            include(position - extent, position + extent);
        }
        for interaction in &self.scene.world.interactions {
            let position = Vec3::from_array(interaction.position);
            let extent = Vec3::splat(interaction.radius.max(0.5));
            include(position - extent, position + extent);
        }
        if !has_authored_geometry {
            minimum = Vec3::from_array(self.scene.player.position);
            maximum = minimum;
        }
        (minimum, maximum)
    }
}
