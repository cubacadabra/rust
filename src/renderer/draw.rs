use super::character_material::CharacterPass;
use super::character_quality;
#[cfg(feature = "studio-ui")]
use glam::Vec4;
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

use super::CharacterRenderMode;
#[cfg(debug_assertions)]
use super::add_floor_pixel_text;
use super::{
    Globals, RenderEntity, Renderer, Vertex, add_billboard, add_cloud, add_cuboid,
    add_cuboid_outline, add_decoration, add_ladder, add_launch_pad, add_pixel_text, add_spawn_pad,
    add_textured_cuboid, faded,
};
use crate::terrain::TerrainVertex;

mod terrain {
    include!("draw/terrain.rs");
}
mod frame {
    include!("draw/frame.rs");
}

pub(super) use terrain::TerrainMeshBuilder;

fn shadow_light_direction(sun_direction: [f32; 3]) -> Vec3 {
    let light_direction = (-Vec3::from_array(sun_direction)).normalize_or_zero();
    if light_direction.length_squared() > 0.001 {
        light_direction
    } else {
        Vec3::new(0.45, 0.82, -0.32).normalize()
    }
}

fn screen_sun(
    view_projection: Mat4,
    camera: Vec3,
    sun_direction: [f32; 3],
    viewport: (f32, f32, f32, f32),
    output: (f32, f32),
) -> ([f32; 4], [f32; 4]) {
    let toward_sun = shadow_light_direction(sun_direction);
    let clip = view_projection * (camera + toward_sun * 180.0).extend(1.0);
    let ndc = (clip.w.abs() > 0.0001)
        .then(|| clip.truncate() / clip.w)
        .unwrap_or(Vec3::ZERO);
    let visible = f32::from(
        clip.w > 0.0001
            && ndc.x.abs() < 1.08
            && ndc.y.abs() < 1.08
            && ndc.z >= 0.0
            && ndc.z <= 1.0
            && toward_sun.y > -0.08,
    );
    let pixel = [
        viewport.0 + (ndc.x * 0.5 + 0.5) * viewport.2,
        viewport.1 + (0.5 - ndc.y * 0.5) * viewport.3,
    ];
    let normalized = [
        pixel[0] / output.0.max(1.0),
        pixel[1] / output.1.max(1.0),
        visible,
        0.0,
    ];
    ([pixel[0], pixel[1], toward_sun.y, visible], normalized)
}

fn vertex_capacity_for(required: usize, max_buffer_size: u64) -> Option<usize> {
    if required == 0 {
        return Some(0);
    }
    let vertex_size = std::mem::size_of::<Vertex>() as u64;
    let required_bytes = (required as u64).checked_mul(vertex_size)?;
    if required_bytes > max_buffer_size {
        return None;
    }

    let rounded = required.checked_next_power_of_two().unwrap_or(required);
    let rounded_bytes = (rounded as u64).checked_mul(vertex_size)?;
    Some(if rounded_bytes <= max_buffer_size {
        rounded
    } else {
        // Power-of-two growth is only an optimization. Near the device limit,
        // allocate the exact mesh size rather than crossing wgpu's hard cap.
        required
    })
}

