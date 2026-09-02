#[cfg(not(target_arch = "wasm32"))]
use std::ffi::c_void;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::engine::{Engine, SNAPSHOT_STRIDE};
use crate::game_package::{AvatarDefinition, GamePackageDefinition, WorldDefinition};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Clone)]
struct RenderBlock {
    position: [f32; 3],
    size: [f32; 3],
    color: [f32; 4],
    outline: bool,
}

#[derive(Clone)]
struct RenderPad {
    x: f32,
    z: f32,
    radius: f32,
    code: String,
    label: String,
    color: [f32; 4],
}

#[derive(Clone, Copy, Default)]
struct RenderEntity {
    position: [f32; 3],
    yaw: f32,
    walk_cycle: f32,
    assembled: f32,
}

#[derive(Clone, Copy)]
struct AvatarStyle {
    skin: [f32; 4],
    shirt: [f32; 4],
    pants: [f32; 4],
    shoes: [f32; 4],
}

#[derive(Clone, Copy)]
struct RenderPalette {
    sky: [f32; 4],
    ground: [f32; 4],
    ground_edge: [f32; 4],
    grid: [f32; 4],
    ink: [f32; 4],
    paper: [f32; 4],
}

impl Default for RenderPalette {
    fn default() -> Self {
        Self {
            sky: color(0x9ab9be),
            ground: color(0xa7bd99),
            ground_edge: color(0x587276),
            grid: color(0xc4d5cf),
            ink: color(0x173f43),
            paper: color(0xf6f1e7),
        }
    }
}

#[derive(Clone)]
struct RenderCloud {
    position: [f32; 3],
    scale: f32,
}

#[derive(Clone)]
struct RenderWorld {
    blocks: Vec<RenderBlock>,
    pads: Vec<RenderPad>,
    clouds: Vec<RenderCloud>,
    ground_size: f32,
    grid_size: f32,
    grid_divisions: usize,
    spawn: [f32; 3],
    show_spawn_pad: bool,
    palette: RenderPalette,
}

impl Default for RenderWorld {
    fn default() -> Self {
        Self {
            blocks: Vec::new(),
            pads: Vec::new(),
            clouds: Vec::new(),
            ground_size: 120.0,
            grid_size: 112.0,
            grid_divisions: 28,
            spawn: [0.0; 3],
            show_spawn_pad: true,
            palette: RenderPalette::default(),
        }
    }
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

struct Scene {
    world: RenderWorld,
    agents: Vec<RenderEntity>,
    player: RenderEntity,
    pad_seconds: Vec<f32>,
    player_style: AvatarStyle,
    npc_styles: Vec<AvatarStyle>,
    camera: [f32; 3],
    elapsed: f32,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: RenderWorld::default(),
            agents: Vec::new(),
            player: RenderEntity::default(),
            pad_seconds: Vec::new(),
            player_style: default_player_style(),
            npc_styles: default_npc_styles(),
            camera: [0.0, -0.095, 8.0],
            elapsed: 0.0,
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    static_vertex_buffer: wgpu::Buffer,
    static_vertex_capacity: usize,
    static_vertex_count: usize,
    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_vertex_capacity: usize,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    width: f32,
    height: f32,
    scene: Scene,
    package_generation: u32,
    active_world: usize,
    worlds: Vec<RenderWorld>,
}

impl Renderer {
    #[cfg(not(target_arch = "wasm32"))]
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
        Some(Self::from_parts(
            surface, adapter, device, queue, width, height,
        ))
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn new_web(
        canvas: web_sys::HtmlCanvasElement,
        width: f32,
        height: f32,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        if width <= 0.0 || height <= 0.0 {
            return Err(wasm_bindgen::JsValue::from_str(
                "The renderer size must be positive.",
            ));
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cubacadabra browser device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;

        Ok(Self::from_parts(
            surface, adapter, device, queue, width, height,
        ))
    }

    fn from_parts(
        surface: wgpu::Surface<'static>,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: f32,
        height: f32,
    ) -> Self {
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
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
        let static_vertex_capacity = 16_384;
        let dynamic_vertex_capacity = 16_384;
        let static_vertex_buffer = create_vertex_buffer(&device, static_vertex_capacity);
        let dynamic_vertex_buffer = create_vertex_buffer(&device, dynamic_vertex_capacity);
        let depth_view = create_depth_view(&device, config.width, config.height);

        Self {
            surface,
            device,
            queue,
            pipeline,
            globals_buffer,
            globals_bind_group,
            static_vertex_buffer,
            static_vertex_capacity,
            static_vertex_count: 0,
            dynamic_vertex_buffer,
            dynamic_vertex_capacity,
            config,
            depth_view,
            width,
            height,
            scene: Scene::default(),
            package_generation: 0,
            active_world: usize::MAX,
            worlds: Vec::new(),
        }
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

    pub fn sync_engine(&mut self, engine: &Engine) {
        if self.package_generation != engine.package_generation {
            self.worlds = engine
                .package
                .as_ref()
                .map(|package| {
                    let (player_style, npc_styles) = resolve_avatar_styles(package);
                    self.scene.player_style = player_style;
                    self.scene.npc_styles = npc_styles;
                    package
                        .world_entries()
                        .into_iter()
                        .map(|(_, world)| resolve_world(&world))
                        .collect()
                })
                .unwrap_or_default();
            self.package_generation = engine.package_generation;
            self.active_world = usize::MAX;
        }

        if self.active_world != engine.active_world
            && let Some(world) = self.worlds.get(engine.active_world).cloned()
        {
            self.active_world = engine.active_world;
            self.scene.world = world;
            self.rebuild_static_vertices();
        }

        let snapshot = engine.snapshot();
        self.scene.player = render_entity(snapshot.get(..SNAPSHOT_STRIDE).unwrap_or(&[]));
        self.scene.agents.clear();
        self.scene.agents.extend(
            snapshot
                .as_chunks::<SNAPSHOT_STRIDE>()
                .0
                .iter()
                .skip(1)
                .take(engine.agent_count())
                .map(|values| render_entity(values)),
        );
        self.scene.pad_seconds.clear();
        self.scene
            .pad_seconds
            .extend((0..engine.launch_pad_count()).map(|index| engine.launch_pad_seconds(index)));
        self.scene.camera = engine.camera();
        self.scene.elapsed = engine.elapsed();
    }

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
                    (distance * pitch.sin()).clamp(-2.0, 7.0),
                    yaw.cos() * horizontal_distance,
                );
            (camera_position, target)
        };
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
            fog_color: self.scene.world.palette.sky,
        };
        let dynamic_vertices = self.build_dynamic_vertices();
        self.ensure_dynamic_vertex_capacity(dynamic_vertices.len());
        if !dynamic_vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                0,
                bytemuck::cast_slice(&dynamic_vertices),
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
        self.queue.submit(Some(encoder.finish()));
        frame.present();
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

