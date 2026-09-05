//! Opt-in GPU acceptance fixture using the production layouts, pipelines,
//! catalog, render targets and UI atlas on both native and browser adapters.
use super::{
    Globals, RenderEntity, Vertex,
    character_gpu::{CharacterRenderer, CharacterStats},
    character_material::CharacterPass,
    targets::{Presenter, SceneTargets},
};
use glam::{Mat4, Vec3};
use serde::Serialize;
use wgpu::util::DeviceExt;

const FRAMES: usize = 60;

#[derive(Serialize)]
pub struct Measurement {
    pub name: String,
    pub sample_count: u32,
    pub width: u32,
    pub height: u32,
    pub render_target_bytes: usize,
    stats: CharacterStats,
    pub cpu_median_ms: f64,
    pub cpu_p95_ms: f64,
    pub gpu_pass_median_ms: Option<f64>,
    pub gpu_pass_p95_ms: Option<f64>,
    pub gpu_valid_samples: usize,
    pub submit_and_wait_p95_ms: f64,
}
#[derive(Serialize)]
pub struct Image {
    pub name: String,
    pub png: Vec<u8>,
}
#[derive(Serialize)]
pub struct ValidationReport {
    pub adapter: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub frames_per_measurement: usize,
    pub samples_supported: u32,
    pub max_vertex_attributes: u32,
    pub instance_stride: usize,
    pub cold_catalog_ms: f64,
    pub surface_color_max_error: u8,
    pub legacy_color_max_error: u8,
    pub occlusion_max_error: u8,
    pub effect_depth_write_max_error: u8,
    pub surface_frames: usize,
    pub measurements: Vec<Measurement>,
    pub notes: Vec<&'static str>,
}
#[derive(Serialize)]
pub struct ValidationOutput {
    pub report: ValidationReport,
    pub images: Vec<Image>,
}

struct Clock {
    #[cfg(not(target_arch = "wasm32"))]
    started: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    started: f64,
}
impl Clock {
    fn start() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            started: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            started: web_sys::window().unwrap().performance().unwrap().now(),
        }
    }
    fn ms(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.started.elapsed().as_secs_f64() * 1000.0
        }
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window().unwrap().performance().unwrap().now() - self.started
        }
    }
}

