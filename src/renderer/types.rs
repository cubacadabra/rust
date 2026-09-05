pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[derive(Clone)]
pub(super) struct RenderBlock {
    pub(super) position: [f32; 3],
    pub(super) size: [f32; 3],
    pub(super) color: [f32; 4],
    pub(super) outline: bool,
}

#[derive(Clone)]
pub(super) struct RenderPad {
    pub(super) x: f32,
    pub(super) z: f32,
    pub(super) radius: f32,
    pub(super) code: String,
    pub(super) label: String,
    pub(super) color: [f32; 4],
    pub(super) enabled: bool,
    pub(super) availability_label: String,
}

#[derive(Clone)]
pub(super) struct RenderSign {
    pub(super) text: String,
    pub(super) position: [f32; 3],
    pub(super) yaw: f32,
    pub(super) max_width: f32,
    pub(super) color: [f32; 4],
}

#[derive(Clone, Copy, Default)]
pub(super) struct RenderEntity {
    pub(super) position: [f32; 3],
    pub(super) yaw: f32,
    pub(super) walk_cycle: f32,
    pub(super) moving: bool,
    pub(super) sprinting: bool,
    /// Only used by the committed Phase 0 before-image. Production motion
    /// never reads the legacy snapshot suffix as an assembled flag.
    #[allow(dead_code)]
    pub(super) legacy_assembled: bool,
    pub(super) body: crate::character::BodyId,
}

#[derive(Clone, Copy)]
pub(super) struct AvatarStyle {
    pub(super) skin: [f32; 4],
    pub(super) shirt: [f32; 4],
    pub(super) pants: [f32; 4],
    pub(super) shoes: [f32; 4],
}

#[derive(Clone, Copy)]
pub(super) struct RenderPalette {
    pub(super) sky: [f32; 4],
    pub(super) ground: [f32; 4],
    pub(super) ground_edge: [f32; 4],
    pub(super) grid: [f32; 4],
    pub(super) ink: [f32; 4],
    pub(super) paper: [f32; 4],
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
pub(super) struct RenderCloud {
    pub(super) position: [f32; 3],
    pub(super) scale: f32,
}

#[derive(Clone)]
pub(super) struct RenderWorld {
    pub(super) blocks: Vec<RenderBlock>,
    pub(super) pads: Vec<RenderPad>,
    pub(super) clouds: Vec<RenderCloud>,
    pub(super) ground_size: f32,
    pub(super) grid_size: f32,
    pub(super) grid_divisions: usize,
    pub(super) spawn: [f32; 3],
    pub(super) show_spawn_pad: bool,
    pub(super) palette: RenderPalette,
    pub(super) signs: Vec<RenderSign>,
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
            signs: Vec::new(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Vertex {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) color: [f32; 4],
    pub(super) tex_coords: [f32; 2],
    pub(super) image_invert: f32,
}

impl Vertex {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Float32x2,
            4 => Float32
        ],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Globals {
    pub(super) view_projection: [[f32; 4]; 4],
    pub(super) camera_position: [f32; 4],
    pub(super) sun_direction: [f32; 4],
    pub(super) fog_color: [f32; 4],
}

pub(super) struct Scene {
    pub(super) world: RenderWorld,
    pub(super) agents: Vec<RenderEntity>,
    pub(super) remote_players: Vec<RenderEntity>,
    pub(super) player: RenderEntity,
    pub(super) pad_seconds: Vec<f32>,
    pub(super) player_style: AvatarStyle,
    pub(super) npc_styles: Vec<AvatarStyle>,
    pub(super) camera: [f32; 3],
    pub(super) elapsed: f32,
    pub(super) username: String,
    pub(super) build_blocks: Vec<BuildBlock>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: RenderWorld::default(),
            agents: Vec::new(),
            remote_players: Vec::new(),
            player: RenderEntity::default(),
            pad_seconds: Vec::new(),
            player_style: default_player_style(),
            npc_styles: default_npc_styles(),
            camera: [0.0, -0.095, 8.0],
            elapsed: 0.0,
            username: "PLAYER".to_owned(),
            build_blocks: Vec::new(),
        }
    }
}

pub struct Renderer {
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) globals_buffer: wgpu::Buffer,
    pub(super) globals_bind_group: wgpu::BindGroup,
    pub(super) static_vertex_buffer: wgpu::Buffer,
    pub(super) static_vertex_capacity: usize,
    pub(super) static_vertex_count: usize,
    pub(super) dynamic_vertex_buffer: wgpu::Buffer,
    pub(super) dynamic_vertex_capacity: usize,
    pub(super) ui_pipeline: wgpu::RenderPipeline,
    pub(super) ui_texture_bind_group: wgpu::BindGroup,
    pub(super) ui_vertex_buffer: wgpu::Buffer,
    pub(super) ui_vertex_capacity: usize,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) depth_view: wgpu::TextureView,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) scene: Scene,
    pub(super) package_generation: u32,
    pub(super) active_world: usize,
    pub(super) worlds: Vec<RenderWorld>,
    pub(super) ui_frame: UiFrame,
    rounded_mesh_cache: rounded_geometry::RoundedMeshCache,
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
