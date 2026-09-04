#[cfg(not(target_arch = "wasm32"))]
use std::ffi::c_void;
use std::io::Cursor;
#[cfg(target_os = "android")]
use std::ptr::NonNull;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use super::{
    DEPTH_FORMAT, Globals, Renderer, Vertex,
    ui::{UI_ATLAS_HEIGHT, UI_ATLAS_PADDING, UI_ATLAS_WIDTH, UI_FONT_ATLAS_Y, ui_atlas_glyphs},
};

const UI_LOGO_BYTES: &[u8] = include_bytes!("../../assets/images/logo.png");
const UI_CUBE_BYTES: &[u8] = include_bytes!("../../assets/images/cube.png");
const UI_CHAT_BYTES: &[u8] = include_bytes!("../../assets/images/chat.png");
const UI_VOICE_BYTES: &[u8] = include_bytes!("../../assets/images/voice.png");

fn decode_ui_image(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .expect("bundled UI image should have a readable PNG header");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut pixels)
        .expect("bundled UI image should decode");
    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "bundled UI image should use RGBA pixels"
    );
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

fn create_ui_texture_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let images = [
        decode_ui_image(UI_LOGO_BYTES),
        decode_ui_image(UI_CUBE_BYTES),
        decode_ui_image(UI_CHAT_BYTES),
        decode_ui_image(UI_VOICE_BYTES),
    ];
    assert_eq!(images[0].0, 1206);
    assert_eq!(images[0].1, 1206);
    assert!(
        images[1..]
            .iter()
            .all(|(width, height, _)| *width == 512 && *height == 512)
    );

    let mut atlas = vec![0_u8; (UI_ATLAS_WIDTH * UI_ATLAS_HEIGHT * 4) as usize];
    let mut x = UI_ATLAS_PADDING;
    for (width, height, pixels) in &images {
        for row in 0..*height as usize {
            let src_start = row * *width as usize * 4;
            let dst_start =
                ((UI_ATLAS_PADDING as usize + row) * UI_ATLAS_WIDTH as usize + x as usize) * 4;
            let length = *width as usize * 4;
            atlas[dst_start..dst_start + length]
                .copy_from_slice(&pixels[src_start..src_start + length]);
        }
        x += *width + UI_ATLAS_PADDING * 2;
    }

    for glyph in ui_atlas_glyphs() {
        let width = glyph.metrics.width;
        let height = glyph.metrics.height;
        for row in 0..height {
            for column in 0..width {
                let coverage = glyph.bitmap[row * width + column];
                let index = ((UI_FONT_ATLAS_Y as usize + row) * UI_ATLAS_WIDTH as usize
                    + glyph.x as usize
                    + column)
                    * 4;
                atlas[index..index + 4].copy_from_slice(&[255, 255, 255, coverage]);
            }
        }
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cubacadabra shared UI image atlas"),
        size: wgpu::Extent3d {
            width: UI_ATLAS_WIDTH,
            height: UI_ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // The native surface is an unorm target and the rest of the UI uses
        // unencoded color values. Keeping this texture unorm preserves the
        // authored orange in logo.png instead of applying an extra sRGB decode.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &atlas,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(UI_ATLAS_WIDTH * 4),
            rows_per_image: Some(UI_ATLAS_HEIGHT),
        },
        wgpu::Extent3d {
            width: UI_ATLAS_WIDTH,
            height: UI_ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cubacadabra shared UI image sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    (view, sampler)
}

impl Renderer {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(layer: *mut c_void, width: f32, height: f32) -> Option<Self> {
        if layer.is_null() || width <= 0.0 || height <= 0.0 {
            return None;
        }

        #[cfg(target_os = "android")]
        let (instance, surface) = {
            let window = NonNull::new(layer)?;
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                // Android Studio emulators commonly expose a software Vulkan
                // device that can be selected before the emulator's GLES
                // device. GLES is the more reliable native SurfaceView path
                // across emulator graphics modes and is already compiled in
                // by the Android build.
                backends: wgpu::Backends::GL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let window_handle = raw_window_handle::AndroidNdkWindowHandle::new(window);
            let display_handle = raw_window_handle::AndroidDisplayHandle::new();
            let surface = match unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                        display_handle,
                    )),
                    raw_window_handle: raw_window_handle::RawWindowHandle::AndroidNdk(
                        window_handle,
                    ),
                })
            } {
                Ok(surface) => surface,
                Err(error) => {
                    eprintln!("[RustRenderer] Android GLES surface creation failed: {error}");
                    return None;
                }
            };
            (instance, surface)
        };

        #[cfg(not(target_os = "android"))]
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        #[cfg(not(target_os = "android"))]
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
                .ok()?
        };
        let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })) {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("[RustRenderer] hardware GLES adapter unavailable: {error}");
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: true,
                }))
                .map_err(|fallback_error| {
                    eprintln!("[RustRenderer] fallback GLES adapter unavailable: {fallback_error}");
                    fallback_error
                })
                .ok()?
            }
        };
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cubacadabra game device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| {
            eprintln!("[RustRenderer] GLES device creation failed: {error}");
            error
        })
        .ok()?;
        Some(Self::from_parts(
            surface, adapter, device, queue, width, height, false,
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
            surface, adapter, device, queue, width, height, true,
        ))
    }

    fn from_parts(
        surface: wgpu::Surface<'static>,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: f32,
        height: f32,
        prefer_srgb: bool,
    ) -> Self {
        let capabilities = surface.get_capabilities(&adapter);
        let format = if prefer_srgb {
            capabilities
                .formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
                .or_else(|| capabilities.formats.first().copied())
        } else {
            capabilities
                .formats
                .iter()
                .copied()
                .find(|format| !format.is_srgb())
                .or_else(|| capabilities.formats.first().copied())
        }
        .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
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
            source: wgpu::ShaderSource::Wgsl(include_str!("../renderer.wgsl").into()),
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
        let ui_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("cubacadabra UI texture layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let ui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cubacadabra UI pipeline layout"),
            bind_group_layouts: &[Some(&ui_texture_bind_group_layout)],
            immediate_size: 0,
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cubacadabra UI pipeline"),
            layout: Some(&ui_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ui"),
                compilation_options: Default::default(),
                buffers: &[Vertex::LAYOUT],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ui"),
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
        let (ui_texture_view, ui_sampler) = create_ui_texture_atlas(&device, &queue);
        let ui_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cubacadabra UI texture bind group"),
            layout: &ui_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ui_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&ui_sampler),
                },
            ],
        });
        let static_vertex_capacity = 16_384;
        let dynamic_vertex_capacity = 16_384;
        let ui_vertex_capacity = 8_192;
        let static_vertex_buffer = create_vertex_buffer(&device, static_vertex_capacity);
        let dynamic_vertex_buffer = create_vertex_buffer(&device, dynamic_vertex_capacity);
        let ui_vertex_buffer = create_vertex_buffer(&device, ui_vertex_capacity);
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
            ui_pipeline,
            ui_texture_bind_group,
            ui_vertex_buffer,
            ui_vertex_capacity,
            config,
            depth_view,
            width,
            height,
            scene: super::Scene::default(),
            package_generation: 0,
            active_world: usize::MAX,
            worlds: Vec::new(),
            ui_frame: Default::default(),
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
}

pub(super) fn create_vertex_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
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
