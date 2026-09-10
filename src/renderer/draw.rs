use super::character_material::CharacterPass;
use super::character_quality;
use glam::{Mat4, Vec3};

use super::CharacterRenderMode;
#[cfg(debug_assertions)]
use super::add_floor_pixel_text;
use super::{
    Globals, RenderEntity, Renderer, Vertex, add_billboard, add_cloud, add_cuboid,
    add_cuboid_outline, add_ladder, add_launch_pad, add_pixel_text, add_spawn_pad,
    add_textured_cuboid, faded,
};

impl Renderer {
    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_overlay_format(&self) -> wgpu::TextureFormat {
        super::targets::SCENE_FORMAT
    }

    pub fn draw(&mut self) {
        let Some((frame, mut encoder, view)) = self.encode_frame() else {
            return;
        };
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn draw_with_overlay<F>(&mut self, overlay: F)
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        let Some((frame, mut encoder, view)) = self.encode_frame() else {
            return;
        };
        overlay(&self.device, &self.queue, &mut encoder, &self.targets.color);
        self.presenter.draw(&mut encoder, &self.targets, &view);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn encode_frame(
        &mut self,
    ) -> Option<(
        wgpu::SurfaceTexture,
        wgpu::CommandEncoder,
        wgpu::TextureView,
    )> {
        let player = Vec3::from_array(self.scene.player.position);
        let [yaw, pitch, distance] = self.scene.camera;
        let body = self.scene.player.body;
        let (camera_position, target) = super::camera::orbit(player, body, yaw, pitch, distance);
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
        let shadow_vertices = if self.character_render_mode == CharacterRenderMode::Magic {
            self.build_support_shadows(view, viewport_aspect)
        } else {
            Vec::new()
        };
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
        let dynamic_count =
            self.opaque_vertices.len() + shadow_vertices.len() + self.translucent_vertices.len();
        let magic_mode = self.character_render_mode == CharacterRenderMode::Magic;
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
            let morph_assets = self
                .scene
                .morph_assets
                .get(&entity.key)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            characters.add_with_quality(
                entity,
                style,
                character_ink,
                lod,
                rank,
                reduced_effects,
                morph_assets,
            );
        };
        if magic_mode {
            // Local player first gives deterministic priority if a development
            // caller supplies more than the bounded render-only crowd capacity.
            if self.scene.camera[2] > 0.75 {
                let mut local = self.scene.player;
                local.camera_fade = super::camera::fade(distance);
                add_character(&mut self.characters, local, local.style, 0, reduced_effects);
            }
            for (index, player) in self.scene.remote_players.iter().enumerate() {
                add_character(
                    &mut self.characters,
                    *player,
                    player.style,
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
        }
        self.characters.upload(&self.queue);
        let mut ui_vertices = Vec::new();
        self.add_world_labels(
            &mut ui_vertices,
            view_projection,
            camera_position,
            world_viewport,
        );
        ui_vertices.extend(super::ui::build_ui_vertices(&self.ui_frame));
        #[cfg(target_os = "android")]
        if !super::device::ANDROID_FIRST_FRAME_REPORTED
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            super::device::android_log(format!(
                "Android first frame camera={:?} world_vertices={} \
                 ui_vertices={} characters={} instances={} character_draws={} culled={}",
                self.scene.camera,
                dynamic_vertices.len() + self.static_vertex_count,
                ui_vertices.len(),
                self.characters.stats.characters,
                self.characters.stats.instances,
                self.characters.stats.draws,
                self.characters.stats.culled,
            ));
        }
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
                #[cfg(target_os = "android")]
                if !super::device::ANDROID_SURFACE_WARNING_REPORTED
                    .swap(true, std::sync::atomic::Ordering::Relaxed)
                {
                    super::device::android_log(
                        "Android surface became outdated or lost; reconfiguring",
                    );
                }
                self.resize(self.width, self.height);
                return None;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                #[cfg(target_os = "android")]
                if !super::device::ANDROID_SURFACE_WARNING_REPORTED
                    .swap(true, std::sync::atomic::Ordering::Relaxed)
                {
                    super::device::android_log(
                        "Android surface frame unavailable (timeout, occluded, or validation)",
                    );
                }
                return None;
            }
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
            pass.set_bind_group(1, &self.world_texture_bind_group, &[]);
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
            if magic_mode {
                self.characters.draw(&mut pass, CharacterPass::Opaque);
                self.characters.draw(&mut pass, CharacterPass::Face);
                self.characters.draw(&mut pass, CharacterPass::Effect);
            }
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
            #[cfg(feature = "studio-ui")]
            if let Some([x, y, width, height]) = self.studio_viewport {
                pass.set_viewport(x, y, width, height, 0.0, 1.0);
                pass.set_scissor_rect(
                    x.max(0.0) as u32,
                    y.max(0.0) as u32,
                    width.max(1.0) as u32,
                    height.max(1.0) as u32,
                );
            }
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_bind_group(0, &self.ui_texture_bind_group, &[]);
            pass.set_vertex_buffer(0, self.ui_vertex_buffer.slice(..));
            pass.draw(0..ui_vertices.len() as u32, 0..1);
        }
        Some((frame, encoder, view))
    }

    /// Keep the 3D world in its normal landscape composition when a window
    /// becomes portrait-ish. The UI pass still covers the full scene so touch
    /// controls can adapt to the actual window dimensions.
    fn world_viewport(&self) -> (f32, f32, f32, f32) {
        const LANDSCAPE_ASPECT: f32 = 16.0 / 9.0;
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
        if let Some(material) = world
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
        } else {
            add_cuboid(
                &mut mesh,
                Vec3::new(0.0, world.ground_y - 0.08, 0.0),
                Vec3::new(world.ground_size, 0.16, world.ground_size),
                world.palette.ground,
            );
        }
        add_cuboid_outline(
            &mut mesh,
            Vec3::new(0.0, world.ground_y - 0.08, 0.0),
            Vec3::new(world.ground_size, 0.16, world.ground_size),
            0.035,
            faded(world.palette.ground_edge, 0.46),
        );
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
            if self.scene.camera[2] > 0.75 {
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

        if self.scene.camera[2] > 0.75 {
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
}