    fn build_dynamic_vertices(&self) -> Vec<Vertex> {
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
        for (index, agent) in self.scene.agents.iter().enumerate() {
            let style = self.scene.npc_styles[index % self.scene.npc_styles.len()];
            add_avatar(&mut mesh, *agent, style, world.palette.ink);
        }
        if self.scene.camera[2] > 0.75 {
            add_avatar(
                &mut mesh,
                self.scene.player,
                self.scene.player_style,
                world.palette.ink,
            );
        }
        mesh
    }

    fn rebuild_static_vertices(&mut self) {
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
        self.static_vertex_buffer = create_vertex_buffer(&self.device, self.static_vertex_capacity);
    }

    fn ensure_dynamic_vertex_capacity(&mut self, required: usize) {
        if required <= self.dynamic_vertex_capacity {
            return;
        }
        self.dynamic_vertex_capacity = required.next_power_of_two();
        self.dynamic_vertex_buffer =
            create_vertex_buffer(&self.device, self.dynamic_vertex_capacity);
    }
}

fn render_entity(values: &[f32]) -> RenderEntity {
    let value = |index: usize| values.get(index).copied().unwrap_or(0.0);
    RenderEntity {
        position: [value(0), value(1), value(2)],
        yaw: value(3),
        walk_cycle: value(4),
        assembled: value(7),
    }
}

