//! Phase 0's opt-in, offscreen review fixture.
//!
//! This module deliberately renders the current legacy avatar path. It is a
//! measurement and comparison tool, not a second production renderer. The
//! feature is kept out of normal client builds so the fixture cannot change
//! simulation capacity, public snapshots, or runtime resource lifetime.

use super::{
    DEPTH_FORMAT, Globals, RenderEntity, RenderPalette, Vertex, add_cuboid, add_legacy_avatar,
    color,
};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;
use wgpu::util::DeviceExt;

const FORMAT_VERSION: u32 = 1;
const WORLD_ASPECT: f32 = 16.0 / 9.0;
const DEFAULT_SEED: u64 = 0xC0BA_CAFE;
const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 360;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CaptureQuality {
    Full,
    Half,
}

impl CaptureQuality {
    fn scale(self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CapturePalette {
    Current,
    HighContrast,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct CaptureConfig {
    pub seed: u64,
    pub pose_time: f32,
    pub width: u32,
    pub height: u32,
    pub portrait_width: u32,
    pub portrait_height: u32,
    pub quality: CaptureQuality,
    pub palette: CapturePalette,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            pose_time: 0.35,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            portrait_width: 390,
            portrait_height: 844,
            quality: CaptureQuality::Full,
            palette: CapturePalette::Current,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CaptureRecord {
    pub name: String,
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub world_viewport: [u32; 4],
    pub actor_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub estimated_vertex_upload_bytes: usize,
    pub estimated_resource_bytes: usize,
    pub cpu_build_ms: f64,
    pub gpu_submit_and_readback_ms: f64,
    pub gpu_timestamp_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CaptureReport {
    pub format_version: u32,
    pub fixture: &'static str,
    pub config: CaptureConfig,
    pub adapter: AdapterRecord,
    pub captures: Vec<CaptureRecord>,
    pub engine_capacity_characters: usize,
    pub render_only_stress_characters: usize,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct AdapterRecord {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub gpu_timestamps: bool,
}

#[derive(Clone, Copy)]
enum Pose {
    Idle,
    Walk,
    Sprint,
    Jump,
}

#[derive(Clone, Copy)]
enum Camera {
    Third,
    First,
}

#[derive(Clone, Copy)]
enum Scenario {
    Single {
        name: &'static str,
        remote: bool,
        pose: Pose,
        camera: Camera,
    },
    Raised,
    Crowd {
        name: &'static str,
        count: usize,
        portrait: bool,
    },
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Single { name, .. } => name,
            Self::Raised => "raised-platform-third",
            Self::Crowd { name, .. } => name,
        }
    }
}

const PHASE0_SCENARIOS: [Scenario; 15] = [
    Scenario::Single {
        name: "local-idle-third",
        remote: false,
        pose: Pose::Idle,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-idle-third",
        remote: true,
        pose: Pose::Idle,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-walk-third",
        remote: false,
        pose: Pose::Walk,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-walk-third",
        remote: true,
        pose: Pose::Walk,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-sprint-third",
        remote: false,
        pose: Pose::Sprint,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-sprint-third",
        remote: true,
        pose: Pose::Sprint,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-jump-third",
        remote: false,
        pose: Pose::Jump,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "remote-jump-third",
        remote: true,
        pose: Pose::Jump,
        camera: Camera::Third,
    },
    Scenario::Single {
        name: "local-idle-first",
        remote: false,
        pose: Pose::Idle,
        camera: Camera::First,
    },
    Scenario::Single {
        name: "remote-idle-first",
        remote: true,
        pose: Pose::Idle,
        camera: Camera::First,
    },
    Scenario::Raised,
    Scenario::Crowd {
        name: "crowd-18-landscape",
        count: 18,
        portrait: false,
    },
    Scenario::Crowd {
        name: "crowd-50-landscape",
        count: 50,
        portrait: false,
    },
    Scenario::Crowd {
        name: "crowd-18-portrait-letterboxed",
        count: 18,
        portrait: true,
    },
    Scenario::Crowd {
        name: "crowd-50-portrait-letterboxed",
        count: 50,
        portrait: true,
    },
];

/// Render the complete Phase 0 baseline suite to PNGs and a JSON measurement
/// report. The report is deterministic except for adapter/device metadata and
/// measured timings.
pub fn capture_phase0_baseline(
    output_dir: impl AsRef<Path>,
    config: CaptureConfig,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;

    let context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };

    let mut captures = Vec::with_capacity(PHASE0_SCENARIOS.len());
    for scenario in PHASE0_SCENARIOS {
        captures.push(context.capture(output_dir, config, scenario)?);
    }

    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "phase-0-legacy-avatar-offscreen",
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "This fixture uses the current legacy CPU-expanded avatar path.",
            "The 50-character scene is render-only and never enters Engine simulation or the public snapshot.",
            "GPU timestamp queries are not requested by the production renderer; unavailable values are null.",
            "Portrait captures use the existing 16:9 world viewport centered inside the portrait target.",
            "Remote and local labels describe the source fixture; both use the current shared player appearance path.",
        ],
    };
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("serialize capture report: {error}"))?;
    fs::write(output_dir.join("phase0_report.json"), report_bytes)
        .map_err(|error| format!("write capture report: {error}"))?;
    Ok(report)
}