impl Renderer {
    pub(crate) fn studio_overlay_format(&self) -> wgpu::TextureFormat {
        super::targets::SCENE_FORMAT
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_project_world_point(&self, point: [f32; 3]) -> Option<[f32; 2]> {
        let (view_projection, viewport) = self.studio_view_projection();
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

    fn camera_view(&self, aspect: f32) -> (Vec3, Vec3) {
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

    fn camera_far_plane(&self, camera: Vec3, target: Vec3) -> f32 {
        #[cfg(feature = "studio-ui")]
        if self.studio_camera_preset != crate::StudioCameraPreset::Gameplay {
            let (minimum, maximum) = self.presentation_bounds();
            let radius = ((maximum - minimum) * 0.5).length();
            return (camera.distance(target) + radius * 2.0 + 32.0).max(240.0);
        }
        240.0
    }

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

    pub fn draw(&mut self) {
        let Some((frame, mut encoder, view)) = self.encode_frame(false) else {
            return;
        };
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.expect("presented frame").present();
    }

    pub(crate) fn draw_with_overlay<F>(&mut self, overlay: F)
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let Some((frame, mut encoder, view)) = self.encode_frame(false) else {
            return;
        };
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.expect("presented frame").present();
    }

    /// Runs the production scene and overlay passes into the app-owned target
    /// when a development capture cannot acquire an on-screen drawable.
    #[cfg(all(feature = "studio-ui", debug_assertions))]
    pub(crate) fn capture_studio_frame<F>(&mut self, overlay: F)
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let Some((_, mut encoder, _)) = self.encode_frame(true) else {
            return;
        };
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        self.queue.submit(Some(encoder.finish()));
    }

    fn review_fog_range(&self) -> (f32, f32) {
        #[cfg(feature = "studio-ui")]
        if self.studio_camera_preset != crate::StudioCameraPreset::Gameplay {
            return (1_000_000.0, 1_000_001.0);
        }
        (self.scene.world.fog_start, self.scene.world.fog_end)
    }

    /// Embedded Studio views fill their editor pane. Player hosts retain their
    /// landscape composition when a window becomes portrait-ish; the UI pass
    /// still covers the full scene so touch controls can adapt.
    fn world_viewport(&self) -> (f32, f32, f32, f32) {
        const LANDSCAPE_ASPECT: f32 = 16.0 / 9.0;
        #[cfg(feature = "studio-ui")]
        if self.studio_viewport.is_some() {
            return self.output_viewport();
        }
        #[cfg(feature = "studio-ui")]
        let (x, y, width, height) = self.output_viewport();
        #[cfg(not(feature = "studio-ui"))]
        let (x, y, width, height) = (0.0, 0.0, self.width.max(1.0), self.height.max(1.0));
        let aspect = width / height;
        if aspect >= 1.25 {
            return (x, y, width, height);
        }
        let viewport_height = (width / LANDSCAPE_ASPECT).min(height);
        (
            x,
            y + (height - viewport_height) * 0.5,
            width,
            viewport_height,
        )
    }

    fn shadow_view_projection(&self, center: Vec3) -> Mat4 {
        // Keep one stable, player-centered orthographic cascade for the first
        // shadow milestone. It covers the playable maze and nearby dressing
        // on mobile/WebGL without requiring a second shadow cascade.
        Self::shadow_view_projection_for(center, self.scene.world.sun_direction)
    }

    fn shadow_view_projection_for(center: Vec3, sun_direction: [f32; 3]) -> Mat4 {
        let light_direction = shadow_light_direction(sun_direction);
        let up = if light_direction.dot(Vec3::Y).abs() > 0.92 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        // Keep the light view itself fixed. Only translate its orthographic
        // projection in light space, then snap that translation to whole shadow
        // texels. Rebuilding look_at around `center` would reintroduce swimming.
        let base_view = Mat4::look_at_rh(light_direction * 180.0, Vec3::ZERO, up);
        let light_space_center = base_view.transform_point3(center);
        let texel_world_size = 240.0 / crate::renderer::device::SHADOW_MAP_SIZE as f32;
        let snapped_center = Vec3::new(
            (light_space_center.x / texel_world_size).round() * texel_world_size,
            (light_space_center.y / texel_world_size).round() * texel_world_size,
            light_space_center.z,
        );
        // The translation must be derived only from the quantized center. Using
        // `snapped_center - light_space_center` would still track sub-texel
        // movement and merely hide the swimming behind a rounded target.
        let snapped_view =
            Mat4::from_translation(Vec3::new(-snapped_center.x, -snapped_center.y, 0.0))
                * base_view;
        Mat4::orthographic_rh(-120.0, 120.0, -120.0, 120.0, 0.1, 420.0) * snapped_view
    }

    #[cfg(feature = "studio-ui")]
    fn output_viewport(&self) -> (f32, f32, f32, f32) {
        if let Some([x, y, width, height]) = self.studio_viewport {
            return (x, y, width, height);
        }
        (0.0, 0.0, self.width.max(1.0), self.height.max(1.0))
    }

    fn build_static_vertices(&self) -> Vec<Vertex> {
        let mut mesh = Vec::with_capacity(16_384);
        let world = &self.scene.world;
        if !world.hide_default_ground
            && let Some(material) = world
                .ground_material
                .as_ref()
                .filter(|material| self.package_image_regions.contains_key(&material.image))
        {
            add_textured_cuboid(
                &mut mesh,
                Vec3::new(0.0, world.ground_y - 0.08, 0.0),
                Vec3::new(world.ground_size, 0.16, world.ground_size),
                material,
                self.package_image_regions[&material.image],
            );
        } else if !world.hide_default_ground {
            add_cuboid(
                &mut mesh,
                Vec3::new(0.0, world.ground_y - 0.08, 0.0),
                Vec3::new(world.ground_size, 0.16, world.ground_size),
                world.palette.ground,
            );
        }
        if !world.hide_default_ground {
            add_cuboid_outline(
                &mut mesh,
                Vec3::new(0.0, world.ground_y - 0.08, 0.0),
                Vec3::new(world.ground_size, 0.16, world.ground_size),
                0.035,
                faded(world.palette.ground_edge, 0.46),
            );
        }
        for block in &world.blocks {
            if let Some(material) = block
                .material
                .as_ref()
                .filter(|material| self.package_image_regions.contains_key(&material.image))
            {
                add_textured_cuboid(
                    &mut mesh,
                    Vec3::from_array(block.position),
                    Vec3::from_array(block.size),
                    material,
                    self.package_image_regions[&material.image],
                );
            } else {
                add_cuboid(
                    &mut mesh,
                    Vec3::from_array(block.position),
                    Vec3::from_array(block.size),
                    block.color,
                );
            }
            if block.outline {
                add_cuboid_outline(
                    &mut mesh,
                    Vec3::from_array(block.position),
                    Vec3::from_array(block.size),
                    0.025,
                    faded(world.palette.paper, 0.22),
                );
            }
        }
        for decoration in &world.decorations {
            // Asset-backed decorations are rendered by WorldMeshRegistry below;
            // they must not also become fallback procedural spheres here.
            if !decoration.kind.eq_ignore_ascii_case("mesh") {
                add_decoration(&mut mesh, decoration, world.palette);
            }
        }
        for ladder in &world.ladders {
            add_ladder(&mut mesh, ladder);
        }
        for billboard in &world.billboards {
            if let Some(&texture_bounds) = self.package_image_regions.get(&billboard.image) {
                add_billboard(&mut mesh, billboard, world.palette, texture_bounds);
            }
        }
        if world.show_grid {
            let divisions = world.grid_divisions.clamp(1, 128);
            let half = world.grid_size * 0.5;
            let grid_step = world.grid_size / divisions as f32;
            for index in 0..=divisions {
                let offset = -half + index as f32 * grid_step;
                add_cuboid(
                    &mut mesh,
                    Vec3::new(offset, world.ground_y + 0.015, 0.0),
                    Vec3::new(0.018, 0.025, world.grid_size),
                    faded(world.palette.grid, 0.34),
                );
                add_cuboid(
                    &mut mesh,
                    Vec3::new(0.0, world.ground_y + 0.016, offset),
                    Vec3::new(world.grid_size, 0.026, 0.018),
                    faded(world.palette.grid, 0.34),
                );
            }
        }
        mesh
    }

    fn build_dynamic_vertices(&mut self) -> Vec<Vertex> {
        let mut mesh = Vec::with_capacity(16_384);
        let world = &self.scene.world;
        if world.show_spawn_pad {
            add_spawn_pad(
                &mut mesh,
                Vec3::from_array(world.spawn),
                world.palette,
                self.scene.elapsed,
            );
        }
        #[cfg(debug_assertions)]
        add_floor_pixel_text(
            &mut mesh,
            super::DEBUG_GIT_SHA,
            Vec3::new(
                world.spawn[0],
                world.spawn[1] + 0.035,
                // Put the label toward the default camera, clear of the pad
                // and the player standing on it.
                world.spawn[2] + 5.5,
            ),
            9.0,
            world.palette.ink,
        );
        for (index, cloud) in world.clouds.iter().enumerate() {
            add_cloud(
                &mut mesh,
                cloud,
                index,
                world.palette.paper,
                self.scene.elapsed,
            );
        }
        for (index, pad) in world.pads.iter().enumerate() {
            add_launch_pad(
                &mut mesh,
                pad,
                self.scene.pad_seconds.get(index).copied().unwrap_or(0.0),
                world.palette,
                self.scene.elapsed,
                index,
            );
        }
        for (index, interaction) in world.interactions.iter().enumerate() {
            let template = interaction
                .visual
                .as_deref()
                .and_then(|id| world.effect_templates.get(id));
            let visual_state = self
                .scene
                .effect_states
                .get(&interaction.id)
                .map(String::as_str)
                .unwrap_or("default");
            super::effects::add_interaction(
                &mut mesh,
                interaction,
                template,
                visual_state,
                self.scene
                    .interaction_states
                    .get(index)
                    .copied()
                    .unwrap_or_default(),
                self.scene.elapsed,
                world.palette,
                self.scene.reduced_effects,
            );
        }
        for instance in &self.scene.effect_instances {
            let Some(template) = world.effect_templates.get(&instance.template) else {
                continue;
            };
            let age = (self.scene.elapsed - instance.started_at).max(0.0);
            if age > template.duration {
                continue;
            }
            super::effects::add_template(
                &mut mesh,
                template,
                Vec3::from_array(instance.position),
                world.palette.paper,
                "default",
                age,
                Some(age / template.duration),
                self.scene.reduced_effects,
            );
        }
        for sign in &world.signs {
            let text = if sign.text == "{{username}}" {
                &self.scene.username
            } else {
                &sign.text
            };
            add_pixel_text(
                &mut mesh,
                text,
                Vec3::from_array(sign.position),
                sign.yaw,
                sign.max_width,
                sign.color,
            );
        }
        for block in &self.scene.build_blocks {
            let size = if block.rotation % 2 == 0 {
                block.size
            } else {
                [block.size[2], block.size[1], block.size[0]]
            };
            let color = super::color(block.color);
            add_cuboid(
                &mut mesh,
                Vec3::from_array(block.position),
                Vec3::from_array(size),
                color,
            );
            add_cuboid_outline(
                &mut mesh,
                Vec3::from_array(block.position),
                Vec3::from_array(size),
                0.025,
                faded(world.palette.paper, 0.3),
            );
        }
        if self.character_render_mode == CharacterRenderMode::Legacy {
            // This is the complete rollback path: it uses the established
            // hard-cuboid avatar and legacy package colors, while preserving
            // the typed pose inputs supplied by the current engine.
            if self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE {
                super::add_legacy_avatar(
                    &mut mesh,
                    self.scene.player,
                    self.scene.player_style,
                    self.scene.world.palette.ink,
                );
            }
            for player in &self.scene.remote_players {
                super::add_legacy_avatar(
                    &mut mesh,
                    *player,
                    player.style,
                    self.scene.world.palette.ink,
                );
            }
            for (index, agent) in self.scene.agents.iter().enumerate() {
                let style = self
                    .scene
                    .npc_styles
                    .get(index % self.scene.npc_styles.len().max(1))
                    .copied()
                    .unwrap_or(self.scene.player_style);
                super::add_legacy_avatar(&mut mesh, *agent, style, self.scene.world.palette.ink);
            }
        }
        mesh
    }

    fn build_support_shadows(&self, view: Mat4, aspect: f32) -> Vec<Vertex> {
        let mut shadows = Vec::with_capacity(
            504 * (1 + self.scene.agents.len() + self.scene.remote_players.len()),
        );
        let mut add = |entity: RenderEntity| {
            if !entity.position.iter().all(|value| value.is_finite()) {
                return;
            }
            if !character_quality::is_visible(entity, view, aspect) {
                return;
            }
            let Some((height, mut alpha)) = support_receiver(&self.scene.world, entity) else {
                return;
            };
            let mut radius: f32 = 0.72;
            if let Some(block) = self.scene.world.blocks.iter().find(|block| {
                let [x, _, z] = block.position;
                let [sx, _, sz] = block.size;
                (entity.position[0] - x).abs() <= sx * 0.5
                    && (entity.position[2] - z).abs() <= sz * 0.5
                    && (height - (block.position[1] + block.size[1] * 0.5)).abs() < 0.08
            }) {
                let edge_x = (block.size[0] * 0.5 - (entity.position[0] - block.position[0]).abs())
                    .max(0.04);
                let edge_z = (block.size[2] * 0.5 - (entity.position[2] - block.position[2]).abs())
                    .max(0.04);
                radius = radius.min(edge_x.min(edge_z) * 0.88);
            }
            // Interpolated opacity keeps the contact shadow soft at every
            // camera distance, within the existing receiver/edge constraints.
            let height_gap = (entity.position[1] - height).max(0.0);
            alpha *= (radius / 0.72).clamp(0.15, 1.0) * (1.0 - height_gap * 0.28).clamp(0.35, 1.0);
            super::add_soft_support_shadow(
                &mut shadows,
                Vec3::new(entity.position[0], height + 0.011, entity.position[2]),
                radius,
                super::faded(self.scene.world.palette.ink, alpha),
            );
        };
        if self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE {
            add(self.scene.player);
        }
        for entity in &self.scene.remote_players {
            add(*entity);
        }
        for entity in &self.scene.agents {
            add(*entity);
        }
        shadows
    }

    pub(super) fn rebuild_static_vertices(&mut self) {
        let all = self.build_static_vertices();
        let mut vertices = Vec::with_capacity(all.len());
        self.static_translucent_vertices.clear();
        split_world_vertices(&all, &mut vertices, &mut self.static_translucent_vertices);
        if !self.ensure_static_vertex_capacity(vertices.len()) {
            log::error!(
                "static world geometry exceeds this GPU's maximum vertex-buffer size; skipping it"
            );
            self.static_vertex_count = 0;
            self.static_translucent_vertices.clear();
        } else {
            self.static_vertex_count = vertices.len();
        }
        if self.static_vertex_count > 0 {
            self.queue.write_buffer(
                &self.static_vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices),
            );
        }
        self.terrain_meshes.clear();
        let meshes = self.build_terrain_meshes();
        self.terrain_meshes = self.upload_terrain_meshes(meshes);
        self.world_meshes.rebuild_instances(
            &self.device,
            &self.scene.world.mesh_instances,
            &self.package_image_regions,
        );
    }

