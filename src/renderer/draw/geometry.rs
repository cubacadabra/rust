use super::*;

impl super::super::Renderer {
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

    pub(super) fn build_dynamic_vertices(&mut self) -> Vec<Vertex> {
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
            super::super::DEBUG_GIT_SHA,
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
            super::super::effects::add_interaction(
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
            super::super::effects::add_template(
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
            let color = super::super::color(block.color);
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
            #[cfg(feature = "studio-ui")]
            let runtime_players_visible = !self.studio_edit_mode;
            #[cfg(not(feature = "studio-ui"))]
            let runtime_players_visible = true;
            // This is the complete rollback path: it uses the established
            // hard-cuboid avatar and legacy package colors, while preserving
            // the typed pose inputs supplied by the current engine.
            if runtime_players_visible
                && self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE
            {
                super::super::add_legacy_avatar(
                    &mut mesh,
                    self.scene.player,
                    self.scene.player_style,
                    self.scene.world.palette.ink,
                );
            }
            if runtime_players_visible {
                for player in &self.scene.remote_players {
                    super::super::add_legacy_avatar(
                        &mut mesh,
                        *player,
                        player.style,
                        self.scene.world.palette.ink,
                    );
                }
            }
            for (index, agent) in self.scene.agents.iter().enumerate() {
                let style = self
                    .scene
                    .npc_styles
                    .get(index % self.scene.npc_styles.len().max(1))
                    .copied()
                    .unwrap_or(self.scene.player_style);
                super::super::add_legacy_avatar(
                    &mut mesh,
                    *agent,
                    style,
                    self.scene.world.palette.ink,
                );
            }
        }
        mesh
    }

    pub(super) fn build_support_shadows(&self, view: Mat4, aspect: f32) -> Vec<Vertex> {
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
            super::super::add_soft_support_shadow(
                &mut shadows,
                Vec3::new(entity.position[0], height + 0.011, entity.position[2]),
                radius,
                super::super::faded(self.scene.world.palette.ink, alpha),
            );
        };
        #[cfg(feature = "studio-ui")]
        let runtime_players_visible = !self.studio_edit_mode;
        #[cfg(not(feature = "studio-ui"))]
        let runtime_players_visible = true;
        if runtime_players_visible && self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE {
            add(self.scene.player);
        }
        if runtime_players_visible {
            for entity in &self.scene.remote_players {
                add(*entity);
            }
        }
        for entity in &self.scene.agents {
            add(*entity);
        }
        shadows
    }

    pub(crate) fn rebuild_static_vertices(&mut self) {
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
            super::super::device::create_vertex_buffer(&self.device, self.static_vertex_capacity);
        true
    }

    pub(super) fn ensure_dynamic_vertex_capacity(&mut self, required: usize) -> bool {
        if required <= self.dynamic_vertex_capacity {
            return true;
        }
        let Some(capacity) = vertex_capacity_for(required, self.device.limits().max_buffer_size)
        else {
            return false;
        };
        self.dynamic_vertex_capacity = capacity;
        self.dynamic_vertex_buffer =
            super::super::device::create_vertex_buffer(&self.device, self.dynamic_vertex_capacity);
        true
    }

    pub(super) fn ensure_ui_vertex_capacity(&mut self, required: usize) -> bool {
        if required <= self.ui_vertex_capacity {
            return true;
        }
        let Some(capacity) = vertex_capacity_for(required, self.device.limits().max_buffer_size)
        else {
            return false;
        };
        self.ui_vertex_capacity = capacity;
        self.ui_vertex_buffer =
            super::super::device::create_vertex_buffer(&self.device, self.ui_vertex_capacity);
        true
    }

    pub(super) fn add_world_labels(
        &self,
        vertices: &mut Vec<Vertex>,
        view_projection: Mat4,
        camera_position: Vec3,
        world_viewport: (f32, f32, f32, f32),
    ) {
        let mut add = |entity: RenderEntity, name: &str| {
            let label_position = Vec3::from_array(entity.position)
                + Vec3::new(
                    0.0,
                    super::super::character::world_label_height(entity.body),
                    0.0,
                );
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
            super::super::ui::add_world_label(vertices, &self.ui_frame, x, y, name, font_size);
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