pub async fn validate(adapter: &wgpu::Adapter) -> Result<ValidationOutput, String> {
    let samples = super::targets::select_samples(&adapter, true);
    // Timestamp support is diagnostic only; production still requests no
    // optional features. A missing timer does not disable any rendering path.
    let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("phase 3 validation"),
            required_features: features,
            required_limits: super::device::required_limits(&adapter),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|e| e.to_string())?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("validation globals"),
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
    let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("validation globals"),
        size: size_of::<Globals>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let globals = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &globals_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buffer.as_entire_binding(),
        }],
    });
    let (ui, atlas) = super::device::ui_resources(&device, &queue);
    let timer = features.contains(wgpu::Features::TIMESTAMP_QUERY).then(|| {
        device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("world pass timing"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        })
    });
    let query_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let context = Context {
        device: &device,
        queue: &queue,
        globals_buffer,
        globals,
        ui,
        atlas,
        timer,
        query_buffer,
    };
    let cold = Clock::start();
    let mut characters = CharacterRenderer::new(&device, &globals_layout, 1);
    let cold_catalog_ms = cold.ms();
    let mut measurements = Vec::new();
    let mut images = Vec::new();
    let mut modes = vec![1];
    if samples > 1 {
        modes.push(samples);
    }
    for sample_count in modes {
        if sample_count != 1 {
            characters = CharacterRenderer::new(&device, &globals_layout, sample_count);
        }
        let mesh_uploads = characters.stats.mesh_uploads;
        let resident_bytes = characters.stats.resident_bytes;
        let staging_bytes = characters.stats.staging_capacity_bytes;
        for (name, count, width, height) in [
            ("world-only", 0, 1280, 800),
            ("crowd-18", 18, 1280, 800),
            ("crowd-50", 50, 1280, 800),
            ("portrait", 18, 390, 844),
            ("tablet", 18, 768, 1024),
            ("wide", 18, 1440, 900),
            ("lineup", 3, 640, 360),
        ] {
            let scene = TestScene::new(
                &context,
                &globals_layout,
                sample_count,
                width,
                height,
                count,
                false,
                wgpu::TextureFormat::Rgba8Unorm,
            );
            let mut cpu = Vec::new();
            let mut gpu = Vec::new();
            let mut submit = Vec::new();
            let mut pixels = Vec::new();
            for frame in 0..FRAMES + 5 {
                let clock = Clock::start();
                populate(&mut characters, count, frame as f32 * 0.03);
                characters.upload(&queue);
                let cpu_ms = clock.ms();
                let clock = Clock::start();
                let capture = frame == FRAMES + 4;
                let (image, gpu_ms) = scene.render(&context, &characters, capture, true).await?;
                if frame >= 5 {
                    cpu.push(cpu_ms);
                    submit.push(clock.ms());
                    if let Some(ms) = gpu_ms {
                        gpu.push(ms);
                    }
                }
                if capture {
                    pixels = image;
                }
            }
            if characters.stats.mesh_uploads != mesh_uploads
                || characters.stats.resident_bytes != resident_bytes
                || characters.stats.staging_capacity_bytes != staging_bytes
            {
                return Err("character resources grew during warm frames or resize".into());
            }
            if count == 18
                && (characters.stats.draws > 100 || characters.stats.upload_bytes > 512 * 1024)
            {
                return Err("18-character draw/upload budget exceeded".into());
            }
            let name = format!("{name}-{sample_count}x");
            images.push(png_image(&name, width, height, &pixels)?);
            measurements.push(Measurement {
                name,
                sample_count,
                width,
                height,
                render_target_bytes: width as usize
                    * height as usize
                    * if sample_count > 1 {
                        4 + 8 * sample_count as usize
                    } else {
                        8
                    },
                stats: characters.stats,
                cpu_median_ms: percentile(&mut cpu, 0.5),
                cpu_p95_ms: percentile(&mut cpu, 0.95),
                gpu_pass_median_ms: (gpu.len() >= FRAMES * 9 / 10)
                    .then(|| percentile(&mut gpu, 0.5)),
                gpu_pass_p95_ms: (gpu.len() >= FRAMES * 9 / 10).then(|| percentile(&mut gpu, 0.95)),
                gpu_valid_samples: gpu.len(),
                submit_and_wait_p95_ms: percentile(&mut submit, 0.95),
            });
        }
    }
    // Recreate the renderer and targets at 1x. These comparisons exercise the
    // exact same scene through unorm, sRGB and legacy direct presentation.
    characters = CharacterRenderer::new(&device, &globals_layout, 1);
    populate(&mut characters, 3, 0.0);
    characters.upload(&queue);
    let mut unorm = TestScene::new(
        &context,
        &globals_layout,
        1,
        640,
        360,
        3,
        false,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let (a, _) = unorm.render(&context, &characters, true, true).await?;
    let srgb = TestScene::new(
        &context,
        &globals_layout,
        1,
        640,
        360,
        3,
        false,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    );
    let (b, _) = srgb.render(&context, &characters, true, true).await?;
    let surface_color_max_error = max_error(&a, &b);
    images.push(png_image("color-unorm", 640, 360, &a)?);
    images.push(png_image("color-srgb", 640, 360, &b)?);
    let (direct, _) = unorm.render(&context, &characters, true, false).await?;
    let legacy_color_max_error = max_error(&a, &direct);
    populate(&mut characters, 3, 0.0);
    characters.add_effect_probe();
    characters.upload(&queue);
    let (visible_effect, _) = unorm.render(&context, &characters, true, true).await?;
    if max_error(&a, &visible_effect) < 20 {
        return Err("seam effect probe is not visible".into());
    }
    unorm.effects_first = true;
    // Existing seam cores may legitimately change when the entire effect
    // pass moves before solids. Compare the probe against that same ordering
    // so the regression isolates whether this probe writes depth.
    populate(&mut characters, 3, 0.0);
    characters.upload(&queue);
    let (effects_first_without_probe, _) = unorm.render(&context, &characters, true, true).await?;
    populate(&mut characters, 3, 0.0);
    characters.add_effect_probe();
    characters.upload(&queue);
    let (overwritten_effect, _) = unorm.render(&context, &characters, true, true).await?;
    let effect_depth_write_max_error = max_error(&effects_first_without_probe, &overwritten_effect);
    images.push(png_image("visible-emission", 640, 360, &visible_effect)?);
    // Full opaque receiver in front of all bodies, faces and seam emission.
    let wall = TestScene::new(
        &context,
        &globals_layout,
        1,
        640,
        360,
        3,
        true,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let (hidden, _) = wall.render(&context, &characters, true, true).await?;
    populate(&mut characters, 0, 0.0);
    characters.upload(&queue);
    let (empty, _) = wall.render(&context, &characters, true, true).await?;
    let occlusion_max_error = max_error(&hidden, &empty);
    images.push(png_image("opaque-occlusion", 640, 360, &hidden)?);
    if surface_color_max_error > 1
        || legacy_color_max_error > 0
        || occlusion_max_error > 0
    {
        return Err(format!(
            "pixel regression: surface={surface_color_max_error}, direct={legacy_color_max_error}, occlusion={occlusion_max_error}, effect-depth={effect_depth_write_max_error}"
        ));
    }
    if let Some(error) = scope.pop().await {
        return Err(error.to_string());
    }
    Ok(ValidationOutput {
        report: ValidationReport {
            adapter: info.name,
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver: info.driver,
            frames_per_measurement: FRAMES,
            samples_supported: samples,
            max_vertex_attributes: device.limits().max_vertex_attributes,
            instance_stride: size_of::<super::character_material::CharacterInstance>(),
            cold_catalog_ms,
            surface_color_max_error,
            legacy_color_max_error,
            occlusion_max_error,
            effect_depth_write_max_error,
            surface_frames: 0,
            measurements,
            notes: vec![
                "CPU timings include pose, batching and instance upload; five warmup frames precede 60 measured frames.",
                "GPU timestamps, when available, measure the whole 3D pass. Compare world-only with the crowd at the same resolution/sample count; submit-and-wait is not GPU time.",
                "Zero or reversed GPU timestamp pairs are excluded; summaries require 90% valid samples and gpu_valid_samples reports coverage. Without queries, a one-pixel copy ties readback completion to the submitted frame. Browser polling includes event-loop scheduling delays.",
                "Color regression includes the production UI logo/font atlas, alpha blending, world colors and character materials; UI is single sampled after resolve.",
                "The fixed bundled catalog is recreated and reused across all viewport sizes; no simulation or network capacity changes.",
                "These are local device measurements against provisional RFC budgets, not ratification by external device owners.",
            ],
        },
        images,
    })
}

struct Context<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    globals_buffer: wgpu::Buffer,
    globals: wgpu::BindGroup,
    ui: wgpu::RenderPipeline,
    atlas: wgpu::BindGroup,
    timer: Option<wgpu::QuerySet>,
    query_buffer: wgpu::Buffer,
}
struct TestScene {
    effects_first: bool,
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
    presenter: Presenter,
    output: wgpu::Texture,
    readback: wgpu::Buffer,
    row_bytes: u32,
}
impl TestScene {
    fn new(
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
        let targets = SceneTargets::new(device, width, height, samples, &presenter.layout);
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
            fog_color: super::color(0x9ab9be),
        };
        let mut world = Vec::new();
        super::add_cuboid(
            &mut world,
            Vec3::new(0.0, -0.08, 0.0),
            Vec3::new(120.0, 0.16, 120.0),
            super::color(0xa7bd99),
        );
        if wall {
            super::add_cuboid(
                &mut world,
                Vec3::new(0.0, 2.0, -2.0),
                Vec3::new(12.0, 5.0, 0.2),
                super::color(0xd0a86f),
            );
        } else {
            super::add_cuboid(
                &mut world,
                Vec3::new(3.5, 0.7, 0.6),
                Vec3::new(1.0, 1.4, 1.2),
                [0.2, 0.6, 0.7, 0.4],
            );
        }
        let (mut opaque, mut alpha) = (Vec::new(), Vec::new());
        super::draw::split_world_vertices(&world, &mut opaque, &mut alpha);
        super::draw::sort_translucent(&mut alpha, camera, target);
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
            world: super::device::world_pipeline(device, layout, samples, false),
            translucent: super::device::world_pipeline(device, layout, samples, true),
            world_buffer,
            opaque_count,
            world_count,
            ui_buffer,
            ui_count: ui_vertices.len() as u32,
            targets,
            presenter,
            output,
            readback,
            row_bytes,
        }
    }

    async fn render(
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
            let mut attachment = self.targets.attachment(clear);
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
            pass.set_pipeline(&self.world);
            pass.set_vertex_buffer(0, self.world_buffer.slice(..));
            pass.draw(0..self.opaque_count, 0..1);
            if self.effects_first {
                characters.draw(&mut pass, CharacterPass::Effect);
            }
            characters.draw(&mut pass, CharacterPass::Opaque);
            characters.draw(&mut pass, CharacterPass::Face);
            if !self.effects_first {
                characters.draw(&mut pass, CharacterPass::Effect);
            }
            pass.set_pipeline(&self.translucent);
            pass.set_vertex_buffer(0, self.world_buffer.slice(..));
            pass.draw(self.opaque_count..self.world_count, 0..1);
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

fn populate(characters: &mut CharacterRenderer, count: usize, phase: f32) {
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
        let mut style = super::default_player_style();
        style.skin = super::color([0xe8ae86, 0xc98464, 0x82b78f][index % 3]);
        style.shirt = super::color([0x2d6663, 0x5f8f78, 0x694c88][index % 3]);
        characters.add(
            RenderEntity {
                position,
                body: crate::character::BodyId::ALL[index % 3],
                walk_cycle: phase + index as f32 * 0.37,
                moving: count > 3,
                sprinting: index % 2 == 0,
                ..Default::default()
            },
            style,
            super::color(0x173f43),
        );
    }
}

