use super::reference_capture_scene::{
    LightingValues, ProjectEffect, ProjectLighting, ReferenceGlobals, ReferenceVertex, SceneBounds,
};
use super::*;

pub(super) fn add_geometry(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
) {
    let center = Vec3::from_array(geometry.transform.position);
    // Format 1 stores the three Roblox CFrame matrix rows. Reconstruct the
    // column vectors expected by glam before transforming local vertices.
    let basis = basis_from_rows(geometry.transform.rotation);
    let mut half = Vec3::from_array(geometry.size) * 0.5;
    let mut center = center;
    if let Some(mesh) = &geometry.mesh
        && mesh.kind == "SpecialMesh"
        && mesh.mesh_type == Some(2)
    {
        half *= Vec3::from_array(mesh.scale.unwrap_or([1.0, 1.0, 1.0]));
        center += basis * Vec3::from_array(mesh.offset.unwrap_or([0.0, 0.0, 0.0]));
    }
    if geometry.class == "WedgePart" {
        add_oriented_wedge(vertices, bounds, geometry, center, basis, half);
    } else {
        add_oriented_box(vertices, bounds, geometry, center, basis, half);
    }
}

pub(super) fn basis_from_rows(rotation: [[f32; 3]; 3]) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(rotation[0][0], rotation[1][0], rotation[2][0]),
        Vec3::new(rotation[0][1], rotation[1][1], rotation[2][1]),
        Vec3::new(rotation[0][2], rotation[1][2], rotation[2][2]),
    )
}

fn add_oriented_box(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
    center: Vec3,
    basis: Mat3,
    half: Vec3,
) {
    let local = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ];
    let world = local.map(|point| center + basis * point);
    for point in world {
        bounds.include(point);
    }
    let color = [
        geometry.color[0],
        geometry.color[1],
        geometry.color[2],
        1.0 - geometry.transparency,
    ];
    for (indices, local_normal) in [
        ([0, 3, 2, 0, 2, 1], Vec3::NEG_Z),
        ([4, 5, 6, 4, 6, 7], Vec3::Z),
        ([0, 4, 7, 0, 7, 3], Vec3::NEG_X),
        ([1, 2, 6, 1, 6, 5], Vec3::X),
        ([0, 1, 5, 0, 5, 4], Vec3::NEG_Y),
        ([3, 7, 6, 3, 6, 2], Vec3::Y),
    ] {
        let normal = (basis * local_normal).normalize_or_zero().to_array();
        vertices.extend(indices.into_iter().map(|index| ReferenceVertex {
            position: world[index].to_array(),
            normal,
            color,
        }));
    }
}

fn add_oriented_wedge(
    vertices: &mut Vec<ReferenceVertex>,
    bounds: &mut SceneBounds,
    geometry: &GeometryInstance,
    center: Vec3,
    basis: Mat3,
    half: Vec3,
) {
    let local = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
    ];
    let world = local.map(|point| center + basis * point);
    for point in world {
        bounds.include(point);
    }
    let color = [
        geometry.color[0],
        geometry.color[1],
        geometry.color[2],
        1.0 - geometry.transparency,
    ];
    for indices in [
        [0, 1, 3],
        [0, 3, 2],
        [0, 4, 5],
        [0, 5, 1],
        [2, 3, 5],
        [2, 5, 4],
        [0, 2, 4],
        [1, 5, 3],
    ] {
        add_triangle(vertices, world, indices, color);
    }
}

fn add_triangle(
    vertices: &mut Vec<ReferenceVertex>,
    points: [Vec3; 6],
    indices: [usize; 3],
    color: [f32; 4],
) {
    let first = points[indices[0]];
    let second = points[indices[1]];
    let third = points[indices[2]];
    let normal = (second - first)
        .cross(third - first)
        .normalize_or_zero()
        .to_array();
    vertices.extend(indices.into_iter().map(|index| ReferenceVertex {
        position: points[index].to_array(),
        normal,
        color,
    }));
}

#[derive(Clone, Copy)]
pub(super) struct CaptureCamera {
    pub(super) position: Vec3,
    pub(super) target: Vec3,
    pub(super) field_of_view_degrees: f32,
    pub(super) near: f32,
    pub(super) far: f32,
}

pub(super) fn fit_camera(bounds: SceneBounds, aspect: f32) -> CaptureCamera {
    let size = bounds.size();
    let center = bounds.center();
    let field_of_view_degrees: f32 = 55.0;
    let vertical_fov = field_of_view_degrees.to_radians();
    let horizontal_fov = 2.0 * ((vertical_fov * 0.5).tan() * aspect).atan();
    let fit_height = size.y / (vertical_fov * 0.5).tan();
    let fit_width = size.x.max(size.z) / (horizontal_fov * 0.5).tan();
    let distance = fit_height.max(fit_width) * 0.58 + size.length() * 0.25;
    let direction = Vec3::new(0.08, 0.28, 1.0).normalize();
    let target = center + Vec3::new(0.0, size.y * 0.06, 0.0);
    let position = target + direction * distance;
    CaptureCamera {
        position,
        target,
        field_of_view_degrees,
        near: (distance * 0.01).max(0.05),
        far: distance + size.length() * 2.5,
    }
}

