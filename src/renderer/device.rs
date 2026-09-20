#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "android", target_os = "ios"))
))]
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
#[cfg(any(target_os = "android", target_os = "ios"))]
use std::ffi::c_void;
use std::io::Cursor;
#[cfg(target_os = "android")]
use std::ptr::NonNull;
#[cfg(target_os = "android")]
use std::sync::Arc;

use bytemuck::Zeroable;
use cubacadabra_morphs::{MorphDiagnostic, decode_morph_pack};
use wgpu::util::DeviceExt;

use super::{
    DEPTH_FORMAT, Globals, Renderer, SkyGlobals, Vertex,
    ui::{
        UI_ATLAS_HEIGHT, UI_ATLAS_PADDING, UI_ATLAS_WIDTH, UI_FONT_ATLAS_Y,
        WORLD_LABEL_FONT_ATLAS_Y, ui_atlas_glyphs, world_label_atlas_glyphs,
    },
    world_mesh::{WorldMeshInstance, WorldMeshVertex},
};

mod resources {
    include!("device/resources.rs");
}
pub(super) use resources::{
    clear_shadow_depth, create_shadow_resources, create_terrain_texture_bind_group,
    create_vertex_buffer, create_world_texture_bind_group, shadow_bind_group_layout,
    shadow_globals_layout, shadow_pipeline, sky_resources, terrain_texture_bind_group_layout,
    ui_resources, world_mesh_pipeline, world_mesh_shadow_pipeline, world_pipeline,
    world_texture_bind_group_layout,
};

const UI_LOGO_BYTES: &[u8] = include_bytes!("../../assets/images/logo.png");
const UI_CUBE_BYTES: &[u8] = include_bytes!("../../assets/images/cube.png");
const UI_CHAT_BYTES: &[u8] = include_bytes!("../../assets/images/chat.png");
const UI_VOICE_BYTES: &[u8] = include_bytes!("../../assets/images/voice.png");
const TERRAIN_GRASS_TOP_BYTES: &[u8] =
    include_bytes!("../../assets/materials/terrain/grass-top-soft.png");
const TERRAIN_GRASS_SIDE_BYTES: &[u8] =
    include_bytes!("../../assets/materials/terrain/grass-side-soft.png");
const TERRAIN_GROUND_BYTES: &[u8] =
    include_bytes!("../../assets/materials/terrain/ground-soft.png");
const TERRAIN_ROCK_BYTES: &[u8] = include_bytes!("../../assets/materials/terrain/rock.png");
const TERRAIN_SAND_BYTES: &[u8] = include_bytes!("../../assets/materials/terrain/sand.png");
const TERRAIN_MUD_BYTES: &[u8] = include_bytes!("../../assets/materials/terrain/mud.png");
const TERRAIN_SNOW_BYTES: &[u8] = include_bytes!("../../assets/materials/terrain/snow.png");
const TERRAIN_TILE_SIZE: u32 = 512;
const TERRAIN_LAYER_COUNT: u32 = 7;
pub(super) const SHADOW_MAP_SIZE: u32 = 1024;

#[cfg(target_os = "android")]
pub(super) static ANDROID_SURFACE_WARNING_REPORTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "android")]
pub(super) static ANDROID_FIRST_FRAME_REPORTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "android")]
pub(super) fn android_log(message: impl AsRef<str>) {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;
    }
    let Ok(message) = CString::new(message.as_ref()) else {
        return;
    };
    const TAG: &[u8] = b"RustRenderer\0";
    const FORMAT: &[u8] = b"%s\0";
    unsafe {
        __android_log_print(
            4,
            TAG.as_ptr().cast(),
            FORMAT.as_ptr().cast(),
            message.as_ptr(),
        );
    }
}

#[cfg(target_os = "ios")]
pub(super) fn android_log(message: impl AsRef<str>) {
    eprintln!("[RustRenderer] {}", message.as_ref());
}

pub(super) fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    let baseline = if adapter.get_info().backend == wgpu::Backend::Gl {
        // GL has no compute/storage requirement. Its 16 attributes and two
        // vertex buffers are sufficient for the character instance layout.
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::downlevel_defaults()
    };
    baseline.using_resolution(adapter.limits())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn browser_instance_descriptor(backends: wgpu::Backends) -> wgpu::InstanceDescriptor {
    #[derive(Debug)]
    struct BrowserDisplay;
    impl wgpu::rwh::HasDisplayHandle for BrowserDisplay {
        fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
            Ok(wgpu::rwh::DisplayHandle::web())
        }
    }
    wgpu::InstanceDescriptor {
        backends,
        // WebGL surface creation needs an explicit display with wgpu 29.
        ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(BrowserDisplay))
    }
}

