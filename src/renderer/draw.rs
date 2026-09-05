use super::character_material::CharacterPass;
use super::character_quality;
use glam::{Mat4, Vec3};

use super::{
    Globals, RenderEntity, Renderer, Vertex, add_cloud, add_cuboid, add_cuboid_outline, add_cylinder, add_launch_pad,
    add_pixel_text, add_spawn_pad, faded,
};

impl Renderer {
    pub fn draw(&mut self) {
        let player = Vec3::from_array(self.scene.player.position);
        let [yaw, pitch, distance] = self.scene.camera;
        let body = self.scene.player.body;
        let (camera_position, target) = if distance <= 0.75 {
            let camera_position = player + super::character::camera_anchor(body);
            let look_direction = Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                -yaw.cos() * pitch.cos(),
            );
            (camera_position, camera_position + look_direction)
        } else {
            let target = player + super::character::camera_target(body);
            let horizontal_distance = distance * pitch.cos();
            let camera_position = target
                + Vec3::new(
                    yaw.sin() * horizontal_distance,
                    (distance * pitch.sin()).clamp(-2.0, distance),
                    yaw.cos() * horizontal_distance,
                );
            (camera_position, target)
        };
        let world_viewport = self.world_viewport();
        let view = Mat4::look_at_rh(camera_position, target, Vec3::Y);
        let view_projection = Mat4::perspective_rh(
            62.0_f32.to_radians(),
            (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1),
            0.05,
            240.0,
        ) * view;
        let globals = Globals {
            view_projection: view_projection.to_cols_array_2d(),
            camera_position: camera_position.extend(1.0).to_array(),
            sun_direction: Vec3::new(-0.45, -0.82, 0.32)
                .normalize()
                .extend(0.0)
                .to_array(),
            fog_color: self.scene.world.palette.sky,
        };
        let dynamic_vertices = self.build_dynamic_vertices();
        let viewport_aspect = (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1);
        let shadow_vertices = self.build_support_shadows(view, viewport_aspect);
        self.opaque_vertices.clear();
        self.translucent_vertices.clear();
        self.translucent_vertices
            .extend_from_slice(&self.static_translucent_vertices);
        split_world_vertices(
            &dynamic_vertices,
            &mut self.opaque_vertices,
            &mut self.translucent_vertices,
        );
        sort_translucent(&mut self.translucent_vertices, camera_position, target);
        let dynamic_count = self.opaque_vertices.len()
            + shadow_vertices.len()
            + self.translucent_vertices.len();
        self.characters.begin();
        let character_ink = self.scene.world.palette.ink;
        let reduced_effects = self.scene.reduced_effects;
        let lods = &mut self.scene.lods;
        let mut add_character = |characters: &mut super::character_gpu::CharacterRenderer,
                             entity: RenderEntity,
                             style: super::AvatarStyle,
                             rank: usize,
                             reduced_effects: bool| {
            let aspect = (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1);
            let Some(lod) = character_quality::is_visible(entity, view, aspect).then(|| {
                character_quality::select_lod(
                    character_quality::projected_height(entity, view, world_viewport.3),
                    lods.get(&entity.key).copied(),
                )
            }) else {
                characters.stats.culled += 1;
                return;
            };
            lods.insert(entity.key, lod);
            characters.add_with_quality(
                entity,
                style,
                character_ink,
                lod,
                rank,
                reduced_effects,
            );
        };
        // Local player first gives deterministic priority if a development
        // caller supplies more than the bounded render-only crowd capacity.
        if self.scene.camera[2] > 0.75 {
            add_character(
                &mut self.characters,
                self.scene.player,
                self.scene.player_style,
                0,
                reduced_effects,
            );
        }
        for (index, player) in self.scene.remote_players.iter().enumerate() {
            add_character(
                &mut self.characters,
                *player,
                self.scene.player_style,
                index + 1,
                reduced_effects,
            );
        }
        for (index, agent) in self.scene.agents.iter().enumerate() {
            let style = self
                .scene
                .npc_styles
                .get(index % self.scene.npc_styles.len().max(1))
                .copied()
                .unwrap_or(self.scene.player_style);
            add_character(
                &mut self.characters,
                *agent,
                style,
                self.scene.remote_players.len() + index + 1,
                reduced_effects,
            );
        }
        self.characters.upload(&self.queue);
        let ui_vertices = super::ui::build_ui_vertices(&self.ui_frame);
        self.ensure_dynamic_vertex_capacity(dynamic_count);
        self.ensure_ui_vertex_capacity(ui_vertices.len());
        if !self.opaque_vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.opaque_vertices),
            );
        }
        if !shadow_vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                size_of_val(self.opaque_vertices.as_slice()) as u64,
                bytemuck::cast_slice(&shadow_vertices),
            );
        }
        if !self.translucent_vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                (size_of_val(self.opaque_vertices.as_slice())
                    + size_of_val(shadow_vertices.as_slice())) as u64,
                bytemuck::cast_slice(&self.translucent_vertices),
            );
        }
        if !ui_vertices.is_empty() {
            self.queue.write_buffer(
                &self.ui_vertex_buffer,
                0,
                bytemuck::cast_slice(&ui_vertices),
            );
        }
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.resize(self.width, self.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cubacadabra frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra world pass"),
                color_attachments: &[Some(self.targets.attachment(wgpu::Color {
                    r: self.scene.world.palette.sky[0] as f64,
                    g: self.scene.world.palette.sky[1] as f64,
                    b: self.scene.world.palette.sky[2] as f64,
                    a: 1.0,
                }))],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_viewport(
                world_viewport.0,
                world_viewport.1,
                world_viewport.2,
                world_viewport.3,
                0.0,
                1.0,
            );
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            if self.static_vertex_count > 0 {
                pass.set_vertex_buffer(0, self.static_vertex_buffer.slice(..));
                pass.draw(0..self.static_vertex_count as u32, 0..1);
            }
            if !self.opaque_vertices.is_empty() {
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(..));
                pass.draw(0..self.opaque_vertices.len() as u32, 0..1);
            }
            if !shadow_vertices.is_empty() {
                pass.set_pipeline(&self.translucent_pipeline);
                let start = size_of_val(self.opaque_vertices.as_slice()) as u64;
                let end = start + size_of_val(shadow_vertices.as_slice()) as u64;
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(start..end));
                pass.draw(0..shadow_vertices.len() as u32, 0..1);
            }
            self.characters.draw(&mut pass, CharacterPass::Opaque);
            self.characters.draw(&mut pass, CharacterPass::Face);
            self.characters.draw(&mut pass, CharacterPass::Effect);
            if !self.translucent_vertices.is_empty() {
                pass.set_pipeline(&self.translucent_pipeline);
                let start = (size_of_val(self.opaque_vertices.as_slice())
                    + size_of_val(shadow_vertices.as_slice())) as u64;
                let end = start + size_of_val(self.translucent_vertices.as_slice()) as u64;
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(start..end));
                pass.draw(0..self.translucent_vertices.len() as u32, 0..1);
            }
        }
        if !ui_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra UI pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_bind_group(0, &self.ui_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, self.ui_vertex_buffer.slice(..));
            pass.draw(0..ui_vertices.len() as u32, 0..1);
        }
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
}

