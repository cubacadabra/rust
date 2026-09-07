//! Deterministic actor, palette, camera, and world-fixture construction.

use super::*;
use super::{hero, motion};
use crate::character::{Pose as CharacterPose, body_recipe};
use glam::{Mat4, Vec3};
use std::fs::File;
use std::path::Path;

pub(super) fn build_scene(
    config: CaptureConfig,
    scenario: Scenario,
    width: u32,
    height: u32,
) -> (Vec<Vertex>, Vec<RenderEntity>, Globals, [f32; 4]) {
    let mut palette = capture_palette(config.palette);
    if matches!(scenario,Scenario::Hero {..}) {
        palette.sky=color(0xf3ece0);
        palette.ground=color(0xe1d9c9);
    }
    let mut vertices = Vec::with_capacity(50 * 10 * 36);
    let mut rounded_mesh_cache = super::super::rounded_geometry::RoundedMeshCache::default();
    add_cuboid(
        &mut vertices,
        Vec3::new(0.0, -0.08, 0.0),
        Vec3::new(120.0, 0.16, 120.0),
        palette.ground,
    );

    let mut actors = Vec::new();
    let mut camera = Camera::Third;
    let mut target = Vec3::new(0.0, 1.78, 0.0);
    let mut distance = 8.0;
    let mut raised = false;
    match scenario {
        Scenario::HairReview => {
            vertices.clear();
            palette.sky = color(0xc6d2d1);
            for (row, body) in [crate::character::BodyId::PersonGirl, crate::character::BodyId::PersonNonbinary].into_iter().enumerate() {
                for (col, yaw) in [0.0, -std::f32::consts::FRAC_PI_2, std::f32::consts::PI].into_iter().enumerate() {
                    actors.push(RenderEntity {
                        body,
                        position: [2.3 - col as f32 * 2.3, 1.0 - row as f32 * 2.0, 0.0],
                        yaw,
                        pose: CharacterPose::rest(&body_recipe(body).rig),
                        face: crate::character::FaceParameters::preset(crate::character::FacePreset::Happy),
                        ..Default::default()
                    });
                }
            }
        }
        Scenario::Single {
            remote,
            pose,
            camera: view,
            ..
        } => {
            camera = view;
            if !(matches!(view, Camera::First) && !remote) {
                let (y, walk_cycle) = pose_values(pose, config.pose_time);
                let z = if matches!(view, Camera::First) {
                    -7.0
                } else {
                    0.0
                };
                actors.push(RenderEntity {
                    position: [0.0, y, z],
                    yaw: 0.0,
                    walk_cycle,
                    moving: !matches!(pose, Pose::Idle),
                    sprinting: matches!(pose, Pose::Sprint),
                    legacy_assembled: matches!(pose, Pose::Sprint) && !remote,
                    body: crate::character::BodyId::Person,
                    ..Default::default()
                });
            }
        }
        Scenario::Raised => {
            raised = true;
            target = Vec3::new(0.0, 1.8, -2.5);
            actors.push(RenderEntity {
                position: [0.0, 2.0, -2.5],
                yaw: 0.0,
                walk_cycle: 0.9,
                moving: true,
                sprinting: false,
                legacy_assembled: false,
                body: crate::character::BodyId::Person,
                ..Default::default()
            });
        }
        Scenario::Crowd { count, .. } => {
            distance = 18.0;
            actors = crowd_actors(count, config.seed);
        }
        Scenario::ShapeLineup { camera_yaw, .. } => {
            distance = 10.0;
            target = Vec3::new(0.0, 1.58, 0.0);
            let line_axis = Vec3::new(camera_yaw.cos(), 0.0, -camera_yaw.sin());
            actors = [
                (crate::character::BodyId::Person, -2.2),
                (crate::character::BodyId::Cat, 0.0),
                (crate::character::BodyId::Dragon, 2.2),
            ]
            .into_iter()
            .map(|(body, x)| RenderEntity {
                position: (line_axis * x).to_array(),
                yaw: 0.0,
                walk_cycle: 0.0,
                moving: false,
                sprinting: false,
                legacy_assembled: false,
                body,
                ..Default::default()
            })
            .collect();
        }
        Scenario::MotionLineup => {
            distance = 8.5;
            target = Vec3::new(0.0, 2.8, 0.0);
            actors = motion::actors(config.pose_time);
        }
        Scenario::MotionMoving | Scenario::MotionMovingRaised => {
            let raised = matches!(scenario, Scenario::MotionMovingRaised);
            distance = 8.5;
            let focus = motion::moving_focus(config.pose_time, raised);
            target = focus + Vec3::new(0.0, 2.8, 0.0);
            actors = motion::moving_actors(config.pose_time, raised);
            motion::add_world_markers(&mut vertices, &palette, raised);
            if raised {
                add_cuboid(
                    &mut vertices,
                    Vec3::new(0.0, 1.0, focus.z),
                    Vec3::new(5.0, 2.0, 5.0),
                    palette.platform,
                );
            }
        }
        Scenario::WardrobeLineup { camera_yaw, .. } => {
            distance = 10.0;
            target = Vec3::new(0.0, 1.85, 0.0);
            let line_axis = Vec3::new(camera_yaw.cos(), 0.0, -camera_yaw.sin());
            let lineup = [
                (
                    crate::character::BodyId::Person,
                    crate::character::OutfitId::EverydayHoodie,
                ),
                (
                    crate::character::BodyId::Cat,
                    crate::character::OutfitId::PufferExplorer,
                ),
                (
                    crate::character::BodyId::Cat,
                    crate::character::OutfitId::GlossyRaincoat,
                ),
                (
                    crate::character::BodyId::Dragon,
                    crate::character::OutfitId::StarWizard,
                ),
                (
                    crate::character::BodyId::Dragon,
                    crate::character::OutfitId::ToyKnight,
                ),
                (
                    crate::character::BodyId::Person,
                    crate::character::OutfitId::FuzzyPajamas,
                ),
            ];
            actors = lineup
                .into_iter()
                .enumerate()
                .map(|(index, (body, outfit))| RenderEntity {
                    position: (line_axis * (index as f32 * 2.05 - 5.125)).to_array(),
                    yaw: 0.0,
                    body,
                    outfit,
                    ..Default::default()
                })
                .collect();
        }
        Scenario::Orbit { distance, .. } => {
            actors.push(RenderEntity {
                camera_fade: super::super::camera::fade(distance),
                face: crate::character::FaceParameters::preset(crate::character::FacePreset::Happy),
                ..Default::default()
            });
        }
        Scenario::Hero {motion,..} => {
            let mut actor = if motion {motion::actors(config.pose_time)[0]} else {hero::actor(config.pose_time)};
            actor.position[0]=0.0;
            actor.position[2]=0.0;
            let alpha=0.32/(1.0+actor.position[1].max(0.0)*0.8);
            actors.push(actor);
            super::super::add_soft_support_shadow(&mut vertices,Vec3::new(0.0,0.011,-0.04),0.92,[0.13,0.18,0.16,alpha]);
        }
    }
    if raised {
        add_cuboid(
            &mut vertices,
            Vec3::new(0.0, 1.0, -2.5),
            Vec3::new(5.0, 2.0, 5.0),
            palette.platform,
        );
    }
    let shape_silhouette = matches!(
        scenario,
        Scenario::ShapeLineup {
            silhouette: true,
            ..
        }
    );
    for actor in &actors {
        match config.avatar {
            CaptureAvatar::Legacy => {
                add_legacy_avatar(&mut vertices, *actor, palette.avatar, palette.ink)
            }
            CaptureAvatar::Rounded => add_avatar(
                &mut vertices,
                *actor,
                palette.avatar,
                palette.ink,
                &mut rounded_mesh_cache,
            ),
            CaptureAvatar::ShapeProof => {
                let style = if shape_silhouette {
                    super::super::AvatarStyle {
                        skin: color(0x10242b),
                        shirt: color(0x10242b),
                        pants: color(0x10242b),
                        shoes: color(0x10242b),
                        ..super::super::default_player_style()
                    }
                } else {
                    match actor.body {
                        crate::character::BodyId::Person
                        | crate::character::BodyId::PersonGirl
                        | crate::character::BodyId::PersonNonbinary => palette.avatar,
                        crate::character::BodyId::Cat => super::super::AvatarStyle {
                            skin: color(0xc98464),
                            shirt: color(0x5f8f78),
                            pants: color(0x536a90),
                            shoes: color(0x293a43),
                            ..super::super::default_player_style()
                        },
                        crate::character::BodyId::Dragon => super::super::AvatarStyle {
                            skin: color(0x82b78f),
                            shirt: color(0x694c88),
                            pants: color(0x536a90),
                            shoes: color(0x293a43),
                            ..super::super::default_player_style()
                        },
                    }
                };
                super::super::character::add_character(
                    &mut vertices,
                    *actor,
                    actor.body,
                    style,
                    if shape_silhouette {
                        style.skin
                    } else {
                        palette.ink
                    },
                    &mut rounded_mesh_cache,
                );
            }
            CaptureAvatar::Wardrobe | CaptureAvatar::Magic => {}
        }
    }

    let (camera_position, look_target) = match camera {
        Camera::First => {
            let position = Vec3::new(0.0, 3.4, 0.0);
            (position, position + Vec3::new(0.0, 0.0, -1.0))
        }
        Camera::Third => {
            let vertical = (distance * (-0.095_f32).sin()).clamp(-2.0, distance);
            let camera_yaw = match scenario {
                Scenario::ShapeLineup { camera_yaw, .. } | Scenario::WardrobeLineup { camera_yaw, .. } => camera_yaw,
                Scenario::MotionLineup | Scenario::MotionMoving | Scenario::MotionMovingRaised => {
                    motion::CAMERA_YAW
                }
                _ => 0.0,
            };
            let position = target
                + Vec3::new(
                    camera_yaw.sin() * distance,
                    vertical,
                    camera_yaw.cos() * distance,
                );
            (position, target)
        }
    };
    let (camera_position, look_target) = if let Scenario::Orbit { yaw, pitch, distance, .. } | Scenario::Hero { yaw, pitch, distance, .. } = scenario {
        super::super::camera::orbit(Vec3::ZERO, crate::character::BodyId::Person, yaw, pitch, distance)
    } else { (camera_position, look_target) };
    let aspect = viewport_aspect(width, height);
    let view_projection = Mat4::perspective_rh(62.0_f32.to_radians(), aspect, 0.05, 240.0)
        * Mat4::look_at_rh(camera_position, look_target, Vec3::Y);
    let (camera_position, view_projection) = if matches!(scenario, Scenario::HairReview) {
        let position = Vec3::new(0.0, 3.02, -10.0);
        let half_height = 2.18;
        let projection = Mat4::orthographic_rh(-half_height * aspect, half_height * aspect,
            -half_height, half_height, 0.05, 240.0);
        (position, projection * Mat4::look_at_rh(position, Vec3::new(0.0, 3.02, 0.0), Vec3::Y))
    } else { (camera_position, view_projection) };
    let globals = Globals {
        view_projection: view_projection.to_cols_array_2d(),
        camera_position: camera_position.extend(1.0).to_array(),
        sun_direction: Vec3::new(-0.45, -0.82, 0.32)
            .normalize()
            .extend(0.0)
            .to_array(),
        fog_color: palette.sky,
    };
    (vertices, actors, globals, palette.sky)
}

