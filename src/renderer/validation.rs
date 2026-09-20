//! Opt-in GPU acceptance fixture using the production layouts, pipelines,
//! catalog, render targets and UI atlas on both native and browser adapters.
use super::{
    Globals, RenderEntity, Vertex,
    character_gpu::{CharacterRenderer, CharacterStats},
    character_material::CharacterPass,
    targets::{PostProcessor, Presenter, SceneTargets},
};
use glam::{Mat4, Vec3};
use serde::Serialize;
use wgpu::util::DeviceExt;

#[path = "validation_scene.rs"]
mod validation_scene;
use validation_scene::{Context, TestScene, populate};

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
    let world_texture_layout = super::device::world_texture_bind_group_layout(&device);
    let terrain_texture_layout = super::device::terrain_texture_bind_group_layout(&device);
    let terrain_texture_bind_group =
        super::device::create_terrain_texture_bind_group(&device, &queue, &terrain_texture_layout);
    let shadow_bind_group_layout = super::device::shadow_bind_group_layout(&device);
    let shadow_globals_layout = super::device::shadow_globals_layout(&device);
    let (_, shadow_depth_view, shadow_bind_group, _) = super::device::create_shadow_resources(
        &device,
        &shadow_globals_layout,
        &shadow_bind_group_layout,
    );
    super::device::clear_shadow_depth(&device, &queue, &shadow_depth_view);
    let placeholder = [255_u8; 4];
    let world_texture_bind_group = super::device::create_world_texture_bind_group(
        &device,
        &queue,
        &world_texture_layout,
        1,
        1,
        &placeholder,
    );
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
        world_texture_layout: &world_texture_layout,
        terrain_texture_layout: &terrain_texture_layout,
        shadow_bind_group_layout: &shadow_bind_group_layout,
        world_texture_bind_group,
        terrain_texture_bind_group,
        shadow_bind_group,
        globals_buffer,
        globals,
        ui,
        atlas,
        timer,
        query_buffer,
    };
    let cold = Clock::start();
    let mut characters =
        CharacterRenderer::new(&device, &globals_layout, &shadow_bind_group_layout, 1);
    let cold_catalog_ms = cold.ms();
    let mut measurements = Vec::new();
    let mut images = Vec::new();
    let mut modes = vec![1];
    if samples > 1 {
        modes.push(samples);
    }
    for sample_count in modes {
        if sample_count != 1 {
            characters = CharacterRenderer::new(
                &device,
                &globals_layout,
                &shadow_bind_group_layout,
                sample_count,
            );
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
    characters = CharacterRenderer::new(&device, &globals_layout, &shadow_bind_group_layout, 1);
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
        if let Some(error) = scope.pop().await {
            return Err(format!("seam probe render failed: {error}"));
        }
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
        || effect_depth_write_max_error > 0
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