    fn ensure_static_vertex_capacity(&mut self, required: usize) -> bool {
        if required <= self.static_vertex_capacity {
            return true;
        }
        let Some(capacity) = vertex_capacity_for(required, self.device.limits().max_buffer_size)
        else {
            return false;
        };
        self.static_vertex_capacity = capacity;
        self.static_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.static_vertex_capacity);
        true
    }

    fn ensure_dynamic_vertex_capacity(&mut self, required: usize) -> bool {
        if required <= self.dynamic_vertex_capacity {
            return true;
        }
        let Some(capacity) = vertex_capacity_for(required, self.device.limits().max_buffer_size)
        else {
            return false;
        };
        self.dynamic_vertex_capacity = capacity;
        self.dynamic_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.dynamic_vertex_capacity);
        true
    }

    fn ensure_ui_vertex_capacity(&mut self, required: usize) -> bool {
        if required <= self.ui_vertex_capacity {
            return true;
        }
        let Some(capacity) = vertex_capacity_for(required, self.device.limits().max_buffer_size)
        else {
            return false;
        };
        self.ui_vertex_capacity = capacity;
        self.ui_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.ui_vertex_capacity);
        true
    }

    fn add_world_labels(
        &self,
        vertices: &mut Vec<Vertex>,
        view_projection: Mat4,
        camera_position: Vec3,
        world_viewport: (f32, f32, f32, f32),
    ) {
        let mut add = |entity: RenderEntity, name: &str| {
            let label_position = Vec3::from_array(entity.position)
                + Vec3::new(0.0, super::character::world_label_height(entity.body), 0.0);
            #[cfg(feature = "studio-ui")]
            let output_viewport = self.output_viewport();
            #[cfg(not(feature = "studio-ui"))]
            let output_viewport = (0.0, 0.0, self.width, self.height);
            let Some((x, y)) = project_world_label_to_ui(
                label_position,
                view_projection,
                world_viewport,
                output_viewport,
                (self.ui_frame.viewport.width, self.ui_frame.viewport.height),
            ) else {
                return;
            };
            let distance = (label_position - camera_position).length().max(1.0);
            let font_size = (190.0 / distance).clamp(12.0, 18.0);
            super::ui::add_world_label(vertices, &self.ui_frame, x, y, name, font_size);
        };

        if self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE && !self.avatar_preview_mode
        {
            add(self.scene.player, &self.scene.username);
        }
        for (index, player) in self.scene.remote_players.iter().enumerate() {
            let fallback = format!("PLAYER {}", index + 1);
            let name = self
                .scene
                .remote_names
                .get(index)
                .map(String::as_str)
                .unwrap_or(&fallback);
            add(*player, name);
        }
        for (index, agent) in self.scene.agents.iter().enumerate() {
            let name = format!("BOT {}", index + 1);
            add(*agent, &name);
        }
    }
}

