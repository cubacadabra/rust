use super::{
    CharacterPass, CharacterRenderMode, Globals, RenderEntity, character_quality, screen_sun,
    sort_translucent, split_world_vertices,
};
use glam::{Mat4, Vec3};
use std::mem::size_of_val;

impl super::super::Renderer {
    pub(crate) fn encode_frame(
        &mut self,
        offscreen: bool,
    ) -> Option<(
        Option<wgpu::SurfaceTexture>,
        wgpu::CommandEncoder,
        wgpu::TextureView,
    )> {
        let distance = self.scene.camera[2];
        let world_viewport = self.world_viewport();
        let aspect = (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1);
        let (camera_position, target) = self.camera_view(aspect);
        let view = Mat4::look_at_rh(camera_position, target, Vec3::Y);
        let view_projection = Mat4::perspective_rh(
            62.0_f32.to_radians(),
            aspect,
            0.05,
            self.camera_far_plane(camera_position, target),
        ) * view;
        // Authored gameplay fog is useful for play, but it can hide a whole
        // subject when Studio's Overview/Showcase camera backs away to fit
        // presentationBounds. Review cameras are diagnostic views, so keep
        // them clear without changing the package's gameplay atmosphere.
        let (fog_start, fog_end) = self.review_fog_range();
        let (sky_sun, post_sun) = screen_sun(
            view_projection,
            camera_position,
            self.scene.world.sun_direction,
            world_viewport,
            (self.width, self.height),
        );
        let globals = Globals {
            view_projection: view_projection.to_cols_array_2d(),
            camera_position: camera_position.extend(1.0).to_array(),
            sun_direction: Vec3::from_array(self.scene.world.sun_direction)
                .normalize()
                .extend(0.0)
                .to_array(),
            fog_color: self.scene.world.palette.sky,
            // The post-process applies color correction once after the sky,
            // terrain, meshes, characters, and fog share one image.
            color_grade: [1.0, 1.0, 1.0, 0.0],
            atmosphere: [fog_start, fog_end, 0.0, 0.0],
            lighting: [
                self.scene.world.outdoor_ambient[0],
                self.scene.world.outdoor_ambient[1],
                self.scene.world.outdoor_ambient[2],
                self.scene.world.sun_brightness,
            ],
        };
        let sky = self.scene.world.palette.sky;
        let sky_globals = super::super::SkyGlobals {
            horizon: sky,
            zenith: [sky[0] * 0.68, sky[1] * 0.82, (sky[2] * 1.04).min(1.0), 1.0],
            viewport: [
                world_viewport.0,
                world_viewport.1,
                world_viewport.2,
                world_viewport.3,
            ],
            sun: sky_sun,
            clouds: [self.scene.elapsed, 0.0, 0.82, 0.0],
        };
        let shadow_view_projection = self.shadow_view_projection(target);
        let shadow_globals = super::super::ShadowGlobals {
            view_projection: shadow_view_projection.to_cols_array_2d(),
            texel_size: [
                1.0 / super::super::device::SHADOW_MAP_SIZE as f32,
                1.0 / super::super::device::SHADOW_MAP_SIZE as f32,
                1.0 + self.scene.world.shadow_softness * 1.5,
                0.0,
            ],
        };
        let post_globals = super::super::PostGlobals {
            color_correction: [
                self.scene.world.color_correction[0],
                self.scene.world.color_correction[1],
                self.scene.world.color_correction[2],
                0.0,
            ],
            sun_rays: [
                post_sun[0],
                post_sun[1],
                self.scene.world.sun_rays_intensity * post_sun[2],
                self.scene.world.sun_rays_spread,
            ],
        };
        let dynamic_vertices = self.build_dynamic_vertices();
        let viewport_aspect = (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1);
        let mut shadow_vertices = if self.character_render_mode == CharacterRenderMode::Magic {
            self.build_support_shadows(view, viewport_aspect)
        } else {
            Vec::new()
        };
        self.opaque_vertices.clear();
        self.translucent_vertices.clear();
        #[cfg(feature = "studio-ui")]
        let static_translucent_vertex_count = if self.about_rendering {
            0
        } else {
            self.static_translucent_vertices.len()
        };
        if !self.about_rendering {
            self.translucent_vertices
                .extend_from_slice(&self.static_translucent_vertices);
        }
        split_world_vertices(
            &dynamic_vertices,
            &mut self.opaque_vertices,
            &mut self.translucent_vertices,
        );
        #[cfg(feature = "studio-ui")]
        if self.about_rendering {
            // The About scene has no authored static translucent geometry.
        } else if self.studio_static_translucent_sort_enabled {
            sort_translucent(&mut self.translucent_vertices, camera_position, target);
        } else {
            // Static translucent geometry is authored once and can be very
            // large. Keep it in authored order; only dynamic translucent
            // geometry needs camera-dependent sorting each frame.
            sort_translucent(
                &mut self.translucent_vertices[static_translucent_vertex_count..],
                camera_position,
                target,
            );
        }
        #[cfg(not(feature = "studio-ui"))]
        sort_translucent(&mut self.translucent_vertices, camera_position, target);
        let dynamic_count =
            self.opaque_vertices.len() + shadow_vertices.len() + self.translucent_vertices.len();
        let magic_mode = self.character_render_mode == CharacterRenderMode::Magic;
        #[cfg(feature = "studio-ui")]
        let shadows_enabled = self.studio_shadows_enabled;
        #[cfg(not(feature = "studio-ui"))]
        let shadows_enabled = true;
        self.characters.begin();
        let character_ink = self.scene.world.palette.ink;
        let reduced_effects = self.scene.reduced_effects;
        let lods = &mut self.scene.lods;
        #[cfg(feature = "studio-ui")]
        let runtime_players_visible = !self.studio_edit_mode;
        #[cfg(not(feature = "studio-ui"))]
        let runtime_players_visible = true;
        let mut add_character = |characters: &mut super::super::character_gpu::CharacterRenderer,
                                 entity: RenderEntity,
                                 style: super::super::AvatarStyle,
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
            if runtime_players_visible
                && self.scene.camera[2] > crate::camera::FIRST_PERSON_DISTANCE
            {
                let mut local = self.scene.player;
                local.camera_fade = crate::camera::fade(distance);
                add_character(&mut self.characters, local, local.style, 0, reduced_effects);
            }
            if runtime_players_visible {
                for (index, player) in self.scene.remote_players.iter().enumerate() {
                    add_character(
                        &mut self.characters,
                        *player,
                        player.style,
                        index + 1,
                        reduced_effects,
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
        #[cfg(feature = "studio-ui")]
        let runtime_ui_visible = !self.studio_edit_mode;
        #[cfg(not(feature = "studio-ui"))]
        let runtime_ui_visible = true;
        if runtime_ui_visible {
            self.add_world_labels(
                &mut ui_vertices,
                view_projection,
                camera_position,
                world_viewport,
            );
            if !self.avatar_preview_mode {
                ui_vertices.extend(super::super::ui::build_ui_vertices(&self.ui_frame));
            }
        }
        #[cfg(target_os = "android")]
        if !super::super::device::ANDROID_FIRST_FRAME_REPORTED
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            super::super::device::android_log(format!(
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
        if !self.ensure_dynamic_vertex_capacity(dynamic_count) {
            log::error!(
                "dynamic world geometry exceeds this GPU's maximum vertex-buffer size; skipping it"
            );
            self.opaque_vertices.clear();
            self.translucent_vertices.clear();
            shadow_vertices.clear();
        }
        if !self.ensure_ui_vertex_capacity(ui_vertices.len()) {
            log::error!(
                "world UI geometry exceeds this GPU's maximum vertex-buffer size; skipping it"
            );
            ui_vertices.clear();
        }
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
        self.queue.write_buffer(
            &self.sky_globals_buffer,
            0,
            bytemuck::bytes_of(&sky_globals),
        );
        self.queue.write_buffer(
            &self.shadow_globals_buffer,
            0,
            bytemuck::bytes_of(&shadow_globals),
        );
        self.post_processor.write_globals(&self.queue, post_globals);

        let frame = if offscreen {
            None
        } else {
            Some(match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    #[cfg(target_os = "android")]
                    if !super::super::device::ANDROID_SURFACE_WARNING_REPORTED
                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        super::super::device::android_log(
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
                    if !super::super::device::ANDROID_SURFACE_WARNING_REPORTED
                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        super::super::device::android_log(
                            "Android surface frame unavailable (timeout, occluded, or validation)",
                        );
                    }
                    return None;
                }
            })
        };
        let view = frame.as_ref().map_or_else(
            || self.targets.color.clone(),
            |frame| {
                frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default())
            },
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cubacadabra frame encoder"),
            });
        if shadows_enabled {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra directional shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_depth_view,
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
            pass.set_bind_group(0, &self.shadow_globals_bind_group, &[]);
            pass.set_viewport(
                0.0,
                0.0,
                super::super::device::SHADOW_MAP_SIZE as f32,
                super::super::device::SHADOW_MAP_SIZE as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(&self.shadow_pipeline);
            if !self.about_rendering && self.static_shadow_vertex_count > 0 {
                pass.set_vertex_buffer(0, self.static_shadow_vertex_buffer.slice(..));
                pass.draw(0..self.static_shadow_vertex_count as u32, 0..1);
            }
            if !self.about_rendering {
                for chunk in &self.terrain_meshes {
                    pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                    pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                }
                self.world_meshes
                    .draw_shadow(&mut pass, &self.world_mesh_shadow_pipeline);
            }
            if !self.opaque_vertices.is_empty() {
                pass.set_pipeline(&self.shadow_pipeline);
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(..));
                pass.draw(0..self.opaque_vertices.len() as u32, 0..1);
            }
            if magic_mode {
                self.characters
                    .draw_shadow(&mut pass, &self.character_shadow_pipeline);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra world pass"),
                color_attachments: &[Some(self.targets.world_attachment(wgpu::Color {
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
            pass.set_viewport(
                world_viewport.0,
                world_viewport.1,
                world_viewport.2,
                world_viewport.3,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                world_viewport.0.max(0.0) as u32,
                world_viewport.1.max(0.0) as u32,
                world_viewport.2.max(1.0) as u32,
                world_viewport.3.max(1.0) as u32,
            );
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.sky_globals_bind_group, &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_bind_group(1, &self.world_texture_bind_group, &[]);
            pass.set_bind_group(2, &self.terrain_texture_bind_group, &[]);
            pass.set_bind_group(3, &self.shadow_bind_group, &[]);
            if !self.about_rendering && self.static_vertex_count > 0 {
                pass.set_vertex_buffer(0, self.static_vertex_buffer.slice(..));
                pass.draw(0..self.static_vertex_count as u32, 0..1);
            }
            if !self.about_rendering {
                for chunk in &self.terrain_meshes {
                    pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                    pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                }
                self.world_meshes.draw(
                    &mut pass,
                    &self.world_mesh_pipeline,
                    &self.shadow_bind_group,
                );
            }
            if !self.opaque_vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
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
                self.characters
                    .draw(&mut pass, CharacterPass::Opaque, &self.shadow_bind_group);
                self.characters
                    .draw(&mut pass, CharacterPass::Face, &self.shadow_bind_group);
                self.characters
                    .draw(&mut pass, CharacterPass::Effect, &self.shadow_bind_group);
            }
            if !self.translucent_vertices.is_empty() {
                // Character pipelines use group 1 for their shadow receiver
                // (and group 2 for textured Morph shadows). Restore the
                // world pipeline's texture groups before drawing translucent
                // world geometry.
                pass.set_bind_group(1, &self.world_texture_bind_group, &[]);
                pass.set_bind_group(2, &self.terrain_texture_bind_group, &[]);
                pass.set_bind_group(3, &self.shadow_bind_group, &[]);
                pass.set_pipeline(&self.translucent_pipeline);
                let start = (size_of_val(self.opaque_vertices.as_slice())
                    + size_of_val(shadow_vertices.as_slice())) as u64;
                let end = start + size_of_val(self.translucent_vertices.as_slice()) as u64;
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(start..end));
                pass.draw(0..self.translucent_vertices.len() as u32, 0..1);
            }
        }
        self.post_processor.draw(&mut encoder, &self.targets);
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
}