pub(super) fn lighting_values(project: Option<&ProjectLighting>) -> LightingValues {
    let defaults = LightingValues {
        brightness: 2.0,
        ambient: Vec3::ZERO,
        outdoor_ambient: Vec3::splat(0.5),
        correction_brightness: 0.0,
        correction_contrast: 0.0,
        correction_saturation: 0.0,
    };
    let Some(project) = project else {
        return defaults;
    };
    let correction = project
        .effects
        .iter()
        .find(|effect| effect.class == "ColorCorrectionEffect" && effect_enabled(effect));
    LightingValues {
        brightness: number(&project.properties, "Brightness").unwrap_or(defaults.brightness),
        ambient: color(&project.properties, "Ambient").unwrap_or(defaults.ambient),
        outdoor_ambient: color(&project.properties, "OutdoorAmbient")
            .unwrap_or(defaults.outdoor_ambient),
        correction_brightness: correction
            .and_then(|effect| number(&effect.properties, "Brightness"))
            .unwrap_or(0.0),
        correction_contrast: correction
            .and_then(|effect| number(&effect.properties, "Contrast"))
            .unwrap_or(0.0),
        correction_saturation: correction
            .and_then(|effect| number(&effect.properties, "Saturation"))
            .unwrap_or(0.0),
    }
}

fn number(properties: &BTreeMap<String, Value>, key: &str) -> Option<f32> {
    properties.get(key)?.as_f64().map(|value| value as f32)
}

fn color(properties: &BTreeMap<String, Value>, key: &str) -> Option<Vec3> {
    let values = properties.get(key)?.as_array()?;
    (values.len() == 3).then(|| {
        Vec3::new(
            values[0].as_f64().unwrap_or_default() as f32,
            values[1].as_f64().unwrap_or_default() as f32,
            values[2].as_f64().unwrap_or_default() as f32,
        )
    })
}

fn effect_enabled(effect: &ProjectEffect) -> bool {
    effect
        .properties
        .get("Enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn enabled_effect_count(scene: &ReferenceScene, class: &str) -> usize {
    scene
        .project_lighting
        .iter()
        .flat_map(|lighting| &lighting.effects)
        .filter(|effect| effect.class == class && effect_enabled(effect))
        .count()
}

pub(super) struct RenderResult {
    pub(super) pixels: Vec<u8>,
    pub(super) sample_count: u32,
    pub(super) adapter: wgpu::AdapterInfo,
}

pub(super) fn render(
    vertices: &[ReferenceVertex],
    width: u32,
    height: u32,
    antialias: bool,
    camera: CaptureCamera,
    lighting: LightingValues,
) -> Result<RenderResult, String> {
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
    let sample_count = super::super::targets::select_samples(&adapter, antialias);
    let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Roblox reference capture device"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("headless device unavailable: {error}"))?;

    let projection = Mat4::perspective_rh(
        camera.field_of_view_degrees.to_radians(),
        width as f32 / height as f32,
        camera.near,
        camera.far,
    );
    let view = Mat4::look_at_rh(camera.position, camera.target, Vec3::Y);
    let ambient = lighting.ambient.max(lighting.outdoor_ambient * 0.72);
    let globals = ReferenceGlobals {
        view_projection: (projection * view).to_cols_array_2d(),
        sun_direction_brightness: [-0.52, 0.78, -0.35, lighting.brightness.max(0.0)],
        ambient: [ambient.x, ambient.y, ambient.z, 0.0],
        color_correction: [
            lighting.correction_brightness,
            lighting.correction_contrast,
            lighting.correction_saturation,
            0.0,
        ],
    };
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Roblox reference vertices"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Roblox reference globals"),
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Roblox reference globals layout"),
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
    let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Roblox reference globals bind group"),
        layout: &globals_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buffer.as_entire_binding(),
        }],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Roblox reference capture shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("reference_capture.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Roblox reference capture pipeline layout"),
        bind_group_layouts: &[Some(&globals_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Roblox reference capture pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[ReferenceVertex::LAYOUT],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: super::super::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
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

    let color_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Roblox reference capture color"),
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
    let multisample_view = (sample_count > 1).then(|| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Roblox reference capture multisample color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    });
    let depth_view = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Roblox reference capture depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: super::super::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());

    let padded_row_bytes = align_to(width as u64 * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Roblox reference capture readback"),
        size: padded_row_bytes * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Roblox reference capture encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Roblox reference capture pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: multisample_view.as_ref().unwrap_or(&color_view),
                depth_slice: None,
                resolve_target: multisample_view.as_ref().map(|_| &color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.30,
                        g: 0.88,
                        b: 0.96,
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
        pass.set_pipeline(&pipeline);
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
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("poll reference capture device: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("receive reference capture readback: {error}"))?
        .map_err(|error| format!("map reference capture readback: {error}"))?;
    let mapped = slice.get_mapped_range();
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let source = row * padded_row_bytes as usize;
        let target = row * width as usize * 4;
        pixels[target..target + width as usize * 4]
            .copy_from_slice(&mapped[source..source + width as usize * 4]);
    }
    drop(mapped);
    readback.unmap();
    Ok(RenderResult {
        pixels,
        sample_count,
        adapter: adapter_info,
    })
}

fn align_to(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

pub(super) fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    Ok(())
}

pub(super) fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file =
        fs::File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
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