pub(super) fn capture_palette(palette: CapturePalette) -> CaptureColors {
    match palette {
        CapturePalette::Current => CaptureColors {
            avatar: super::super::default_player_style(),
            sky: RenderPalette::default().sky,
            ground: RenderPalette::default().ground,
            platform: color(0xd0a86f),
            ink: RenderPalette::default().ink,
        },
        CapturePalette::HighContrast => CaptureColors {
            avatar: super::super::AvatarStyle {
                skin: color(0xffc18f),
                shirt: color(0x176b87),
                pants: color(0x313a72),
                shoes: color(0x141c2b),
                ..super::super::default_player_style()
            },
            sky: color(0xd9edf0),
            ground: color(0xc2d6a5),
            platform: color(0xe2a04f),
            ink: color(0x102f3c),
        },
    }
}

pub(super) struct CaptureColors {
    pub(super) avatar: super::super::AvatarStyle,
    pub(super) sky: [f32; 4],
    pub(super) ground: [f32; 4],
    pub(super) platform: [f32; 4],
    pub(super) ink: [f32; 4],
}

pub(super) fn pose_values(pose: Pose, pose_time: f32) -> (f32, f32) {
    let time = if pose_time.is_finite() {
        pose_time.max(0.0)
    } else {
        0.0
    };
    match pose {
        Pose::Idle => (0.0, 0.0),
        Pose::Walk => (0.0, time * 6.4),
        Pose::Sprint => (0.0, time * 11.5),
        Pose::Jump => (1.15, time * 6.4),
    }
}