fn project_world_label_to_ui(
    position: Vec3,
    view_projection: Mat4,
    world_viewport: (f32, f32, f32, f32),
    output_viewport: (f32, f32, f32, f32),
    ui_size: (f32, f32),
) -> Option<(f32, f32)> {
    if output_viewport.2 <= 0.0 || output_viewport.3 <= 0.0 || ui_size.0 <= 0.0 || ui_size.1 <= 0.0
    {
        return None;
    }
    let clip = view_projection * position.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.01 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite()
        || ndc.z < 0.0
        || ndc.z > 1.0
        || !(-1.0..=1.0).contains(&ndc.x)
        || !(-1.0..=1.0).contains(&ndc.y)
    {
        return None;
    }

    let surface_x = world_viewport.0 + (ndc.x + 1.0) * 0.5 * world_viewport.2;
    let surface_y = world_viewport.1 + (1.0 - (ndc.y + 1.0) * 0.5) * world_viewport.3;
    Some((
        (surface_x - output_viewport.0) * ui_size.0 / output_viewport.2,
        (surface_y - output_viewport.1) * ui_size.1 / output_viewport.3,
    ))
}

fn support_receiver(_world: &super::RenderWorld, entity: RenderEntity) -> Option<(f32, f32)> {
    match entity.support {
        crate::types::CharacterSupport::Grounded { height } if height.is_finite() => {
            Some((height, 0.18))
        }
        crate::types::CharacterSupport::Unknown
            if entity.position[1].is_finite() && entity.position[1].abs() <= 0.08 =>
        {
            // Legacy remotes do not report support. The ground fallback is
            // intentionally faint and is omitted at any raised height.
            Some((0.0, 0.08))
        }
        _ => None,
    }
}