impl Renderer {
    /// Selects the reversible character visual rollout mode. The mode is
    /// latched by the renderer only; no engine state or snapshot is touched.
    pub(crate) fn set_character_render_mode(&mut self, mode: super::CharacterRenderMode) {
        self.character_render_mode = mode;
    }

    pub(crate) fn character_render_mode(&self) -> super::CharacterRenderMode {
        self.character_render_mode
    }

    /// Uses the normal character/equipment renderer in a neutral, isolated
    /// scene. This is shared by Studio and the platform character editors.
    pub(crate) fn set_avatar_preview_mode(&mut self, enabled: bool) {
        if self.avatar_preview_mode != enabled {
            self.avatar_preview_mode = enabled;
            self.active_world = usize::MAX;
        }
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
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
                // Keep the GLES/EGL validation channel enabled on Android so
                // driver shader and framebuffer failures reach logcat.
                flags: wgpu::InstanceFlags::DEBUG | wgpu::InstanceFlags::VALIDATION,
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
                    android_log(format!("Android GLES surface creation failed: {error}"));
                    return None;
                }
            };
            (instance, surface)
        };

        #[cfg(target_os = "ios")]
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        #[cfg(target_os = "ios")]
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
                .ok()?
        };
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) {
                Ok(adapter) => adapter,
                Err(error) => {
                    android_log(format!("hardware GLES adapter unavailable: {error}"));
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: true,
                    }))
                    .map_err(|fallback_error| {
                        android_log(format!(
                            "fallback GLES adapter unavailable: {fallback_error}"
                        ));
                        fallback_error
                    })
                    .ok()?
                }
            };
        #[cfg(target_os = "android")]
        {
            let info = adapter.get_info();
            let limits = adapter.limits();
            let surface_caps = surface.get_capabilities(&adapter);
            android_log(format!(
                "Android adapter name={:?} backend={:?} driver={:?} \
                 surface_formats={:?} alpha_modes={:?} max_vertex_attributes={} \
                 max_vertex_buffer_array_stride={} max_texture_dimension_2d={}",
                info.name,
                info.backend,
                info.driver,
                surface_caps.formats,
                surface_caps.alpha_modes,
                limits.max_vertex_attributes,
                limits.max_vertex_buffer_array_stride,
                limits.max_texture_dimension_2d,
            ));
        }
        let limits = required_limits(&adapter);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cubacadabra game device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| {
            android_log(format!("GLES device creation failed: {error}"));
            error
        })
        .ok()?;
        #[cfg(target_os = "android")]
        device.on_uncaptured_error(Arc::new(|error| {
            android_log(format!("Android wgpu uncaptured error: {error}"));
        }));
        let renderer = Self::from_parts(surface, adapter, device, queue, width, height, false);
        #[cfg(target_os = "android")]
        android_log("Android renderer resources initialized");
        Some(renderer)
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(any(target_os = "android", target_os = "ios"))
    ))]
    pub fn new_from_window_handles(
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
        width: f32,
        height: f32,
    ) -> Option<Self> {
        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(display_handle),
                    raw_window_handle: window_handle,
                })
                .ok()?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .or_else(|_| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: true,
            }))
        })
        .ok()?;
        let limits = required_limits(&adapter);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cubacadabra desktop game device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
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
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(wasm_bindgen::JsValue::from_str(
                "The renderer size must be positive.",
            ));
        }

        let instance = wgpu::util::new_instance_with_webgpu_detection(browser_instance_descriptor(
            wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
        ))
        .await;
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
        let limits = required_limits(&adapter);
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

    pub(super) fn from_parts(
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
        #[cfg(target_os = "android")]
        {
            let scene_features = adapter.get_texture_format_features(super::targets::SCENE_FORMAT);
            let depth_features = adapter.get_texture_format_features(DEPTH_FORMAT);
            android_log(format!(
                "Android surface format={format:?} scene_format={:?} \
                 scene_flags={:?} depth_flags={:?} size={}x{}",
                super::targets::SCENE_FORMAT,
                scene_features.flags,
                depth_features.flags,
                width.max(1.0) as u32,
                height.max(1.0) as u32,
            ));
        }
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
        // Mali GLES devices can advertise multisample/resolve support while
        // producing a black resolve target for this offscreen scene path.
        // Keep Android on the single-sample compatibility path; Metal and
        // browser backends retain the validated MSAA path.
        let sample_count = super::targets::select_samples(&adapter, !cfg!(target_os = "android"));
        let world_texture_layout = world_texture_bind_group_layout(&device);
        let terrain_texture_layout = terrain_texture_bind_group_layout(&device);
        let terrain_texture_bind_group =
            create_terrain_texture_bind_group(&device, &queue, &terrain_texture_layout);
        let placeholder_pixel = [255_u8, 255, 255, 255];
        let world_texture_bind_group = create_world_texture_bind_group(
            &device,
            &queue,
            &world_texture_layout,
            1,
            1,
            &placeholder_pixel,
        );
        let shadow_globals_layout = shadow_globals_layout(&device);
        let shadow_bind_group_layout = shadow_bind_group_layout(&device);
        let pipeline = world_pipeline(
            &device,
            &globals_layout,
            &world_texture_layout,
            &terrain_texture_layout,
            &shadow_bind_group_layout,
            sample_count,
            false,
        );
        let (sky_pipeline, sky_globals_buffer, sky_globals_bind_group) =
            sky_resources(&device, sample_count);
        let translucent_pipeline = world_pipeline(
            &device,
            &globals_layout,
            &world_texture_layout,
            &terrain_texture_layout,
            &shadow_bind_group_layout,
            sample_count,
            true,
        );
        let world_mesh_pipeline = world_mesh_pipeline(
            &device,
            &globals_layout,
            &world_texture_layout,
            &terrain_texture_layout,
            &shadow_bind_group_layout,
            sample_count,
        );
        let (
            shadow_globals_buffer,
            shadow_depth_view,
            shadow_bind_group,
            shadow_globals_bind_group,
        ) = create_shadow_resources(&device, &shadow_globals_layout, &shadow_bind_group_layout);
        let shadow_pipeline = shadow_pipeline(&device, &shadow_globals_layout);
        let world_mesh_shadow_pipeline =
            world_mesh_shadow_pipeline(&device, &shadow_globals_layout);
        let characters = super::character_gpu::CharacterRenderer::new(
            &device,
            &globals_layout,
            &shadow_bind_group_layout,
            sample_count,
        );
        let character_shadow_pipeline =
            super::character_material::shadow_pipeline(&device, &shadow_globals_layout);
        let presenter = super::targets::Presenter::new(&device, format);
        let post_processor = super::targets::PostProcessor::new(&device);
        let targets = super::targets::SceneTargets::new(
            &device,
            config.width,
            config.height,
            sample_count,
            &post_processor,
            &presenter.layout,
        );
        let (ui_pipeline, ui_texture_bind_group) = ui_resources(&device, &queue);
        let static_vertex_capacity = 16_384;
        let dynamic_vertex_capacity = 16_384;
        let ui_vertex_capacity = 8_192;
        let static_vertex_buffer = create_vertex_buffer(&device, static_vertex_capacity);
        let dynamic_vertex_buffer = create_vertex_buffer(&device, dynamic_vertex_capacity);
        let ui_vertex_buffer = create_vertex_buffer(&device, ui_vertex_capacity);

        Self {
            surface,
            device,
            queue,
            pipeline,
            sky_pipeline,
            sky_globals_buffer,
            sky_globals_bind_group,
            post_processor,
            translucent_pipeline,
            world_mesh_pipeline,
            shadow_pipeline,
            world_mesh_shadow_pipeline,
            shadow_bind_group,
            shadow_globals_bind_group,
            shadow_globals_buffer,
            shadow_depth_view,
            globals_buffer,
            globals_bind_group,
            world_texture_layout,
            world_texture_bind_group,
            terrain_texture_bind_group,
            static_vertex_buffer,
            static_vertex_capacity,
            static_vertex_count: 0,
            terrain_meshes: Vec::new(),
            world_meshes: super::world_mesh::WorldMeshRegistry::default(),
            dynamic_vertex_buffer,
            dynamic_vertex_capacity,
            ui_pipeline,
            ui_texture_bind_group,
            ui_vertex_buffer,
            ui_vertex_capacity,
            config,
            targets,
            presenter,
            sample_count,
            characters,
            character_shadow_pipeline,
            translucent_vertices: Vec::with_capacity(16_384),
            opaque_vertices: Vec::with_capacity(16_384),
            static_translucent_vertices: Vec::new(),
            width,
            height,
            scene: super::Scene::default(),
            // Keep the current production visual as the default. Hosts can
            // select Legacy before their first sync for staged rollout or
            // instant comparison; changing this setting is presentation-only.
            character_render_mode: super::CharacterRenderMode::Magic,
            avatar_preview_mode: false,
            package_image_regions: std::collections::BTreeMap::new(),
            package_generation: 0,
            active_world: usize::MAX,
            worlds: Vec::new(),
            ui_frame: Default::default(),
            #[cfg(feature = "studio-ui")]
            studio_viewport: None,
            #[cfg(feature = "studio-ui")]
            studio_camera_preset: crate::StudioCameraPreset::Gameplay,
            #[cfg(feature = "studio-ui")]
            studio_camera: Default::default(),
        }
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn set_studio_viewport(&mut self, viewport: Option<[f32; 4]>) {
        self.studio_viewport = viewport.filter(|[x, y, width, height]| {
            x.is_finite()
                && y.is_finite()
                && width.is_finite()
                && height.is_finite()
                && *x >= 0.0
                && *y >= 0.0
                && *width > 0.0
                && *height > 0.0
        });
    }

    /// Selects a temporary Studio review camera. This never changes Engine
    /// camera state or snapshots.
    #[cfg(feature = "studio-ui")]
    pub(crate) fn set_studio_camera_preset(&mut self, preset: crate::StudioCameraPreset) {
        if self.studio_camera_preset != preset {
            self.reset_studio_camera();
        }
        self.studio_camera_preset = preset;
    }

    #[cfg(feature = "studio-ui")]
    pub(crate) fn reset_studio_camera(&mut self) {
        self.studio_camera = Default::default();
    }

    pub(crate) fn set_package_image(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> bool {
        const MAX_IMAGE_DIMENSION: u32 = 2048;
        const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
        let Some(byte_count) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|size| size.checked_mul(4))
        else {
            return false;
        };
        if id.is_empty()
            || id.len() > 64
            || width == 0
            || height == 0
            || width > MAX_IMAGE_DIMENSION
            || height > MAX_IMAGE_DIMENSION
            || byte_count != pixels.len()
            || byte_count > MAX_IMAGE_BYTES
        {
            return false;
        }
        self.world_texture_bind_group = create_world_texture_bind_group(
            &self.device,
            &self.queue,
            &self.world_texture_layout,
            width,
            height,
            pixels,
        );
        self.package_image_regions
            .insert(id.to_owned(), [0.0, 0.0, 1.0, 1.0]);
        self.world_meshes.rebuild_instances(
            &self.device,
            &self.scene.world.mesh_instances,
            &self.package_image_regions,
        );
        true
    }

    pub(crate) fn register_morph_pack(&mut self, bytes: &[u8]) -> Result<(), Vec<MorphDiagnostic>> {
        let pack = decode_morph_pack(bytes)?;
        self.characters
            .register_morph_pack(&self.device, &self.queue, pack)
    }

    pub(crate) fn register_world_mesh(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        self.world_meshes.register(&self.device, id, bytes)?;
        self.world_meshes.rebuild_instances(
            &self.device,
            &self.scene.world.mesh_instances,
            &self.package_image_regions,
        );
        Ok(())
    }

    pub(crate) fn clear_world_meshes(&mut self) {
        self.world_meshes.clear();
        self.rebuild_static_vertices();
    }

    pub(crate) fn replace_world_meshes(&mut self, models: &[(&str, &[u8])]) -> Result<(), String> {
        self.world_meshes.replace(
            &self.device,
            models,
            &self.scene.world.mesh_instances,
            &self.package_image_regions,
        )?;
        Ok(())
    }

    pub(crate) fn set_package_image_atlas(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        regions: std::collections::BTreeMap<String, [f32; 4]>,
    ) -> bool {
        const MAX_IMAGE_DIMENSION: u32 = 2048;
        const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
        let Some(byte_count) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|size| size.checked_mul(4))
        else {
            return false;
        };
        if width == 0
            || height == 0
            || width > MAX_IMAGE_DIMENSION
            || height > MAX_IMAGE_DIMENSION
            || byte_count != pixels.len()
            || byte_count > MAX_IMAGE_BYTES
            || regions.len() > 32
            || regions.iter().any(|(id, bounds)| {
                id.is_empty()
                    || id.len() > 64
                    || bounds
                        .iter()
                        .any(|value| !value.is_finite() || *value < 0.0)
                    || bounds[0] + bounds[2] > 1.0
                    || bounds[1] + bounds[3] > 1.0
                    || bounds[2] <= 0.0
                    || bounds[3] <= 0.0
            })
        {
            return false;
        }
        self.world_texture_bind_group = create_world_texture_bind_group(
            &self.device,
            &self.queue,
            &self.world_texture_layout,
            width,
            height,
            pixels,
        );
        self.package_image_regions = regions;
        self.world_meshes.rebuild_instances(
            &self.device,
            &self.scene.world.mesh_instances,
            &self.package_image_regions,
        );
        true
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width.max(1.0) as u32;
        self.config.height = height.max(1.0) as u32;
        self.surface.configure(&self.device, &self.config);
        self.targets = super::targets::SceneTargets::new(
            &self.device,
            self.config.width,
            self.config.height,
            self.sample_count,
            &self.post_processor,
            &self.presenter.layout,
        );
    }
}
