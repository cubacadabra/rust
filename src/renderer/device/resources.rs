use super::*;

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

fn decode_terrain_image(bytes: &[u8]) -> Vec<u8> {
    let (width, height, pixels) = decode_ui_image(bytes);
    assert_eq!(
        width, TERRAIN_TILE_SIZE,
        "terrain tiles must be 512 pixels wide"
    );
    assert_eq!(
        height, TERRAIN_TILE_SIZE,
        "terrain tiles must be 512 pixels high"
    );
    pixels
}

fn downsample_terrain_image(width: usize, pixels: &[u8]) -> Vec<u8> {
    let next_width = width / 2;
    let mut result = vec![0; next_width * next_width * 4];
    for y in 0..next_width {
        for x in 0..next_width {
            let destination = (y * next_width + x) * 4;
            let samples = [
                ((y * 2) * width + x * 2) * 4,
                ((y * 2) * width + x * 2 + 1) * 4,
                ((y * 2 + 1) * width + x * 2) * 4,
                ((y * 2 + 1) * width + x * 2 + 1) * 4,
            ];
            for channel in 0..4 {
                let total = samples
                    .iter()
                    .map(|sample| u16::from(pixels[sample + channel]))
                    .sum::<u16>();
                result[destination + channel] = (total / 4) as u8;
            }
        }
    }
    result
}

pub fn terrain_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cubacadabra built-in terrain material layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
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
    })
}

pub fn create_terrain_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::BindGroup {
    let mut layers = [
        decode_terrain_image(TERRAIN_GRASS_TOP_BYTES),
        decode_terrain_image(TERRAIN_GRASS_SIDE_BYTES),
        decode_terrain_image(TERRAIN_GROUND_BYTES),
        decode_terrain_image(TERRAIN_ROCK_BYTES),
        decode_terrain_image(TERRAIN_SAND_BYTES),
        decode_terrain_image(TERRAIN_MUD_BYTES),
        decode_terrain_image(TERRAIN_SNOW_BYTES),
    ];
    let mip_level_count = TERRAIN_TILE_SIZE.ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cubacadabra built-in terrain material maps"),
        size: wgpu::Extent3d {
            width: TERRAIN_TILE_SIZE,
            height: TERRAIN_TILE_SIZE,
            depth_or_array_layers: TERRAIN_LAYER_COUNT,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut width = TERRAIN_TILE_SIZE as usize;
    for mip_level in 0..mip_level_count {
        let mut pixels = Vec::with_capacity(width * width * 4 * TERRAIN_LAYER_COUNT as usize);
        for layer in &layers {
            pixels.extend_from_slice(layer);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32 * 4),
                rows_per_image: Some(width as u32),
            },
            wgpu::Extent3d {
                width: width as u32,
                height: width as u32,
                depth_or_array_layers: TERRAIN_LAYER_COUNT,
            },
        );
        if width > 1 {
            layers = layers.map(|layer| downsample_terrain_image(width, &layer));
            width /= 2;
        }
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("cubacadabra built-in terrain material array view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        base_mip_level: 0,
        mip_level_count: Some(mip_level_count),
        base_array_layer: 0,
        array_layer_count: Some(TERRAIN_LAYER_COUNT),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cubacadabra built-in terrain material sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cubacadabra built-in terrain material maps"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
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
    for glyph in world_label_atlas_glyphs() {
        let width = glyph.metrics.width;
        let height = glyph.metrics.height;
        for row in 0..height {
            for column in 0..width {
                let coverage = glyph.bitmap[row * width + column];
                let index = ((WORLD_LABEL_FONT_ATLAS_Y as usize + row) * UI_ATLAS_WIDTH as usize
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
        // UI colors and atlas texels use authored display-encoded values in
        // the compatibility unorm target, independently of the surface format.
        // Keeping the atlas unorm preserves the authored orange in logo.png.
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


pub fn create_vertex_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cubacadabra vertices"),
        size: (capacity * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub fn world_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cubacadabra world image texture layout"),
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
    })
}

pub fn create_world_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> wgpu::BindGroup {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cubacadabra game image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cubacadabra game image sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cubacadabra game image bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
}

pub fn shadow_globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cubacadabra shadow globals layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

pub fn shadow_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cubacadabra shadow sampling layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    })
}

pub fn create_shadow_resources(
    device: &wgpu::Device,
    globals_layout: &wgpu::BindGroupLayout,
    layout: &wgpu::BindGroupLayout,
) -> (
    wgpu::Buffer,
    wgpu::TextureView,
    wgpu::BindGroup,
    wgpu::BindGroup,
) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cubacadabra directional shadow map"),
        size: wgpu::Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("cubacadabra directional shadow depth view"),
        aspect: wgpu::TextureAspect::DepthOnly,
        ..Default::default()
    });
    let sampled_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("cubacadabra directional shadow sampled view"),
        aspect: wgpu::TextureAspect::DepthOnly,
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cubacadabra soft shadow sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cubacadabra shadow globals"),
        size: std::mem::size_of::<super::super::ShadowGlobals>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cubacadabra shadow sampling bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&sampled_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cubacadabra shadow globals bind group"),
        layout: globals_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, depth_view, bind_group, globals_bind_group)
}