/// Keep the 3D world in its normal landscape composition when a window
    /// becomes portrait-ish. The UI pass still covers the full scene so touch
    /// controls can adapt to the actual window dimensions.
    fn world_viewport(&self) -> (f32, f32, f32, f32) {
        const LANDSCAPE_ASPECT: f32 = 16.0 / 9.0;
        let width = self.width.max(1.0);
        let height = self.height.max(1.0);
        let aspect = width / height;
        if aspect >= 1.25 {
            return (0.0, 0.0, width, height);
        }
        let viewport_height = (width / LANDSCAPE_ASPECT).min(height);
        (
            0.0,
            (height - viewport_height) * 0.5,
            width,
            viewport_height,
        )
    }

    fn build_static_vertices(&self) -> Vec<Vertex> {
        let mut mesh = Vec::with_capacity(16_384);
        let world = &self.scene.world;
        add_cuboid(
            &mut mesh,
            Vec3::new(0.0, -0.08, 0.0),
            Vec3::new(world.ground_size, 0.16, world.ground_size),
            world.palette.ground,
        );
        add_cuboid_outline(
            &mut mesh,
            Vec3::new(0.0, -0.08, 0.0),
            Vec3::new(world.ground_size, 0.16, world.ground_size),
            0.035,
            faded(world.palette.ground_edge, 0.46),
        );
        for block in &world.blocks {
            add_cuboid(
                &mut mesh,
                Vec3::from_array(block.position),
                Vec3::from_array(block.size),
                block.color,
            );
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
        let divisions = world.grid_divisions.clamp(1, 128);
        let half = world.grid_size * 0.5;
        let grid_step = world.grid_size / divisions as f32;
        for index in 0..=divisions {
            let offset = -half + index as f32 * grid_step;
            add_cuboid(
                &mut mesh,
                Vec3::new(offset, 0.015, 0.0),
                Vec3::new(0.018, 0.025, world.grid_size),
                faded(world.palette.grid, 0.34),
            );
            add_cuboid(
                &mut mesh,
                Vec3::new(0.0, 0.016, offset),
                Vec3::new(world.grid_size, 0.026, 0.018),
                faded(world.palette.grid, 0.34),
            );
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
        mesh
    }

    fn build_support_shadows(&self, view: Mat4, aspect: f32) -> Vec<Vertex> {
        let mut shadows = Vec::with_capacity(24 * (1 + self.scene.agents.len() + self.scene.remote_players.len()));
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
                let edge_x = (block.size[0] * 0.5 - (entity.position[0] - block.position[0]).abs()).max(0.04);
                let edge_z = (block.size[2] * 0.5 - (entity.position[2] - block.position[2]).abs()).max(0.04);
                radius = radius.min(edge_x.min(edge_z) * 0.88);
            }
            // Two receiver-aligned layers provide a cheap soft/contact shadow:
            // the wider layer fades into the receiver and the smaller layer
            // keeps the feet grounded. Both remain depth-tested and never
            // write depth through the translucent world pipeline.
            let height_gap = (entity.position[1] - height).max(0.0);
            alpha *= (radius / 0.72).clamp(0.15, 1.0)
                * (1.0 - height_gap * 0.28).clamp(0.35, 1.0);
            add_cylinder(
                &mut shadows,
                Vec3::new(entity.position[0], height + 0.011, entity.position[2]),
                radius * 1.10,
                0.012,
                super::faded(self.scene.world.palette.ink, alpha * 0.32),
            );
            add_cylinder(
                &mut shadows,
                Vec3::new(entity.position[0], height + 0.015, entity.position[2]),
                radius * 0.78,
                0.014,
                super::faded(self.scene.world.palette.ink, alpha * 0.58),
            );
        };
        if self.scene.camera[2] > 0.75 {
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
        self.ensure_static_vertex_capacity(vertices.len());
        self.static_vertex_count = vertices.len();
        if !vertices.is_empty() {
            self.queue.write_buffer(
                &self.static_vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices),
            );
        }
    }

    fn ensure_static_vertex_capacity(&mut self, required: usize) {
        if required <= self.static_vertex_capacity {
            return;
        }
        self.static_vertex_capacity = required.next_power_of_two();
        self.static_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.static_vertex_capacity);
    }

    fn ensure_dynamic_vertex_capacity(&mut self, required: usize) {
        if required <= self.dynamic_vertex_capacity {
            return;
        }
        self.dynamic_vertex_capacity = required.next_power_of_two();
        self.dynamic_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.dynamic_vertex_capacity);
    }

    fn ensure_ui_vertex_capacity(&mut self, required: usize) {
        if required <= self.ui_vertex_capacity {
            return;
        }
        self.ui_vertex_capacity = required.next_power_of_two();
        self.ui_vertex_buffer =
            super::device::create_vertex_buffer(&self.device, self.ui_vertex_capacity);
    }
}

fn support_receiver(
    _world: &super::RenderWorld,
    entity: RenderEntity,
) -> Option<(f32, f32)> {
    match entity.support {
        crate::types::CharacterSupport::Grounded { height } if height.is_finite() => {
            Some((height, 0.18))
        }
        crate::types::CharacterSupport::Unknown
            if entity.position[1].is_finite() && entity.position[1].abs() <= 0.08 => {
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
    #[test]
    fn world_alpha_is_separated_and_sorted_back_to_front() {
        let triangle = |z, alpha| {
            [Vertex {
                position: [0.0, 0.0, z],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, alpha],
                tex_coords: [0.0; 2],
                image_invert: 0.0,
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
        assert_eq!(support_receiver(&crate::renderer::RenderWorld::default(), entity), Some((2.0, 0.18)));
        entity.support = crate::types::CharacterSupport::Airborne;
        assert_eq!(support_receiver(&crate::renderer::RenderWorld::default(), entity), None);
        entity.support = crate::types::CharacterSupport::Unknown;
        entity.position[1] = 2.0;
        assert_eq!(support_receiver(&crate::renderer::RenderWorld::default(), entity), None);
    }
}