fn resolve_world(definition: &WorldDefinition) -> RenderWorld {
    let defaults = RenderPalette::default();
    let palette = RenderPalette {
        sky: resolve_color(&definition.palette, "sky", defaults.sky),
        ground: resolve_color(&definition.palette, "ground", defaults.ground),
        ground_edge: resolve_color(&definition.palette, "groundEdge", defaults.ground_edge),
        grid: resolve_color(&definition.palette, "grid", defaults.grid),
        ink: resolve_color(&definition.palette, "ink", defaults.ink),
        paper: resolve_color(&definition.palette, "paper", defaults.paper),
    };
    RenderWorld {
        blocks: definition
            .blocks
            .iter()
            .map(|block| RenderBlock {
                position: block.position(),
                size: block.size(),
                color: resolve_color(&definition.palette, &block.color, color(0xffffff)),
                outline: block.outline,
            })
            .collect(),
        pads: definition
            .launch_pads
            .iter()
            .map(|pad| RenderPad {
                x: pad.x(),
                z: pad.z(),
                radius: pad.radius.max(0.2),
                code: pad.code.clone(),
                label: pad.label.clone(),
                color: resolve_color(&definition.palette, &pad.color, palette.paper),
            })
            .collect(),
        clouds: definition
            .world
            .clouds
            .iter()
            .map(|cloud| RenderCloud {
                position: cloud.position(),
                scale: cloud.scale.max(0.1),
            })
            .collect(),
        ground_size: definition.world.ground_size.max(10.0),
        grid_size: definition.world.grid_size.max(1.0),
        grid_divisions: definition.world.grid_divisions,
        spawn: definition.world.spawn(),
        show_spawn_pad: definition.world.show_spawn_pad,
        palette,
    }
}

fn resolve_avatar_styles(package: &GamePackageDefinition) -> (AvatarStyle, Vec<AvatarStyle>) {
    let player = package
        .avatars
        .player
        .as_ref()
        .map_or_else(default_player_style, |style| {
            resolve_avatar_style(style, default_player_style())
        });
    let npcs = if package.avatars.npcs.is_empty() {
        default_npc_styles()
    } else {
        let defaults = default_npc_styles();
        package
            .avatars
            .npcs
            .iter()
            .enumerate()
            .map(|(index, style)| resolve_avatar_style(style, defaults[index % defaults.len()]))
            .collect()
    };
    (player, npcs)
}

fn resolve_avatar_style(definition: &AvatarDefinition, fallback: AvatarStyle) -> AvatarStyle {
    AvatarStyle {
        skin: definition
            .skin
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.skin),
        shirt: definition
            .shirt
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shirt),
        pants: definition
            .pants
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.pants),
        shoes: definition
            .shoes
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shoes),
    }
}

fn default_player_style() -> AvatarStyle {
    AvatarStyle {
        skin: color(0xe8ae86),
        shirt: color(0x2d6663),
        pants: color(0x536a90),
        shoes: color(0x293a43),
    }
}

fn default_npc_styles() -> Vec<AvatarStyle> {
    [
        (0xf0b18a, 0xe76f51, 0x355070),
        (0xd99770, 0x5f8f78, 0x3e5974),
        (0xf4c39f, 0x748bd2, 0x43515e),
        (0xc98263, 0xf0b54d, 0x385c62),
        (0xe4a77b, 0xb276a9, 0x4b5e80),
        (0xf1c29b, 0x3f8884, 0x414b5b),
    ]
    .map(|(skin, shirt, pants)| AvatarStyle {
        skin: color(skin),
        shirt: color(shirt),
        pants: color(pants),
        shoes: color(0x293a43),
    })
    .to_vec()
}

fn resolve_color(
    palette: &std::collections::BTreeMap<String, String>,
    token: &str,
    fallback: [f32; 4],
) -> [f32; 4] {
    palette
        .get(token)
        .map(String::as_str)
        .or_else(|| token.starts_with('#').then_some(token))
        .and_then(parse_hex_color)
        .unwrap_or(fallback)
}

fn parse_hex_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    u32::from_str_radix(value, 16).ok().map(color)
}