fn ui_fixture(width: u32, height: u32) -> Vec<Vertex> {
    use crate::ui::*;
    let mut nodes = Vec::new();
    for (index, color) in [0xe8ae86, 0x45302b, 0x2d6663, 0x536a90, 0xf68b1f]
        .into_iter()
        .enumerate()
    {
        nodes.push(UiRenderNode {
            id: format!("swatch-{index}"),
            kind: UiNodeKind::Button,
            rect: UiRect {
                x: 12.0 + index as f32 * 55.0,
                y: 12.0,
                width: 50.0,
                height: 44.0,
            },
            text: if index == 4 {
                "Aa".into()
            } else {
                String::new()
            },
            icon: None,
            background: Some(super::color(color)),
            foreground: [1.0; 4],
            border_color: None,
            border_width: 0.0,
            corner_radius: 8.0,
            font_size: 18.0,
            text_align: UiAlignment::Center,
            accent: [1.0; 4],
            image: (index == 0).then_some(UiImage::Logo),
            image_invert: false,
            value: 0.0,
            value_x: 0.0,
            value_y: 0.0,
            checked: false,
            pressed: false,
            disabled: false,
        });
    }
    super::ui::build_ui_vertices(&UiFrame {
        viewport: UiViewport {
            width: width as f32,
            height: height as f32,
            scale: 1.0,
            safe_area: Default::default(),
        },
        nodes,
    })
}
fn percentile(values: &mut [f64], p: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * p).ceil() as usize]
}
fn max_error(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0)
}
fn png_image(name: &str, width: u32, height: u32, pixels: &[u8]) -> Result<Image, String> {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(pixels)
            .map_err(|e| e.to_string())?;
    }
    Ok(Image {
        name: name.into(),
        png,
    })
}