pub(super) fn crowd_actors(count: usize, seed: u64) -> Vec<RenderEntity> {
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns.max(1));
    let spacing_x = if count >= 50 { 2.2 } else { 3.1 };
    let spacing_z = if count >= 50 { 2.4 } else { 3.4 };
    let mut random = seed;
    (0..count)
        .map(|index| {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let jitter_x = ((random >> 32) as f32 / u32::MAX as f32 - 0.5) * 0.35;
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let jitter_z = ((random >> 32) as f32 / u32::MAX as f32 - 0.5) * 0.35;
            let column = index % columns.max(1);
            let row = index / columns.max(1);
            RenderEntity {
                position: [
                    (column as f32 - (columns as f32 - 1.0) * 0.5) * spacing_x + jitter_x,
                    0.0,
                    (row as f32 - (rows as f32 - 1.0) * 0.5) * spacing_z + jitter_z,
                ],
                yaw: if index % 2 == 0 {
                    0.0
                } else {
                    std::f32::consts::PI
                },
                walk_cycle: index as f32 * 0.37,
                moving: true,
                sprinting: false,
                legacy_assembled: false,
                body: crate::character::BodyId::ALL[index % crate::character::BodyId::ALL.len()],
                ..Default::default()
            }
        })
        .collect()
}

pub(super) fn dimensions(config: CaptureConfig, scenario: Scenario) -> (u32, u32) {
    let (width, height) = match scenario {
        Scenario::Crowd { portrait: true, .. } => (config.portrait_width, config.portrait_height),
        _ => (config.width, config.height),
    };
    let scale = config.quality.scale();
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

pub(super) fn viewport_aspect(width: u32, height: u32) -> f32 {
    let viewport = world_viewport(width, height);
    (viewport[2] as f32 / viewport[3].max(1) as f32).max(0.1)
}

pub(super) fn world_viewport(width: u32, height: u32) -> [u32; 4] {
    let aspect = width as f32 / height.max(1) as f32;
    if aspect >= 1.25 {
        return [0, 0, width, height];
    }
    let viewport_height = (width as f32 / WORLD_ASPECT).min(height as f32);
    [
        0,
        ((height as f32 - viewport_height) * 0.5).round() as u32,
        width,
        viewport_height.round().max(1.0) as u32,
    ]
}

pub(super) fn align_to(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

pub(super) fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
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
