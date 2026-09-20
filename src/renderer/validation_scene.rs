use super::*;

pub(super) struct Context<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) queue: &'a wgpu::Queue,
    pub(super) world_texture_layout: &'a wgpu::BindGroupLayout,
    pub(super) terrain_texture_layout: &'a wgpu::BindGroupLayout,
    pub(super) shadow_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub(super) world_texture_bind_group: wgpu::BindGroup,
    pub(super) terrain_texture_bind_group: wgpu::BindGroup,
    pub(super) shadow_bind_group: wgpu::BindGroup,
    pub(super) globals_buffer: wgpu::Buffer,
    pub(super) globals: wgpu::BindGroup,
    pub(super) ui: wgpu::RenderPipeline,
    pub(super) atlas: wgpu::BindGroup,
    pub(super) timer: Option<wgpu::QuerySet>,
    pub(super) query_buffer: wgpu::Buffer,
}
pub(super) struct TestScene {
    pub(super) effects_first: bool,
    width: u32,
    height: u32,
    viewport: [f32; 4],
    globals: Globals,
    world: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
    world_buffer: wgpu::Buffer,
    opaque_count: u32,
    world_count: u32,
    ui_buffer: wgpu::Buffer,
    ui_count: u32,
    targets: SceneTargets,
    post_processor: PostProcessor,
    presenter: Presenter,
    output: wgpu::Texture,
    readback: wgpu::Buffer,
    row_bytes: u32,
}
impl TestScene {
    pub(super) fn new(
        ctx: &Context<'_>,
        layout: &wgpu::BindGroupLayout,
        samples: u32,
        width: u32,
        height: u32,
        count: usize,
        wall: bool,
        format: wgpu::TextureFormat,
    ) -> Self {
        let device = ctx.device;
        let presenter = Presenter::new(device, format);
        let post_processor = PostProcessor::new(device);
        let targets = SceneTargets::new(
            device,
            width,
            height,
            samples,
            &post_processor,
            &presenter.layout,
        );
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row_bytes = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: row_bytes as u64 * height as u64 + 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let viewport = if width as f32 / height as f32 >= 1.25 {
            [0.0, 0.0, width as f32, height as f32]
        } else {
            let h = width as f32 * 9.0 / 16.0;
            [0.0, (height as f32 - h) * 0.5, width as f32, h]
        };
        let target = Vec3::new(0.0, 1.58, 0.0);
        let camera = target + Vec3::new(0.0, 2.0, if count == 3 { -9.0 } else { -22.0 });
        let globals = Globals {
            view_projection: (Mat4::perspective_rh(
                62_f32.to_radians(),
                viewport[2] / viewport[3],
                0.05,
                240.0,
            ) * Mat4::look_at_rh(camera, target, Vec3::Y))
            .to_cols_array_2d(),
            camera_position: camera.extend(1.0).to_array(),
            sun_direction: Vec3::new(-0.45, -0.82, 0.32)
                .normalize()
                .extend(0.0)
                .to_array(),
            fog_color: super::super::color(0x9ab9be),
            color_grade: [1.0, 1.0, 1.0, 0.0],
            atmosphere: [52.0, 115.0, 0.0, 0.0],
            lighting: [0.72, 0.72, 0.72, 2.0],
        };
        let mut world = Vec::new();
        super::super::add_cuboid(
            &mut world,
            Vec3::new(0.0, -0.08, 0.0),
            Vec3::new(120.0, 0.16, 120.0),
            super::super::color(0xa7bd99),
        );
        if wall {
            super::super::add_cuboid(
                &mut world,
                Vec3::new(0.0, 2.0, -2.0),
                Vec3::new(12.0, 5.0, 0.2),
                super::super::color(0xd0a86f),
            );
        } else {
            super::super::add_cuboid(
                &mut world,
                Vec3::new(3.5, 0.7, 0.6),
                Vec3::new(1.0, 1.4, 1.2),
                [0.2, 0.6, 0.7, 0.4],
            );
        }
        let (mut opaque, mut alpha) = (Vec::new(), Vec::new());
        super::super::draw::split_world_vertices(&world, &mut opaque, &mut alpha);
        super::super::draw::sort_translucent(&mut alpha, camera, target);
        let opaque_count = opaque.len() as u32;
        opaque.extend_from_slice(&alpha);
        let world_count = opaque.len() as u32;
        let world_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&opaque),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ui_vertices = ui_fixture(width, height);
        let ui_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&ui_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            effects_first: false,
            width,
            height,
            viewport,
            globals,
            world: super::super::device::world_pipeline(
                device,
                layout,
                ctx.world_texture_layout,
                ctx.terrain_texture_layout,
                ctx.shadow_bind_group_layout,
                samples,
                false,
            ),
            translucent: super::super::device::world_pipeline(
                device,
                layout,
                ctx.world_texture_layout,
                ctx.terrain_texture_layout,
                ctx.shadow_bind_group_layout,
                samples,
                true,
            ),
            world_buffer,
            opaque_count,
            world_count,
            ui_buffer,
            ui_count: ui_vertices.len() as u32,
            targets,
            post_processor,
            presenter,
            output,
            readback,
            row_bytes,
        }
    }

    pub(super) async fn render(
        &self,
        ctx: &Context<'_>,
        characters: &CharacterRenderer,
        capture: bool,
        present: bool,
    ) -> Result<(Vec<u8>, Option<f64>), String> {
        ctx.queue
            .write_buffer(&ctx.globals_buffer, 0, bytemuck::bytes_of(&self.globals));
        let view = self.output.create_view(&Default::default());
        let mut encoder = ctx.device.create_command_encoder(&Default::default());
        let clear = wgpu::Color {
            r: 154.0 / 255.0,
            g: 185.0 / 255.0,
            b: 190.0 / 255.0,
            a: 1.0,
        };
        {
            let mut attachment = self.targets.world_attachment(clear);
            if !present {
                attachment.view = &view;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("production 3D validation"),
                color_attachments: &[Some(attachment)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: ctx
                    .timer
                    .as_ref()
                    .map(|timer| wgpu::RenderPassTimestampWrites {
                        query_set: timer,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let v = self.viewport;
            pass.set_viewport(v[0], v[1], v[2], v[3], 0.0, 1.0);
            pass.set_bind_group(0, &ctx.globals, &[]);
            pass.set_bind_group(1, &ctx.world_texture_bind_group, &[]);
            pass.set_bind_group(2, &ctx.terrain_texture_bind_group, &[]);
            pass.set_bind_group(3, &ctx.shadow_bind_group, &[]);
            pass.set_pipeline(&self.world);
            pass.set_vertex_buffer(0, self.world_buffer.slice(..));
            pass.draw(0..self.opaque_count, 0..1);
            if self.effects_first {
                characters.draw(&mut pass, CharacterPass::Effect, &ctx.shadow_bind_group);
            }
            characters.draw(&mut pass, CharacterPass::Opaque, &ctx.shadow_bind_group);
            characters.draw(&mut pass, CharacterPass::Face, &ctx.shadow_bind_group);
            if !self.effects_first {
                characters.draw(&mut pass, CharacterPass::Effect, &ctx.shadow_bind_group);
            }
            pass.set_bind_group(1, &ctx.world_texture_bind_group, &[]);
            pass.set_bind_group(2, &ctx.terrain_texture_bind_group, &[]);
            pass.set_bind_group(3, &ctx.shadow_bind_group, &[]);
            pass.set_pipeline(&self.translucent);
            pass.set_vertex_buffer(0, self.world_buffer.slice(..));
            pass.draw(self.opaque_count..self.world_count, 0..1);
        }
        if present {
            self.post_processor.write_globals(
                ctx.queue,
                super::super::PostGlobals {
                    color_correction: [0.0, 0.0, 0.0, 0.0],
                    sun_rays: [0.0, 0.0, 0.0, 0.0],
                },
            );
            self.post_processor.draw(&mut encoder, &self.targets);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("production UI validation"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if present { &self.targets.color } else { &view },
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
            pass.set_pipeline(&ctx.ui);
            pass.set_bind_group(0, &ctx.atlas, &[]);
            pass.set_vertex_buffer(0, self.ui_buffer.slice(..));
            pass.draw(0..self.ui_count, 0..1);
        }
        if present {
            self.presenter.draw(&mut encoder, &self.targets, &view);
        }
        if let Some(timer) = &ctx.timer {
            encoder.resolve_query_set(timer, 0..2, &ctx.query_buffer, 0);
            encoder.copy_buffer_to_buffer(&ctx.query_buffer, 0, &self.readback, 0, 16);
        }
        if capture {
            encoder.copy_texture_to_buffer(
                self.output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 256,
                        bytes_per_row: Some(self.row_bytes),
                        rows_per_image: Some(self.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
        } else if ctx.timer.is_none() {
            // Associate readback with this submission even on adapters without
            // timestamps. Mapping an otherwise unused buffer is not a GPU
            // completion fence and would under-report submit-and-wait time.
            encoder.copy_texture_to_buffer(
                self.output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 256,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        ctx.queue.submit(Some(encoder.finish()));
        map(ctx.device, &self.readback).await?;
        let mapped = self.readback.slice(..).get_mapped_range();
        let gpu_ms = ctx.timer.as_ref().and_then(|_| {
            let start = u64::from_le_bytes(mapped[0..8].try_into().unwrap());
            let end = u64::from_le_bytes(mapped[8..16].try_into().unwrap());
            end.checked_sub(start)
                .filter(|delta| *delta > 0)
                .map(|delta| delta as f64 * ctx.queue.get_timestamp_period() as f64 / 1_000_000.0)
        });
        let mut pixels = Vec::new();
        if capture {
            pixels.reserve((self.width * self.height * 4) as usize);
            for row in 0..self.height {
                let start = (256 + row * self.row_bytes) as usize;
                pixels.extend_from_slice(&mapped[start..start + self.width as usize * 4]);
            }
        }
        drop(mapped);
        self.readback.unmap();
        Ok((pixels, gpu_ms))
    }
}

pub(super) fn populate(characters: &mut CharacterRenderer, count: usize, phase: f32) {
    characters.begin();
    let columns = (count as f32).sqrt().ceil() as usize;
    for index in 0..count {
        let position = if count == 3 {
            [(index as f32 - 1.0) * 2.2, 0.0, 0.0]
        } else {
            [
                (index % columns) as f32 * 2.2 - (columns as f32 - 1.0) * 1.1,
                0.0,
                (index / columns) as f32 * 2.4 - (count.div_ceil(columns) as f32 - 1.0) * 1.2,
            ]
        };
        let mut style = super::super::default_player_style();
        style.body = crate::character::BodyId::ALL[index % crate::character::BodyId::ALL.len()];
        let requested_outfit =
            crate::character::OutfitId::ALL[index % crate::character::OutfitId::ALL.len()];
        style.outfit = requested_outfit
            .supported_by(style.body)
            .then_some(requested_outfit)
            .unwrap_or(crate::character::OutfitId::fallback());
        let body = style.body;
        let outfit = style.outfit;
        let walk_cycle = phase + index as f32 * 0.37;
        let pose = crate::character::Pose::locomotion(
            &crate::character::body_recipe(body).rig,
            walk_cycle,
            count > 3,
            index % 2 == 0,
        );
        style.skin = super::super::color([0xe8ae86, 0xc98464, 0x82b78f][index % 3]);
        style.shirt = super::super::color([0x2d6663, 0x5f8f78, 0x694c88][index % 3]);
        characters.add(
            RenderEntity {
                position,
                body,
                outfit,
                walk_cycle,
                pose,
                moving: count > 3,
                sprinting: index % 2 == 0,
                ..Default::default()
            },
            style,
            super::super::color(0x173f43),
        );
    }
}