#[cfg(any(feature = "dev-showcase", test))]
pub fn clear_shadow_depth(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shadow_depth_view: &wgpu::TextureView,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("cubacadabra clear shadow depth"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cubacadabra clear shadow depth pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: shadow_depth_view,
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
    }
    queue.submit(Some(encoder.finish()));
}

pub fn shadow_pipeline(
    device: &wgpu::Device,
    shadow_globals_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cubacadabra world shadow shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shadow.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cubacadabra world shadow pipeline layout"),
        bind_group_layouts: &[Some(shadow_globals_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cubacadabra world shadow pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Vertex::LAYOUT],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.01,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}

pub fn world_mesh_shadow_pipeline(
    device: &wgpu::Device,
    shadow_globals_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cubacadabra world mesh shadow shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../world_mesh_shadow.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cubacadabra world mesh shadow pipeline layout"),
        bind_group_layouts: &[Some(shadow_globals_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cubacadabra world mesh shadow pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[WorldMeshVertex::LAYOUT, WorldMeshInstance::LAYOUT],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.01,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}

pub fn world_pipeline(
    device: &wgpu::Device,
    globals_layout: &wgpu::BindGroupLayout,
    world_texture_layout: &wgpu::BindGroupLayout,
    terrain_texture_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
    samples: u32,
    translucent: bool,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("world shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../renderer.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cubacadabra pipeline layout"),
        bind_group_layouts: &[
            Some(globals_layout),
            Some(world_texture_layout),
            Some(terrain_texture_layout),
            Some(shadow_layout),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            depth_write_enabled: Some(!translucent),
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
                format: super::super::targets::SCENE_FORMAT,
                blend: translucent.then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub fn sky_resources(
    device: &wgpu::Device,
    samples: u32,
) -> (wgpu::RenderPipeline, wgpu::Buffer, wgpu::BindGroup) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cubacadabra sky globals layout"),
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
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cubacadabra sky globals"),
        contents: bytemuck::bytes_of(&SkyGlobals::zeroed()),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cubacadabra sky globals bind group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cubacadabra sky shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../sky.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cubacadabra sky pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cubacadabra sky pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_sky"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        // The sky is drawn in the world pass, which owns the shared scene
        // depth attachment. Declare the format for pipeline compatibility,
        // but never read or write scene depth for the background.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_sky"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: super::super::targets::SCENE_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    (pipeline, buffer, bind_group)
}

pub fn world_mesh_pipeline(
    device: &wgpu::Device,
    globals_layout: &wgpu::BindGroupLayout,
    world_texture_layout: &wgpu::BindGroupLayout,
    terrain_texture_layout: &wgpu::BindGroupLayout,
    shadow_layout: &wgpu::BindGroupLayout,
    samples: u32,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("world mesh shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../world_mesh.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cubacadabra world mesh pipeline layout"),
        bind_group_layouts: &[
            Some(globals_layout),
            Some(world_texture_layout),
            Some(terrain_texture_layout),
            Some(shadow_layout),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cubacadabra world mesh pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[WorldMeshVertex::LAYOUT, WorldMeshInstance::LAYOUT],
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
                format: super::super::targets::SCENE_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod terrain_material_tests {
    use super::*;

    #[test]
    fn built_in_terrain_maps_are_rgba_512_tiles() {
        for pixels in [
            decode_terrain_image(TERRAIN_GRASS_TOP_BYTES),
            decode_terrain_image(TERRAIN_GRASS_SIDE_BYTES),
            decode_terrain_image(TERRAIN_GROUND_BYTES),
            decode_terrain_image(TERRAIN_ROCK_BYTES),
            decode_terrain_image(TERRAIN_SAND_BYTES),
            decode_terrain_image(TERRAIN_MUD_BYTES),
            decode_terrain_image(TERRAIN_SNOW_BYTES),
        ] {
            assert_eq!(
                pixels.len(),
                TERRAIN_TILE_SIZE as usize * TERRAIN_TILE_SIZE as usize * 4
            );
            assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
        }
    }

    #[test]
    fn terrain_mip_downsampling_averages_each_four_pixel_footprint() {
        let source = [0, 0, 0, 255, 4, 4, 4, 255, 8, 8, 8, 255, 12, 12, 12, 255];
        assert_eq!(downsample_terrain_image(2, &source), [6, 6, 6, 255]);
    }
}

pub fn ui_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shared UI shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../renderer.wgsl").into()),
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
                format: super::super::targets::SCENE_FORMAT,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let (ui_texture_view, ui_sampler) = create_ui_texture_atlas(device, queue);
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
    (ui_pipeline, ui_texture_bind_group)
}
