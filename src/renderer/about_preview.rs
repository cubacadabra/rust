use glam::Vec3;

use super::{RenderPalette, Renderer, Scene};
use crate::types::{BuildBlock, CharacterEntityKey, CharacterEntityKind};

pub(super) const ABOUT_WIDTH: u32 = 1_000;
pub(super) const ABOUT_HEIGHT: u32 = 560;

const LETTERS: &[(&str, [[u8; 5]; 5])] = &[
    (
        "C",
        [
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0],
            [1, 0, 0, 0, 0],
            [1, 0, 0, 0, 0],
            [1, 1, 1, 1, 1],
        ],
    ),
    (
        "U",
        [
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [0, 1, 1, 1, 0],
        ],
    ),
    (
        "B",
        [
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
        ],
    ),
    (
        "A",
        [
            [0, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
        ],
    ),
    (
        "C",
        [
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0],
            [1, 0, 0, 0, 0],
            [1, 0, 0, 0, 0],
            [1, 1, 1, 1, 1],
        ],
    ),
    (
        "A",
        [
            [0, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
        ],
    ),
    (
        "D",
        [
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
        ],
    ),
    (
        "A",
        [
            [0, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
        ],
    ),
    (
        "B",
        [
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
        ],
    ),
    (
        "R",
        [
            [1, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 0],
            [1, 0, 1, 0, 0],
            [1, 0, 0, 1, 0],
        ],
    ),
    (
        "A",
        [
            [0, 1, 1, 1, 0],
            [1, 0, 0, 0, 1],
            [1, 1, 1, 1, 1],
            [1, 0, 0, 0, 1],
            [1, 0, 0, 0, 1],
        ],
    ),
];

const LETTER_COLORS: [u32; 11] = [
    0x2d777b, 0xe07a5f, 0xf2cc8f, 0x3d8b8b, 0x2d777b, 0xe07a5f, 0xf2cc8f, 0x3d8b8b, 0x2d777b,
    0xe07a5f, 0xf2cc8f,
];

fn ease(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn about_scene(elapsed: f32) -> Scene {
    let elapsed = elapsed.rem_euclid(15.0);
    let mut scene = Scene::default();
    scene.elapsed = elapsed;
    scene.world = super::RenderWorld {
        hide_default_ground: true,
        show_grid: false,
        show_spawn_pad: false,
        palette: RenderPalette {
            sky: super::color(0x9fc8c5),
            ground: super::color(0xd8b77c),
            ground_edge: super::color(0x668c83),
            grid: super::color(0xc8ded5),
            ink: super::color(0x173f43),
            paper: super::color(0xf7f0df),
        },
        outdoor_ambient: [0.84, 0.84, 0.84],
        sun_brightness: 2.4,
        sun_direction: [-0.55, -0.9, 0.25],
        ..Default::default()
    };

    scene.build_blocks.push(BuildBlock {
        position: [0.0, -0.12, 0.2],
        size: [22.0, 0.24, 7.0],
        color: 0xd8b77c,
        rotation: 0,
    });

    let cell = 0.31;
    let cube = 0.25;
    let letter_width = 5.0 * cell;
    let spacing = 0.18;
    let total_width = LETTERS.len() as f32 * letter_width + (LETTERS.len() - 1) as f32 * spacing;
    for (letter_index, (_, pattern)) in LETTERS.iter().enumerate() {
        let letter_x = -total_width * 0.5 + letter_index as f32 * (letter_width + spacing);
        let settle_start = 0.75 + letter_index as f32 * 0.34;
        for (row, cells) in pattern.iter().enumerate() {
            for (column, on) in cells.iter().enumerate() {
                if *on == 0 {
                    continue;
                }
                let target = Vec3::new(
                    letter_x + column as f32 * cell + cell * 0.5,
                    1.15 + (4 - row) as f32 * cell,
                    -0.35,
                );
                let seed = (letter_index * 31 + row * 7 + column * 3) as f32;
                let start = target
                    + Vec3::new(
                        (seed * 1.73).sin() * 2.5,
                        2.7 + (seed * 0.91).cos() * 1.1,
                        (seed * 0.67).cos() * 1.8,
                    );
                let progress = ease((elapsed - settle_start) / 1.25);
                let position = start.lerp(target, progress);
                scene.build_blocks.push(BuildBlock {
                    position: position.to_array(),
                    size: [cube, cube, cube],
                    color: LETTER_COLORS[letter_index],
                    rotation: 0,
                });
            }
        }
    }

    let walk_phase = (elapsed * 0.55).rem_euclid(1.0);
    let walk_x = -7.0 + (elapsed.min(12.0) / 12.0) * 14.0;
    let direction = if elapsed < 12.0 { 1.0 } else { 0.0 };
    scene.player = super::RenderEntity {
        key: CharacterEntityKey {
            kind: CharacterEntityKind::LocalPlayer,
            ..Default::default()
        },
        position: [walk_x, 0.0, 1.25],
        yaw: if direction > 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        },
        walk_cycle: walk_phase,
        moving: direction > 0.0,
        style: scene.player_style,
        ..Default::default()
    };
    scene.camera = [0.0, -0.1, 13.0];
    scene
}

impl Renderer {
    pub(crate) fn about_preview_texture(&self) -> &wgpu::TextureView {
        &self.about_targets.color
    }

    pub(crate) fn render_about_preview(&mut self, elapsed: f32) {
        let saved_scene = std::mem::replace(&mut self.scene, about_scene(elapsed));
        let saved_width = self.width;
        let saved_height = self.height;
        self.width = ABOUT_WIDTH as f32;
        self.height = ABOUT_HEIGHT as f32;
        self.about_rendering = true;
        #[cfg(feature = "studio-ui")]
        let saved_viewport = self.studio_viewport.take();
        #[cfg(feature = "studio-ui")]
        let saved_camera_preset = std::mem::replace(
            &mut self.studio_camera_preset,
            crate::StudioCameraPreset::Gameplay,
        );
        #[cfg(feature = "studio-ui")]
        let saved_edit_mode = std::mem::replace(&mut self.studio_edit_mode, false);
        std::mem::swap(&mut self.targets, &mut self.about_targets);
        let Some((_, encoder, _)) = self.encode_frame(true) else {
            std::mem::swap(&mut self.targets, &mut self.about_targets);
            self.about_rendering = false;
            self.scene = saved_scene;
            self.width = saved_width;
            self.height = saved_height;
            return;
        };
        self.queue.submit(Some(encoder.finish()));
        std::mem::swap(&mut self.targets, &mut self.about_targets);
        self.about_rendering = false;
        self.scene = saved_scene;
        self.width = saved_width;
        self.height = saved_height;
        #[cfg(feature = "studio-ui")]
        {
            self.studio_viewport = saved_viewport;
            self.studio_camera_preset = saved_camera_preset;
            self.studio_edit_mode = saved_edit_mode;
        }
    }
}
