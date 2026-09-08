//! Headless GPU setup, rendering, readback, and capture measurement.

use super::scene::{align_to, build_scene, capture_palette, dimensions, world_viewport, write_png};
use super::*;
use crate::character::{Pose as CharacterPose, body_recipe};
use glam::{Mat4, Vec3};
use std::path::Path;
use std::time::Instant;
use wgpu::util::DeviceExt;

pub(super) struct HeadlessContext {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) globals_layout: wgpu::BindGroupLayout,
    pub(super) characters: super::super::character_gpu::CharacterRenderer,
    pub(super) adapter_info: wgpu::AdapterInfo,
    pub(super) samples: u32,
}

impl HeadlessContext {
    pub(super) fn new() -> Result<Self, String> {
        Self::new_with_quality(false)
    }

    pub(super) fn new_with_quality(antialias: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|_| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })
        .map_err(|error| format!("headless adapter unavailable: {error}"))?;
        let adapter_info = adapter.get_info();
        let samples = super::super::targets::select_samples(&adapter, antialias);
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cubacadabra phase 0 capture device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("headless device unavailable: {error}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cubacadabra phase 0 world shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../renderer.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cubacadabra phase 0 globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cubacadabra phase 0 pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cubacadabra phase 0 world pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::LAYOUT],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let characters =
            super::super::character_gpu::CharacterRenderer::new(&device, &globals_layout, samples);
        Ok(Self {
            device,
            queue,
            pipeline,
            globals_layout,
            characters,
            adapter_info,
            samples,
        })
    }

    pub(super) fn capture(
        &mut self,
        output_dir: &Path,
        config: CaptureConfig,
        scenario: Scenario,
    ) -> Result<CaptureRecord, String> {
        let (width, height) = dimensions(config, scenario);
        let viewport = world_viewport(width, height);
        let build_started = Instant::now();
        let (vertices, actors, globals, sky) = build_scene(config, scenario, width, height);
        let actor_count = actors.len();
        let magic = matches!(
            config.avatar,
            CaptureAvatar::Magic | CaptureAvatar::Wardrobe
        );
        if magic {
            self.characters.head_only = matches!(scenario, Scenario::HairReview);
            self.characters.hero_study = if let Scenario::Hero { study, .. } = scenario {
                study
            } else {
                super::super::hero_character::Study::Everyday
            };
            self.characters.begin();
            let palette = capture_palette(config.palette);
            for (rank, actor) in actors.iter().enumerate() {
                let mut entity = *actor;
                let recipe = body_recipe(entity.body);
                if !matches!(
                    scenario,
                    Scenario::MotionLineup | Scenario::Hero { .. } | Scenario::HairReview
                ) {
                    entity.secondary.stride_blend = if entity.moving {
                        if entity.sprinting { 1.0 } else { 6.4 / 11.5 }
                    } else {
                        0.0
                    };
                    entity.support = if matches!(
                        scenario,
                        Scenario::Single {
                            pose: Pose::Jump,
                            ..
                        }
                    ) {
                        crate::types::CharacterSupport::Airborne
                    } else {
                        crate::types::CharacterSupport::Grounded {
                            height: entity.position[1],
                        }
                    };
                    entity.pose = CharacterPose::locomotion(
                        &recipe.rig,
                        entity.walk_cycle,
                        entity.moving,
                        entity.sprinting,
                    );
                }
                let mut style = palette.avatar;
                style.body = entity.body;
                style.outfit = entity.outfit;
                if matches!(scenario, Scenario::HairReview) {
                    style.skin = if entity.body == crate::character::BodyId::PersonGirl {
                        color(0xefb083)
                    } else {
                        color(0xc98245)
                    };
                }
                if matches!(scenario, Scenario::Hero { .. }) {
                    style.skin = color(0xe1a66d);
                    style.shirt = color(0x14733e);
                    style.pants = color(0x243349);
                    style.shoes = color(0x23563b);
                }
                if matches!(
                    scenario,
                    Scenario::ShapeLineup { .. } | Scenario::MotionLineup
                ) {
                    use crate::character::BodyId;
                    match entity.body {
                        BodyId::Person | BodyId::PersonGirl | BodyId::PersonNonbinary => {}
                        BodyId::Cat => {
                            style.skin = color(0xc98464);
                            style.shirt = color(0xc7542b);
                        }
                        BodyId::Dragon => {
                            style.skin = color(0x82b78f);
                            style.shirt = color(0x694c88);
                        }
                    }
                }
                if matches!(scenario, Scenario::WardrobeLineup { .. }) {
                    // Explicit fixture palettes demonstrate species and material
                    // separation without overriding player-selected live colors.
                    let palettes = [
                        ([0.86, 0.61, 0.44, 1.0], [0.18, 0.43, 0.40, 1.0]),
                        ([0.80, 0.53, 0.29, 1.0], [0.78, 0.33, 0.17, 1.0]),
                        ([0.66, 0.70, 0.76, 1.0], [0.91, 0.67, 0.18, 1.0]),
                        ([0.42, 0.65, 0.57, 1.0], [0.34, 0.28, 0.58, 1.0]),
                        ([0.45, 0.63, 0.68, 1.0], [0.31, 0.39, 0.53, 1.0]),
                        ([0.46, 0.28, 0.18, 1.0], [0.69, 0.47, 0.50, 1.0]),
                    ];
                    (style.skin, style.shirt) = palettes[rank % palettes.len()];
                    style.pants = if rank == 5 {
                        [0.58, 0.38, 0.46, 1.0]
                    } else {
                        [0.24, 0.31, 0.42, 1.0]
                    };
                    style.shoes = [0.18, 0.22, 0.28, 1.0];
                    entity.face = crate::character::FaceParameters::preset(
                        crate::character::FacePreset::Happy,
                    );
                }
                let lod = if let Scenario::Orbit {
                    yaw,
                    pitch,
                    distance,
                    ..
                }
                | Scenario::Hero {
                    yaw,
                    pitch,
                    distance,
                    ..
                } = scenario
                {
                    let (position, target) =
                        super::super::camera::orbit(Vec3::ZERO, entity.body, yaw, pitch, distance);
                    let view = Mat4::look_at_rh(position, target, Vec3::Y);
                    super::super::character_quality::select_lod(
                        super::super::character_quality::projected_height(
                            entity,
                            view,
                            viewport[3] as f32,
                        ),
                        None,
                    )
                } else {
                    CharacterLod::Mid
                };
                self.characters
                    .add_with_quality(entity, style, palette.ink, lod, rank, false);
            }
            if matches!(
                scenario,
                Scenario::ShapeLineup {
                    silhouette: true,
                    ..
                } | Scenario::Hero {
                    silhouette: true,
                    ..
                }
            ) {
                self.characters.make_silhouette();
            }
            self.characters.upload(&self.queue);
        }
        let cpu_build_ms = build_started.elapsed().as_secs_f64() * 1000.0;
        let vertex_upload_bytes = std::mem::size_of_val(vertices.as_slice());
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cubacadabra phase 0 capture vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let globals_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cubacadabra phase 0 capture globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cubacadabra phase 0 capture globals bind group"),
            layout: &self.globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });
        let color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cubacadabra phase 0 capture color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let multisample_view = (self.samples > 1).then(|| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("hero multisample color"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: self.samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let depth_view = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("cubacadabra phase 0 capture depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: self.samples,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let padded_row_bytes =
            align_to(width as u64 * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64);
        let readback_size = padded_row_bytes * height as u64;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cubacadabra phase 0 capture readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let render_started = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cubacadabra phase 0 capture encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cubacadabra phase 0 capture pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: multisample_view.as_ref().unwrap_or(&color_view),
                    depth_slice: None,
                    resolve_target: multisample_view.as_ref().map(|_| &color_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky[0] as f64,
                            g: sky[1] as f64,
                            b: sky[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
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
                viewport[0] as f32,
                viewport[1] as f32,
                viewport[2] as f32,
                viewport[3] as f32,
                0.0,
                1.0,
            );
            pass.set_bind_group(0, &globals_bind_group, &[]);
            if !vertices.is_empty() {
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
            if magic {
                self.characters.draw(
                    &mut pass,
                    super::super::character_material::CharacterPass::Opaque,
                );
                self.characters.draw(
                    &mut pass,
                    super::super::character_material::CharacterPass::Face,
                );
                self.characters.draw(
                    &mut pass,
                    super::super::character_material::CharacterPass::Effect,
                );
            }
        }
        encoder.copy_texture_to_buffer(
            color_texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("poll capture device: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("receive capture readback: {error}"))?
            .map_err(|error| format!("map capture readback: {error}"))?;
        let mapped = slice.get_mapped_range();
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        for row in 0..height as usize {
            let source = row * padded_row_bytes as usize;
            let target = row * width as usize * 4;
            pixels[target..target + width as usize * 4]
                .copy_from_slice(&mapped[source..source + width as usize * 4]);
        }
        drop(mapped);
        readback.unmap();
        let gpu_submit_and_readback_ms = render_started.elapsed().as_secs_f64() * 1000.0;

        let file_name = format!("{}.png", scenario.name());
        write_png(&output_dir.join(&file_name), width, height, &pixels)?;

        Ok(CaptureRecord {
            name: scenario.name().to_owned(),
            image: file_name,
            width,
            height,
            sample_count: self.samples,
            world_viewport: viewport,
            actor_count,
            vertex_count: vertices.len()
                + if magic {
                    self.characters.stats.triangles * 3
                } else {
                    0
                },
            triangle_count: vertices.len() / 3
                + if magic {
                    self.characters.stats.triangles
                } else {
                    0
                },
            render_mode: if magic { "magic" } else { "legacy" },
            character_draws: if magic {
                self.characters.stats.draws
            } else {
                0
            },
            character_instances: if magic {
                self.characters.stats.instances
            } else {
                0
            },
            character_mesh_uploads: if magic {
                self.characters.stats.mesh_uploads
            } else {
                0
            },
            character_resident_bytes: if magic {
                self.characters.stats.resident_bytes
            } else {
                0
            },
            estimated_vertex_upload_bytes: vertex_upload_bytes
                + if magic {
                    self.characters.stats.upload_bytes
                } else {
                    0
                },
            estimated_resource_bytes: vertex_upload_bytes
                + readback_size as usize
                + width as usize
                    * height as usize
                    * if self.samples > 1 {
                        4 + 8 * self.samples as usize
                    } else {
                        8
                    }
                + if magic {
                    self.characters.stats.resident_bytes
                } else {
                    0
                },
            cpu_build_ms,
            gpu_submit_and_readback_ms,
            gpu_timestamp_ms: None,
        })
    }
}