fn color(value: u32) -> [f32; 4] {
    [
        ((value >> 16) & 0xff) as f32 / 255.0,
        ((value >> 8) & 0xff) as f32 / 255.0,
        (value & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn faded(mut color: [f32; 4], alpha: f32) -> [f32; 4] {
    color[3] = alpha;
    color
}

fn add_cuboid(vertices: &mut Vec<Vertex>, center: Vec3, size: Vec3, color: [f32; 4]) {
    add_transformed_cuboid(vertices, Mat4::from_translation(center), size, color);
}

fn add_cuboid_outline(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    size: Vec3,
    thickness: f32,
    color: [f32; 4],
) {
    let half = size * 0.5;
    for y in [-half.y, half.y] {
        for z in [-half.z, half.z] {
            add_cuboid(
                vertices,
                center + Vec3::new(0.0, y, z),
                Vec3::new(size.x + thickness, thickness, thickness),
                color,
            );
        }
    }
    for x in [-half.x, half.x] {
        for z in [-half.z, half.z] {
            add_cuboid(
                vertices,
                center + Vec3::new(x, 0.0, z),
                Vec3::new(thickness, size.y + thickness, thickness),
                color,
            );
        }
    }
    for x in [-half.x, half.x] {
        for y in [-half.y, half.y] {
            add_cuboid(
                vertices,
                center + Vec3::new(x, y, 0.0),
                Vec3::new(thickness, thickness, size.z + thickness),
                color,
            );
        }
    }
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

fn add_avatar(
    vertices: &mut Vec<Vertex>,
    agent: RenderEntity,
    style: AvatarStyle,
    face_color: [f32; 4],
) {
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
        style.shirt,
    );
    part(
        Vec3::new(0.0, 3.01, 0.0),
        Vec3::splat(0.84),
        0.0,
        style.skin,
    );
    part(
        Vec3::new(-0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        stride,
        style.shirt,
    );
    part(
        Vec3::new(0.76, 1.84, 0.0),
        Vec3::new(0.36, 1.15, 0.45),
        -stride,
        style.shirt,
    );
    part(
        Vec3::new(-0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        -stride,
        style.pants,
    );
    part(
        Vec3::new(0.28, 0.62, 0.0),
        Vec3::new(0.47, 1.25, 0.55),
        stride,
        style.pants,
    );
    part(
        Vec3::new(-0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        style.shoes,
    );
    part(
        Vec3::new(0.28, 0.11, -0.06),
        Vec3::new(0.56, 0.22, 0.7),
        0.0,
        style.shoes,
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

fn add_spawn_pad(vertices: &mut Vec<Vertex>, origin: Vec3, palette: RenderPalette, elapsed: f32) {
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.08, 0.0),
        2.35,
        0.16,
        palette.ink,
    );
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.18, 0.0),
        1.9,
        0.08,
        faded(palette.paper, 0.46),
    );
    add_ring(
        vertices,
        origin + Vec3::new(0.0, 0.24, 0.0),
        1.55,
        0.13,
        palette.ground_edge,
    );
    let rotation = Quat::from_rotation_y(elapsed * 0.22);
    for angle in [0.0_f32, std::f32::consts::FRAC_PI_2] {
        let transform = Mat4::from_translation(origin + Vec3::new(0.0, 0.30, 0.0))
            * Mat4::from_quat(rotation * Quat::from_rotation_y(angle));
        add_transformed_cuboid(
            vertices,
            transform,
            Vec3::new(2.45, 0.045, 0.12),
            faded(palette.paper, 0.62),
        );
    }
}

fn add_cloud(
    vertices: &mut Vec<Vertex>,
    cloud: &RenderCloud,
    index: usize,
    paper: [f32; 4],
    elapsed: f32,
) {
    let drift =
        (elapsed * (0.02 + index as f32 * 0.003) + index as f32).sin() * (1.2 + index as f32 * 0.2);
    let origin = Vec3::from_array(cloud.position) + Vec3::new(drift, 0.0, 0.0);
    let cloud_color = faded(paper, 0.52);
    for (offset, radius) in [
        (Vec3::new(-1.1, 0.0, 0.0), 1.1),
        (Vec3::new(0.0, 0.24, 0.0), 1.45),
        (Vec3::new(1.1, 0.02, 0.0), 0.92),
        (Vec3::new(0.42, -0.15, 0.08), 1.05),
    ] {
        add_sphere(
            vertices,
            origin + offset * cloud.scale,
            radius * cloud.scale,
            cloud_color,
        );
    }
}

fn add_launch_pad(
    vertices: &mut Vec<Vertex>,
    pad: &RenderPad,
    seconds: f32,
    palette: RenderPalette,
    elapsed: f32,
    index: usize,
) {
    let origin = Vec3::new(pad.x, 0.0, pad.z);
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.10, 0.0),
        pad.radius + 0.45,
        0.20,
        palette.ink,
    );

    let mut inner = pad.color;
    inner[3] = 0.30;
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.215, 0.0),
        pad.radius,
        0.025,
        inner,
    );
    let pulse = 1.0 + (elapsed * 2.2 + index as f32 * 1.7).sin() * 0.045;
    let mut ring_color = pad.color;
    if seconds > 0.0 {
        ring_color[3] = 0.82 + (elapsed * 5.0).sin().abs() * 0.18;
    }
    add_ring(
        vertices,
        origin + Vec3::new(0.0, 0.28, 0.0),
        (pad.radius - 0.15).max(0.2) * pulse,
        0.11,
        ring_color,
    );

    for (beacon_index, x) in [-2.35, 2.35].into_iter().enumerate() {
        let y = 1.35 + (elapsed * 2.6 + index as f32 + beacon_index as f32).sin() * 0.12;
        add_cylinder(
            vertices,
            origin + Vec3::new(x, y, -0.35),
            0.19,
            2.70,
            pad.color,
        );
    }
    add_cuboid(
        vertices,
        origin + Vec3::new(0.0, 2.62, -0.35),
        Vec3::new(5.05, 0.32, 0.38),
        pad.color,
    );
    add_cuboid(
        vertices,
        origin + Vec3::new(0.0, 4.0, -0.35),
        Vec3::new(5.05, 0.82, 0.22),
        palette.ink,
    );
    add_cuboid(
        vertices,
        origin + Vec3::new(-2.42, 4.0, -0.23),
        Vec3::new(0.12, 0.82, 0.08),
        pad.color,
    );
    let label = format!("{} {}", pad.code, pad.label);
    add_pixel_text(
        vertices,
        label.trim(),
        origin + Vec3::new(0.08, 4.0, -0.225),
        0.0,
        4.55,
        palette.paper,
    );
    add_pixel_text(
        vertices,
        label.trim(),
        origin + Vec3::new(-0.08, 4.0, -0.475),
        std::f32::consts::PI,
        4.55,
        palette.paper,
    );
}