struct HeadlessContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_layout: wgpu::BindGroupLayout,
    adapter_info: wgpu::AdapterInfo,
}

impl HeadlessContext {
    fn new() -> Result<Self, String> {
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
            multisample: wgpu::MultisampleState::default(),
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
        Ok(Self {
            device,
            queue,
            pipeline,
            globals_layout,
            adapter_info,
        })
    }

    fn capture(
        &self,
        output_dir: &Path,
        config: CaptureConfig,
        scenario: Scenario,
    ) -> Result<CaptureRecord, String> {
        let (width, height) = dimensions(config, scenario);
        let viewport = world_viewport(width, height);
        let build_started = Instant::now();
        let (vertices, actor_count, globals, sky) = build_scene(config, scenario, width, height);
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
                sample_count: 1,
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
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
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
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
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
            world_viewport: viewport,
            actor_count,
            vertex_count: vertices.len(),
            triangle_count: vertices.len() / 3,
            estimated_vertex_upload_bytes: vertex_upload_bytes,
            estimated_resource_bytes: vertex_upload_bytes
                + readback_size as usize
                + (width as usize * height as usize * 4)
                + (width as usize * height as usize * 4),
            cpu_build_ms,
            gpu_submit_and_readback_ms,
            gpu_timestamp_ms: None,
        })
    }
}

fn build_scene(
    config: CaptureConfig,
    scenario: Scenario,
    width: u32,
    height: u32,
) -> (Vec<Vertex>, usize, Globals, [f32; 4]) {
    let palette = capture_palette(config.palette);
    let mut vertices = Vec::with_capacity(50 * 10 * 36);
    add_cuboid(
        &mut vertices,
        Vec3::new(0.0, -0.08, 0.0),
        Vec3::new(120.0, 0.16, 120.0),
        palette.ground,
    );

    let mut actors = Vec::new();
    let mut camera = Camera::Third;
    let mut target = Vec3::new(0.0, 1.78, 0.0);
    let mut distance = 8.0;
    let mut raised = false;
    match scenario {
        Scenario::Single {
            remote,
            pose,
            camera: view,
            ..
        } => {
            camera = view;
            if !(matches!(view, Camera::First) && !remote) {
                let (y, walk_cycle) = pose_values(pose, config.pose_time);
                let z = if matches!(view, Camera::First) {
                    -7.0
                } else {
                    0.0
                };
                actors.push(RenderEntity {
                    position: [0.0, y, z],
                    yaw: 0.0,
                    walk_cycle,
                    moving: !matches!(pose, Pose::Idle),
                    sprinting: matches!(pose, Pose::Sprint),
                });
            }
        }
        Scenario::Raised => {
            raised = true;
            target = Vec3::new(0.0, 1.8, -2.5);
            actors.push(RenderEntity {
                position: [0.0, 2.0, -2.5],
                yaw: 0.0,
                walk_cycle: 0.9,
                moving: true,
                sprinting: false,
            });
        }
        Scenario::Crowd { count, .. } => {
            distance = 18.0;
            actors = crowd_actors(count, config.seed);
        }
    }
    if raised {
        add_cuboid(
            &mut vertices,
            Vec3::new(0.0, 1.0, -2.5),
            Vec3::new(5.0, 2.0, 5.0),
            palette.platform,
        );
    }
    for actor in &actors {
        add_legacy_avatar(&mut vertices, *actor, palette.avatar, palette.ink);
    }

    let (camera_position, look_target) = match camera {
        Camera::First => {
            let position = Vec3::new(0.0, 3.4, 0.0);
            (position, position + Vec3::new(0.0, 0.0, -1.0))
        }
        Camera::Third => {
            let vertical = (distance * (-0.095_f32).sin()).clamp(-2.0, distance);
            let position = target + Vec3::new(0.0, vertical, distance);
            (position, target)
        }
    };
    let aspect = viewport_aspect(width, height);
    let view_projection = Mat4::perspective_rh(62.0_f32.to_radians(), aspect, 0.05, 240.0)
        * Mat4::look_at_rh(camera_position, look_target, Vec3::Y);
    let globals = Globals {
        view_projection: view_projection.to_cols_array_2d(),
        camera_position: camera_position.extend(1.0).to_array(),
        sun_direction: Vec3::new(-0.45, -0.82, 0.32)
            .normalize()
            .extend(0.0)
            .to_array(),
        fog_color: palette.sky,
    };
    (vertices, actors.len(), globals, palette.sky)
}

