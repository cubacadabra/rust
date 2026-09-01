use std::ffi::c_void;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RenderBlock {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RenderPad {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub seconds: f32,
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub struct RenderAgent {
    pub position: [f32; 3],
    pub yaw: f32,
    pub walk_cycle: f32,
    pub assembled: f32,
    pub skin: [f32; 4],
    pub shirt: [f32; 4],
    pub pants: [f32; 4],
    pub shoes: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub struct RenderPalette {
    pub sky: [f32; 4],
    pub ground: [f32; 4],
    pub ground_edge: [f32; 4],
    pub grid: [f32; 4],
    pub ink: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4
        ],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
    sun_direction: [f32; 4],
    fog_color: [f32; 4],
}

#[derive(Default)]
struct Scene {
    blocks: Vec<RenderBlock>,
    pads: Vec<RenderPad>,
    agents: Vec<RenderAgent>,
    player: RenderAgent,
    ground_size: f32,
    palette: RenderPalette,
    elapsed: f32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    width: f32,
    height: f32,
    scene: Scene,
}

impl Renderer {
    pub fn new(layer: *mut c_void, width: f32, height: f32) -> Option<Self> {
        if layer.is_null() || width <= 0.0 || height <= 0.0 {
            return None;
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
                .ok()?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok()?;
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cubacadabra game device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .ok()?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1.0) as u32,
            height: height.max(1.0) as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cubacadabra world shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("renderer.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cubacadabra globals layout"),
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
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cubacadabra globals"),
            contents: bytemuck::bytes_of(&Globals::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cubacadabra globals bind group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cubacadabra pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cubacadabra world pipeline"),
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
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 16_384;
        let vertex_buffer = create_vertex_buffer(&device, vertex_capacity);
        let depth_view = create_depth_view(&device, config.width, config.height);

        Some(Self {
            surface,
            device,
            queue,
            pipeline,
            globals_buffer,
            globals_bind_group,
            vertex_buffer,
            vertex_capacity,
            config,
            depth_view,
            width,
            height,
            scene: Scene::default(),
        })
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width.max(1.0) as u32;
        self.config.height = height.max(1.0) as u32;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, self.config.width, self.config.height);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_scene(
        &mut self,
        blocks: &[RenderBlock],
        pads: &[RenderPad],
        agents: &[RenderAgent],
        player: RenderAgent,
        ground_size: f32,
        palette: RenderPalette,
        elapsed: f32,
    ) {
        self.scene.blocks.clear();
        self.scene.blocks.extend_from_slice(blocks);
        self.scene.pads.clear();
        self.scene.pads.extend_from_slice(pads);
        self.scene.agents.clear();
        self.scene.agents.extend_from_slice(agents);
        self.scene.player = player;
        self.scene.ground_size = ground_size.max(10.0);
        self.scene.palette = palette;
        self.scene.elapsed = elapsed;
    }

    pub fn draw(&mut self) {
        let player = Vec3::from_array(self.scene.player.position);
        let camera_position = player + Vec3::new(0.0, 8.0, 11.0);
        let target = player + Vec3::new(0.0, 1.0, 0.0);
        let view_projection = Mat4::perspective_rh(
            62.0_f32.to_radians(),
            (self.width / self.height.max(1.0)).max(0.1),
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
            fog_color: self.scene.palette.sky,
        };
        let vertices = self.build_vertices();
        self.ensure_vertex_capacity(vertices.len());
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
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
                            r: self.scene.palette.sky[0] as f64,
                            g: self.scene.palette.sky[1] as f64,
                            b: self.scene.palette.sky[2] as f64,
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
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn build_vertices(&self) -> Vec<Vertex> {
        let mut mesh = Vec::with_capacity(16_384);
        let half = self.scene.ground_size * 0.5;
        add_cuboid(
            &mut mesh,
            Vec3::new(0.0, -0.08, 0.0),
            Vec3::new(self.scene.ground_size, 0.16, self.scene.ground_size),
            self.scene.palette.ground,
        );
        for block in &self.scene.blocks {
            add_cuboid(
                &mut mesh,
                Vec3::from_array(block.position),
                Vec3::from_array(block.size),
                block.color,
            );
        }
        for pad in &self.scene.pads {
            add_cylinder(
                &mut mesh,
                Vec3::new(pad.x, 0.10, pad.z),
                pad.radius,
                0.12,
                pad.color,
            );
        }
        for agent in &self.scene.agents {
            add_avatar(&mut mesh, *agent, self.scene.palette.ink);
        }
        add_avatar(&mut mesh, self.scene.player, self.scene.palette.ink);
        let grid_step = self.scene.ground_size / 12.0;
        for index in 0..=12 {
            let offset = -half + index as f32 * grid_step;
            add_cuboid(
                &mut mesh,
                Vec3::new(offset, 0.015, 0.0),
                Vec3::new(0.018, 0.025, self.scene.ground_size),
                self.scene.palette.grid,
            );
            add_cuboid(
                &mut mesh,
                Vec3::new(0.0, 0.016, offset),
                Vec3::new(self.scene.ground_size, 0.026, 0.018),
                self.scene.palette.grid,
            );
        }
        mesh
    }

    fn ensure_vertex_capacity(&mut self, required: usize) {
        if required <= self.vertex_capacity {
            return;
        }
        self.vertex_capacity = required.next_power_of_two();
        self.vertex_buffer = create_vertex_buffer(&self.device, self.vertex_capacity);
    }
}

fn add_cuboid(vertices: &mut Vec<Vertex>, center: Vec3, size: Vec3, color: [f32; 4]) {
    add_transformed_cuboid(vertices, Mat4::from_translation(center), size, color);
}

fn add_transformed_cuboid(
    vertices: &mut Vec<Vertex>,
    transform: Mat4,
    size: Vec3,
    color: [f32; 4],
) {
    let half = size * 0.5;
    let corners = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ]
    .map(|corner| transform.transform_point3(corner));
    let normal = |direction: Vec3| transform.transform_vector3(direction).normalize_or_zero();
    add_quad(
        vertices,
        corners[0],
        corners[1],
        corners[2],
        corners[3],
        normal(Vec3::NEG_Z),
        color,
    );
    add_quad(
        vertices,
        corners[5],
        corners[4],
        corners[7],
        corners[6],
        normal(Vec3::Z),
        color,
    );
    add_quad(
        vertices,
        corners[1],
        corners[5],
        corners[6],
        corners[2],
        normal(Vec3::X),
        color,
    );
    add_quad(
        vertices,
        corners[4],
        corners[0],
        corners[3],
        corners[7],
        normal(Vec3::NEG_X),
        color,
    );
    add_quad(
        vertices,
        corners[3],
        corners[2],
        corners[6],
        corners[7],
        normal(Vec3::Y),
        color,
    );
    add_quad(
        vertices,
        corners[4],
        corners[5],
        corners[1],
        corners[0],
        normal(Vec3::NEG_Y),
        color,
    );
}

fn add_avatar(vertices: &mut Vec<Vertex>, agent: RenderAgent, face_color: [f32; 4]) {
    let mut shadow_color = face_color;
    shadow_color[3] = 0.14;
    add_cylinder(
        vertices,
        Vec3::new(agent.position[0], 0.018, agent.position[2]),
        0.72,
        0.025,
        shadow_color,
    );
    let root = Mat4::from_translation(Vec3::from_array(agent.position))
        * Mat4::from_quat(Quat::from_rotation_y(agent.yaw));
    let stride = if agent.assembled > 0.5 {
        0.03
    } else {
        agent.walk_cycle.sin() * 0.5
    };
    let bob = if agent.position[1] <= 0.01 {
        agent.walk_cycle.sin().abs() * 0.025
    } else {
        0.0
    };

    let mut part = |position: Vec3, size: Vec3, pitch: f32, color: [f32; 4]| {
        let transform =
            root * Mat4::from_translation(position) * Mat4::from_quat(Quat::from_rotation_x(pitch));
        add_transformed_cuboid(vertices, transform, size, color);
    };

    part(
        Vec3::new(0.0, 1.82 + bob, 0.0),
        Vec3::new(1.1, 1.25, 0.64),
        0.0,
        agent.shirt,
    );
    part(
        Vec3::new(0.0, 3.01, 0.0),
        Vec3::splat(0.84),
        0.0,
        agent.skin,
    );
    part(
        Vec3::new(-0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        stride,
        agent.shirt,
    );
    part(
        Vec3::new(0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        -stride,
        agent.shirt,
    );
    part(
        Vec3::new(-0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        -stride,
        agent.pants,
    );
    part(
        Vec3::new(0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        stride,
        agent.pants,
    );
    part(
        Vec3::new(-0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        agent.shoes,
    );
    part(
        Vec3::new(0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        agent.shoes,
    );
    part(
        Vec3::new(-0.16, 3.04, -0.43),
        Vec3::new(0.1, 0.12, 0.03),
        0.0,
        face_color,
    );
    part(
        Vec3::new(0.16, 3.04, -0.43),
        Vec3::new(0.1, 0.12, 0.03),
        0.0,
        face_color,
    );
}

fn add_cylinder(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    height: f32,
    color: [f32; 4],
) {
    let segments = 24;
    for index in 0..segments {
        let next = (index + 1) % segments;
        let a = index as f32 / segments as f32 * std::f32::consts::TAU;
        let b = next as f32 / segments as f32 * std::f32::consts::TAU;
        let bottom_a = center + Vec3::new(a.cos() * radius, -height / 2.0, a.sin() * radius);
        let bottom_b = center + Vec3::new(b.cos() * radius, -height / 2.0, b.sin() * radius);
        let top_a = center + Vec3::new(a.cos() * radius, height / 2.0, a.sin() * radius);
        let top_b = center + Vec3::new(b.cos() * radius, height / 2.0, b.sin() * radius);
        add_quad(
            vertices,
            bottom_a,
            bottom_b,
            top_b,
            top_a,
            Vec3::new(a.cos(), 0.0, a.sin()),
            color,
        );
        add_triangle(
            vertices,
            center + Vec3::new(0.0, height / 2.0, 0.0),
            top_a,
            top_b,
            Vec3::Y,
            color,
        );
    }
}

fn add_quad(
    vertices: &mut Vec<Vertex>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
    normal: Vec3,
    color: [f32; 4],
) {
    add_triangle(vertices, a, b, c, normal, color);
    add_triangle(vertices, a, c, d, normal, color);
}

fn add_triangle(
    vertices: &mut Vec<Vertex>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    normal: Vec3,
    color: [f32; 4],
) {
    let normal = normal.to_array();
    vertices.extend([
        Vertex {
            position: a.to_array(),
            normal,
            color,
        },
        Vertex {
            position: b.to_array(),
            normal,
            color,
        },
        Vertex {
            position: c.to_array(),
            normal,
            color,
        },
    ]);
}

fn create_vertex_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cubacadabra vertices"),
        size: (capacity * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("cubacadabra depth"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}