fn add_pixel_text(
    vertices: &mut Vec<Vertex>,
    text: &str,
    origin: Vec3,
    yaw: f32,
    max_width: f32,
    color: [f32; 4],
) {
    let characters = text
        .chars()
        .filter(|character| character.is_ascii())
        .map(|character| character.to_ascii_uppercase())
        .take(24)
        .collect::<Vec<_>>();
    if characters.is_empty() {
        return;
    }
    let columns = characters.len() * 6 - 1;
    let pixel = (max_width / columns as f32).min(0.072);
    let text_width = columns as f32 * pixel;
    let root = Mat4::from_translation(origin) * Mat4::from_quat(Quat::from_rotation_y(yaw));
    for (character_index, character) in characters.into_iter().enumerate() {
        let glyph = glyph(character);
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let x =
                    character_index as f32 * pixel * 6.0 + column as f32 * pixel - text_width * 0.5;
                let y = (3.0 - row as f32) * pixel;
                add_transformed_cuboid(
                    vertices,
                    root * Mat4::from_translation(Vec3::new(x, y, 0.0)),
                    Vec3::new(pixel * 0.82, pixel * 0.82, 0.035),
                    color,
                );
            }
        }
    }
}

fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}

fn add_ring(
    vertices: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    thickness: f32,
    color: [f32; 4],
) {
    let inner_radius = (radius - thickness).max(0.05);
    let segments = 32;
    for index in 0..segments {
        let next = (index + 1) % segments;
        let a = index as f32 / segments as f32 * std::f32::consts::TAU;
        let b = next as f32 / segments as f32 * std::f32::consts::TAU;
        let outer_a = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
        let outer_b = center + Vec3::new(b.cos() * radius, 0.0, b.sin() * radius);
        let inner_a = center + Vec3::new(a.cos() * inner_radius, 0.0, a.sin() * inner_radius);
        let inner_b = center + Vec3::new(b.cos() * inner_radius, 0.0, b.sin() * inner_radius);
        add_quad(vertices, outer_a, outer_b, inner_b, inner_a, Vec3::Y, color);
    }
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

fn add_sphere(vertices: &mut Vec<Vertex>, center: Vec3, radius: f32, color: [f32; 4]) {
    let latitude_segments = 6;
    let longitude_segments = 12;
    let point = |latitude: usize, longitude: usize| {
        let vertical = latitude as f32 / latitude_segments as f32;
        let horizontal = longitude as f32 / longitude_segments as f32;
        let phi = vertical * std::f32::consts::PI;
        let theta = horizontal * std::f32::consts::TAU;
        Vec3::new(theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin())
    };
    for latitude in 0..latitude_segments {
        for longitude in 0..longitude_segments {
            let next = (longitude + 1) % longitude_segments;
            let normal_a = point(latitude, longitude);
            let normal_b = point(latitude + 1, longitude);
            let normal_c = point(latitude + 1, next);
            let normal_d = point(latitude, next);
            add_triangle(
                vertices,
                center + normal_a * radius,
                center + normal_b * radius,
                center + normal_c * radius,
                (normal_a + normal_b + normal_c).normalize_or_zero(),
                color,
            );
            add_triangle(
                vertices,
                center + normal_a * radius,
                center + normal_c * radius,
                center + normal_d * radius,
                (normal_a + normal_c + normal_d).normalize_or_zero(),
                color,
            );
        }
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