fn capture_palette(palette: CapturePalette) -> CaptureColors {
    match palette {
        CapturePalette::Current => CaptureColors {
            avatar: super::default_player_style(),
            sky: RenderPalette::default().sky,
            ground: RenderPalette::default().ground,
            platform: color(0xd0a86f),
            ink: RenderPalette::default().ink,
        },
        CapturePalette::HighContrast => CaptureColors {
            avatar: super::AvatarStyle {
                skin: color(0xffc18f),
                shirt: color(0x176b87),
                pants: color(0x313a72),
                shoes: color(0x141c2b),
            },
            sky: color(0xd9edf0),
            ground: color(0xc2d6a5),
            platform: color(0xe2a04f),
            ink: color(0x102f3c),
        },
    }
}

struct CaptureColors {
    avatar: super::AvatarStyle,
    sky: [f32; 4],
    ground: [f32; 4],
    platform: [f32; 4],
    ink: [f32; 4],
}

fn pose_values(pose: Pose, pose_time: f32) -> (f32, f32) {
    let time = if pose_time.is_finite() {
        pose_time.max(0.0)
    } else {
        0.0
    };
    match pose {
        Pose::Idle => (0.0, 0.0),
        Pose::Walk => (0.0, time * 6.4),
        Pose::Sprint => (0.0, time * 11.5),
        Pose::Jump => (1.15, time * 6.4),
    }
}

fn crowd_actors(count: usize, seed: u64) -> Vec<RenderEntity> {
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns.max(1));
    let spacing_x = if count >= 50 { 2.2 } else { 3.1 };
    let spacing_z = if count >= 50 { 2.4 } else { 3.4 };
    let mut random = seed;
    (0..count)
        .map(|index| {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let jitter_x = ((random >> 32) as f32 / u32::MAX as f32 - 0.5) * 0.35;
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let jitter_z = ((random >> 32) as f32 / u32::MAX as f32 - 0.5) * 0.35;
            let column = index % columns.max(1);
            let row = index / columns.max(1);
            RenderEntity {
                position: [
                    (column as f32 - (columns as f32 - 1.0) * 0.5) * spacing_x + jitter_x,
                    0.0,
                    (row as f32 - (rows as f32 - 1.0) * 0.5) * spacing_z + jitter_z,
                ],
                yaw: if index % 2 == 0 {
                    0.0
                } else {
                    std::f32::consts::PI
                },
                walk_cycle: index as f32 * 0.37,
                moving: true,
                sprinting: false,
            }
        })
        .collect()
}

fn dimensions(config: CaptureConfig, scenario: Scenario) -> (u32, u32) {
    let (width, height) = match scenario {
        Scenario::Crowd { portrait: true, .. } => (config.portrait_width, config.portrait_height),
        _ => (config.width, config.height),
    };
    let scale = config.quality.scale();
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

fn viewport_aspect(width: u32, height: u32) -> f32 {
    let viewport = world_viewport(width, height);
    (viewport[2] as f32 / viewport[3].max(1) as f32).max(0.1)
}

fn world_viewport(width: u32, height: u32) -> [u32; 4] {
    let aspect = width as f32 / height.max(1) as f32;
    if aspect >= 1.25 {
        return [0, 0, width, height];
    }
    let viewport_height = (width as f32 / WORLD_ASPECT).min(height as f32);
    [
        0,
        ((height as f32 - viewport_height) * 0.5).round() as u32,
        width,
        viewport_height.round().max(1.0) as u32,
    ]
}

fn align_to(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("write PNG header: {error}"))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("write PNG pixels: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase0_dimensions_keep_portrait_world_letterbox() {
        let config = CaptureConfig::default();
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[0]), (640, 360));
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[13]), (390, 844));
        assert_eq!(world_viewport(390, 844), [0, 312, 390, 219]);
    }

    #[test]
    fn phase0_crowd_fixture_is_seeded_and_isolated() {
        let first = crowd_actors(50, DEFAULT_SEED);
        let second = crowd_actors(50, DEFAULT_SEED);
        assert_eq!(first.len(), 50);
        assert_eq!(first[17].position, second[17].position);
        assert_ne!(
            first[17].position,
            crowd_actors(50, DEFAULT_SEED + 1)[17].position
        );
    }

    #[test]
    fn phase0_quality_scales_capture_targets() {
        let config = CaptureConfig {
            quality: CaptureQuality::Half,
            ..CaptureConfig::default()
        };
        assert_eq!(dimensions(config, PHASE0_SCENARIOS[0]), (320, 180));
    }
}