async fn map(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback_result = result.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |value| {
                *callback_result.lock().unwrap() = Some(value);
            });
        loop {
            // wgpu-core WebGL maps become available after GPU fences signal.
            // Yield to the browser between maintenance polls; never block its
            // event loop waiting for the callback that it must itself deliver.
            device
                .poll(wgpu::PollType::Poll)
                .map_err(|e| e.to_string())?;
            if let Some(value) = result.lock().unwrap().take() {
                return value.map_err(|e| e.to_string());
            }
            let delay = js_sys::Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
                    .unwrap();
            });
            wasm_bindgen_futures::JsFuture::from(delay)
                .await
                .map_err(|e| format!("{e:?}"))?;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn capture_phase3(output: impl AsRef<std::path::Path>) -> Result<ValidationReport, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|e| e.to_string())?;
    let result = pollster::block_on(validate(&adapter))?;
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    for image in result.images {
        std::fs::write(
            output.as_ref().join(format!("{}.png", image.name)),
            image.png,
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        output.as_ref().join("phase3_report.json"),
        serde_json::to_vec_pretty(&result.report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(result.report)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn validate_character_gpu(
    canvas: web_sys::HtmlCanvasElement,
    backend: String,
) -> Result<String, wasm_bindgen::JsValue> {
    let backend = match backend.as_str() {
        "webgpu" => wgpu::Backends::BROWSER_WEBGPU,
        "gl" => wgpu::Backends::GL,
        _ => return Err("expected webgpu or gl".into()),
    };
    let instance = wgpu::Instance::new(super::device::browser_instance_descriptor(backend));
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))?;
    let mut output = validate(&adapter)
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e))?;
    // Also exercise actual host surface configuration/presentation, resize,
    // repeated sync/draw, first-person hiding and resource teardown with the
    // production empty-feature device contract on the selected browser backend.
    // WebGPU adapters are single-use for device creation.
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("production surface validation"),
            required_features: wgpu::Features::empty(),
            required_limits: super::device::required_limits(&adapter),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer =
        super::Renderer::from_parts(surface, adapter, device, queue, 640.0, 360.0, true);
    let mut engine = crate::Engine::new();
    for (width, height) in [(390, 844), (768, 1024), (1280, 800), (1440, 900)] {
        canvas.set_width(width);
        canvas.set_height(height);
        engine.set_ui_viewport(crate::ui::UiViewport {
            width: width as f32,
            height: height as f32,
            scale: 1.0,
            safe_area: Default::default(),
        });
        renderer.resize(width as f32, height as f32);
        renderer.sync_engine(&engine);
        renderer.draw();
        renderer.scene.camera[2] = 0.5;
        renderer.draw();
        if renderer.characters.stats.characters != 0 {
            return Err("first-person body was submitted".into());
        }
        output.report.surface_frames += 2;
    }
    if let Some(error) = scope.pop().await {
        return Err(error.to_string().into());
    }
    serde_json::to_string(&output).map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))
}
