use glam::{Mat4, Vec3};

use super::{
    Globals, Renderer, Vertex, add_avatar, add_cloud, add_cuboid, add_cuboid_outline,
    add_launch_pad, add_pixel_text, add_spawn_pad, faded,
};

impl Renderer {
    pub fn draw(&mut self) {
        let player = Vec3::from_array(self.scene.player.position);
        let [yaw, pitch, distance] = self.scene.camera;
        let (camera_position, target) = if distance <= 0.75 {
            let camera_position = player + Vec3::new(0.0, 3.4, 0.0);
            let look_direction = Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                -yaw.cos() * pitch.cos(),
            );
            (camera_position, camera_position + look_direction)
        } else {
            let target = player + Vec3::new(0.0, 1.78, 0.0);
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
        let view_projection = Mat4::perspective_rh(
            62.0_f32.to_radians(),
            (world_viewport.2 / world_viewport.3.max(1.0)).max(0.1),
            0.05,
            240.0,
        ) * Mat4::look_at_rh(camera_position, target, Vec3::Y);
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
        let ui_vertices = super::ui::build_ui_vertices(&self.ui_frame);
        self.ensure_dynamic_vertex_capacity(dynamic_vertices.len());
        self.ensure_ui_vertex_capacity(ui_vertices.len());
        if !dynamic_vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                0,
                bytemuck::cast_slice(&dynamic_vertices),
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
                self.surface.configure(&self.device, &self.config);
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
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.scene.world.palette.sky[0] as f64,
                            g: self.scene.world.palette.sky[1] as f64,
                            b: self.scene.world.palette.sky[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
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
            if !dynamic_vertices.is_empty() {
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(..));
                pass.draw(0..dynamic_vertices.len() as u32, 0..1);
            }
        }
        if !ui_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra UI pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
        for player in &self.scene.remote_players {
            add_avatar(
                &mut mesh,
                *player,
                self.scene.player_style,
                world.palette.ink,
                &mut self.rounded_mesh_cache,
            );
        }
        if self.scene.camera[2] > 0.75 {
            add_avatar(
                &mut mesh,
                self.scene.player,
                self.scene.player_style,
                world.palette.ink,
                &mut self.rounded_mesh_cache,
            );
        }
        mesh
    }

    pub(super) fn rebuild_static_vertices(&mut self) {
        let vertices = self.build_static_vertices();
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
