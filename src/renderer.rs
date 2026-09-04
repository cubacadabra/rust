mod device;
mod draw;
mod scene;
mod ui;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

use crate::types::BuildBlock;
use crate::ui::UiFrame;

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
    pub(super) assembled: f32,
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
}

impl Vertex {
    pub(super) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
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

    let enabled = pad.enabled;
    let disabled_color = faded(palette.ink, 0.32);
    let pad_color = if enabled { pad.color } else { disabled_color };
    let mut inner = pad_color;
    inner[3] = 0.30;
    add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.215, 0.0),
        pad.radius,
        0.025,
        inner,
    );
    let pulse = 1.0 + (elapsed * 2.2 + index as f32 * 1.7).sin() * 0.045;
    let mut ring_color = pad_color;
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
            pad_color,
        );
    }
    add_cuboid(
        vertices,
        origin + Vec3::new(0.0, 2.62, -0.35),
        Vec3::new(5.05, 0.32, 0.38),
        pad_color,
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
        pad_color,
    );
    let label = if enabled {
        format!("{} {}", pad.code, pad.label)
    } else {
        pad.availability_label.clone()
    };
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

pub(super) fn glyph(character: char) -> [u8; 7] {
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
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '&' => [12, 18, 20, 8, 21, 18, 13],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
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