pub(super) fn split_world_vertices(
    source: &[Vertex],
    opaque: &mut Vec<Vertex>,
    translucent: &mut Vec<Vertex>,
) {
    for triangle in source.chunks_exact(3) {
        if triangle.iter().all(|vertex| vertex.color[3] >= 1.0) {
            opaque.extend_from_slice(triangle);
        } else {
            translucent.extend_from_slice(triangle);
        }
    }
}

pub(super) fn sort_translucent(vertices: &mut [Vertex], camera: Vec3, target: Vec3) {
    let forward = (target - camera).normalize_or_zero();
    let depth = |triangle: &[Vertex; 3]| {
        triangle
            .iter()
            .map(|v| (Vec3::from_array(v.position) - camera).dot(forward))
            .sum::<f32>()
    };
    vertices
        .as_chunks_mut::<3>()
        .0
        .sort_unstable_by(|a, b| depth(b).total_cmp(&depth(a)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain_vertex(position: [f32; 3]) -> TerrainVertex {
        TerrainVertex {
            position,
            normal: [0.0, 1.0, 0.0],
            material: 1,
        }
    }

    #[test]
    fn terrain_chunk_mesh_reuses_vertices_and_keeps_triangle_indices() {
        let a = terrain_vertex([0.0, 0.0, 0.0]);
        let b = terrain_vertex([1.0, 0.0, 0.0]);
        let c = terrain_vertex([0.0, 0.0, 1.0]);
        let d = terrain_vertex([1.0, 0.0, 1.0]);
        let mut mesh = TerrainMeshBuilder::default();

        mesh.push_triangle([a, b, c], true);
        mesh.push_triangle([b, d, c], true);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices, [0, 1, 2, 1, 3, 2]);
    }

    #[test]
    fn terrain_surface_is_built_as_indexed_chunk_meshes() {
        let definition: crate::terrain::TerrainDefinition = serde_json::from_str(
            r#"{
                "cellSize":0.5,
                "operations":[{
                    "shape":"block","operation":"fill","position":[0,0,0],
                    "size":[8,8,8],"material":"grass"
                }]
            }"#,
        )
        .unwrap();
        let terrain = crate::terrain::TerrainGrid::build(&definition)
            .unwrap()
            .unwrap();
        let mut chunks = BTreeMap::<[i32; 3], TerrainMeshBuilder>::new();
        terrain.for_each_chunk_triangle(|coordinate, triangle| {
            chunks
                .entry(coordinate)
                .or_default()
                .push_triangle(triangle, true);
        });

        let indexed_vertices = chunks
            .values()
            .map(|chunk| chunk.vertices.len())
            .sum::<usize>();
        let index_count = chunks
            .values()
            .map(|chunk| chunk.indices.len())
            .sum::<usize>();
        assert!(!chunks.is_empty());
        assert!(index_count > 0 && index_count % 3 == 0);
        assert!(indexed_vertices < index_count);
        let indexed_bytes = indexed_vertices * std::mem::size_of::<Vertex>()
            + index_count * std::mem::size_of::<u32>();
        let triangle_soup_bytes = index_count * std::mem::size_of::<Vertex>();
        assert!(indexed_bytes < triangle_soup_bytes);
    }

    #[test]
    fn vertex_capacity_does_not_round_past_device_limit() {
        let max_buffer_size = 256 * 1024 * 1024;
        // Maze 101 currently emits this many terrain vertices. Rounding its
        // capacity to 2^22 used to request 285,212,672 bytes and panic.
        let maze_vertices = 2_511_408;
        assert_eq!(
            vertex_capacity_for(maze_vertices, max_buffer_size),
            Some(maze_vertices)
        );
        assert_eq!(maze_vertices * std::mem::size_of::<Vertex>(), 170_775_744);
        assert_eq!(vertex_capacity_for(100, max_buffer_size), Some(128));
        assert_eq!(vertex_capacity_for(4_000_000, max_buffer_size), None);
    }

    #[test]
    fn world_labels_convert_render_pixels_to_ui_points_and_cull_offscreen() {
        let viewport = (0.0, 0.0, 2_000.0, 1_000.0);
        let center = project_world_label_to_ui(
            Vec3::ZERO,
            Mat4::IDENTITY,
            viewport,
            viewport,
            (1_000.0, 500.0),
        );
        assert_eq!(center, Some((500.0, 250.0)));
        assert!(
            project_world_label_to_ui(
                Vec3::new(1.01, 0.0, 0.0),
                Mat4::IDENTITY,
                viewport,
                viewport,
                (1_000.0, 500.0),
            )
            .is_none()
        );
    }

    #[test]
    fn world_labels_are_local_to_an_embedded_output_viewport() {
        let output = (240.0, 80.0, 1_000.0, 600.0);
        assert_eq!(
            project_world_label_to_ui(Vec3::ZERO, Mat4::IDENTITY, output, output, (1_000.0, 600.0),),
            Some((500.0, 300.0))
        );
    }

    #[test]
    fn world_alpha_is_separated_and_sorted_back_to_front() {
        let triangle = |z, alpha| {
            [Vertex {
                position: [0.0, 0.0, z],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, alpha],
                tex_coords: [0.0; 2],
                image_invert: 0.0,
                texture_bounds: [0.0, 0.0, 1.0, 1.0],
            }; 3]
        };
        let mut source = Vec::new();
        source.extend(triangle(-2.0, 0.4));
        source.extend(triangle(-1.0, 1.0));
        source.extend(triangle(-5.0, 0.5));
        let (mut opaque, mut alpha) = (Vec::new(), Vec::new());
        split_world_vertices(&source, &mut opaque, &mut alpha);
        sort_translucent(&mut alpha, Vec3::ZERO, -Vec3::Z);
        assert_eq!(opaque.len(), 3);
        assert_eq!(alpha.len(), 6);
        assert_eq!(alpha[0].position[2], -5.0);
    }

    #[test]
    fn support_shadow_fallback_is_conservative() {
        let mut entity = RenderEntity::default();
        entity.support = crate::types::CharacterSupport::Grounded { height: 2.0 };
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            Some((2.0, 0.18))
        );
        entity.support = crate::types::CharacterSupport::Airborne;
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            None
        );
        entity.support = crate::types::CharacterSupport::Unknown;
        entity.position[1] = 2.0;
        assert_eq!(
            support_receiver(&crate::renderer::RenderWorld::default(), entity),
            None
        );
    }

    #[test]
    fn shadow_camera_is_above_the_maze_and_light_space_center_snaps() {
        let sun = [-0.45, -0.82, 0.32];
        let light = shadow_light_direction(sun);
        let eye = Vec3::ZERO + light * 180.0;
        assert!(eye.y > 0.0, "shadow camera must sit toward the sun");

        let first = Renderer::shadow_view_projection_for(Vec3::ZERO, sun);
        let texel = 240.0 / crate::renderer::device::SHADOW_MAP_SIZE as f32;
        let view = Mat4::look_at_rh(light * 180.0, Vec3::ZERO, Vec3::Y);
        let sub_texel_world =
            view.inverse()
                .transform_vector3(Vec3::new(texel * 0.40, texel * 0.30, 0.0));
        let sub_texel = Renderer::shadow_view_projection_for(sub_texel_world, sun);
        assert_eq!(
            first, sub_texel,
            "sub-texel motion must not move the shadow map"
        );

        let light_space_step = view
            .inverse()
            .transform_vector3(Vec3::new(texel * 0.60, 0.0, 0.0));
        let moved = Renderer::shadow_view_projection_for(light_space_step, sun);
        assert_ne!(
            first, moved,
            "crossing a shadow texel must update the projection"
        );
    }
}
