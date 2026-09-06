//! Deterministic presentation review: simulate at 60 Hz, save at 30 Hz.
use super::*;
use crate::character::{
    body_recipe, foot_is_planted, BodyId, CharacterPresentationState, JointId, OutfitId,
};
use crate::types::{
    CharacterEmote, CharacterEntityKey, CharacterEntityKind, CharacterMotionEvent,
    CharacterMotionSample, CharacterMotionSource, CharacterSupport,
};
use super::super::character::{Feature, Part};
use super::super::hero_character::{self, Study};
use super::super::hero_geometry;
use glam::{Mat4, Quat, Vec2};

pub(super) const CAMERA_YAW: f32 = 2.75;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionCaptureMode {
    #[default]
    Staged,
    Moving,
    MovingRaised,
}

fn sample_at_support(tick: u64, slot: usize, support_height: f32) -> CharacterMotionSample {
    let time = tick as f32 / 60.0;
    let speed = if (60..180).contains(&tick) {
        6.4
    } else if (180..300).contains(&tick) {
        11.5
    } else if (300..360).contains(&tick) {
        6.4
    } else if (420..480).contains(&tick) {
        6.4
    } else {
        0.0
    };
    let straight_travel = (time - 1.0).clamp(0.0, 2.0) * 6.4
        + (time - 3.0).clamp(0.0, 2.0) * 11.5
        + (time - 5.0).clamp(0.0, 1.0) * 6.4;
    let turn_distance = (time - 7.0).clamp(0.0, 1.0) * 6.4;
    let travel = straight_travel + turn_distance;
    let airborne = (300..360).contains(&tick);
    let jump_age = time - 5.0;
    let turned = time >= 7.0;
    let position_x = if turned { turn_distance * 0.38 } else { 0.0 };
    let position_z = if turned {
        -straight_travel - turn_distance * 0.92
    } else {
        -straight_travel
    };
    let facing_yaw = if turned { 0.39 } else { 0.0 };
    CharacterMotionSample {
        key: CharacterEntityKey {
            kind: CharacterEntityKind::LocalNpc,
            slot,
            generation: 1,
            identity: 0,
        },
        sequence: tick + 1,
        time,
        position: [
            position_x,
            if airborne {
                support_height + 10.5 * jump_age * (1.0 - jump_age)
            } else {
                support_height
            },
            position_z,
        ],
        facing_yaw,
        look_yaw: if time >= 8.5 {
            ((time - 8.5) * 2.0).sin() * 0.7
        } else {
            0.0
        },
        planar_velocity: Some(if turned && speed > 0.0 {
            [speed * 0.38, -speed * 0.92]
        } else {
            [0.0, -speed]
        }),
        vertical_velocity: Some(if airborne {
            10.5 - 21.0 * jump_age
        } else {
            0.0
        }),
        support: if airborne {
            CharacterSupport::Airborne
        } else {
            CharacterSupport::Grounded {
                height: support_height,
            }
        },
        stride_phase: crate::player::walk_cycle_delta(travel),
        moving: speed > 0.0,
        sprinting: (180..300).contains(&tick),
        source: CharacterMotionSource::Simulation,
        event: match tick {
            300 => CharacterMotionEvent::Takeoff,
            360 => CharacterMotionEvent::Landing,
            _ => CharacterMotionEvent::None,
        },
        emote: if tick == 420 {
            CharacterEmote::Wave
        } else {
            CharacterEmote::None
        },
        emote_sequence: u64::from(tick >= 420),
        appearance_revision: 0,
    }
}

#[cfg(test)]
fn sample(tick: u64, slot: usize) -> CharacterMotionSample {
    sample_at_support(tick, slot, 0.0)
}

pub(super) fn actors(time: f32) -> Vec<RenderEntity> {
    actors_with_mode(time, MotionCaptureMode::Staged)
}

pub(super) fn moving_actors(time: f32, raised: bool) -> Vec<RenderEntity> {
    actors_with_mode(
        time,
        if raised {
            MotionCaptureMode::MovingRaised
        } else {
            MotionCaptureMode::Moving
        },
    )
}

fn actors_with_mode(time: f32, mode: MotionCaptureMode) -> Vec<RenderEntity> {
    let tick = (time.clamp(0.0, 10.0) * 60.0).round() as u64;
    let support_height = if matches!(mode, MotionCaptureMode::MovingRaised) {
        2.0
    } else {
        0.0
    };
    BodyId::ALL
        .into_iter()
        .enumerate()
        .map(|(slot, body)| {
            let initial = sample_at_support(0, slot, support_height);
            let mut state = CharacterPresentationState::new(initial.key, body);
            let mut output = state.evaluate(initial, body, false);
            for step in 1..=tick {
                output = state.evaluate(
                    sample_at_support(step, slot, support_height),
                    body,
                    false,
                );
            }
            let motion = sample_at_support(tick, slot, support_height);
            let axis = Vec3::new(CAMERA_YAW.cos(), 0.0, -CAMERA_YAW.sin());
            let lineup = axis * (slot as f32 - 1.0) * 2.6;
            let root = if matches!(mode, MotionCaptureMode::Staged) {
                lineup + Vec3::Y * motion.position[1]
            } else {
                Vec3::from_array(motion.position) + lineup
            };
            let mut secondary = output.secondary;
            if body == BodyId::Person {
                secondary.left_foot_target = secondary
                    .left_foot_target
                    .map(|target| target + lineup);
                secondary.right_foot_target = secondary
                    .right_foot_target
                    .map(|target| target + lineup);
            }
            RenderEntity {
                position: root.to_array(),
                body,
                outfit: match body {
                    BodyId::Person => OutfitId::EverydayHoodie,
                    BodyId::Cat => OutfitId::PufferExplorer,
                    BodyId::Dragon => OutfitId::ToyKnight,
                },
                moving: motion.moving,
                sprinting: motion.sprinting,
                walk_cycle: motion.stride_phase,
                support: motion.support,
                pose: output.pose,
                face: output.face,
                secondary,
                ..Default::default()
            }
        })
        .collect()
}

pub(super) fn moving_focus(time: f32, raised: bool) -> Vec3 {
    let tick = (time.clamp(0.0, 10.0) * 60.0).round() as u64;
    let support_height = if raised { 2.0 } else { 0.0 };
    Vec3::from_array(sample_at_support(tick, 0, support_height).position)
}

pub(super) fn add_world_markers(
    vertices: &mut Vec<Vertex>,
    palette: &super::CaptureColors,
    raised: bool,
) {
    let y = if raised { 2.006 } else { 0.006 };
    for index in -18..=3 {
        let z = index as f32 * crate::player::WALK_CYCLE_DISTANCE;
        add_cuboid(
            vertices,
            Vec3::new(0.0, y, z),
            Vec3::new(8.0, 0.012, 0.025),
            palette.ink,
        );
        add_cuboid(
            vertices,
            Vec3::new(0.0, y, z),
            Vec3::new(0.025, 0.012, 0.34),
            palette.ink,
        );
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ContactDiagnostic {
    pub support_height: f32,
    pub samples: usize,
    pub max_horizontal_drift: f32,
    pub max_vertical_error: f32,
    pub walk_max_horizontal_drift: f32,
    pub sprint_max_horizontal_drift: f32,
}

fn sole_contact_landmark(entity: RenderEntity, part: Part) -> Vec3 {
    let recipe = body_recipe(BodyId::Person);
    let pose = hero_character::fit_pose(entity, Study::Everyday, &recipe.rig);
    let joints = recipe.rig.world_matrices(&pose.transforms);
    let root = Mat4::from_rotation_translation(
        Quat::from_rotation_y(entity.yaw),
        Vec3::from_array(entity.position),
    );
    let transform = root
        * joints[part.anchor.joint.index()]
        * part.anchor.local
        * Mat4::from_scale(part.spec.size);
    let mesh = hero_geometry::build(part.shape, Vec3::ONE, CharacterLod::Mid.subdivisions());
    let world_vertices: Vec<Vec3> = mesh
        .vertices
        .iter()
        .map(|vertex| transform.transform_point3(vertex.position))
        .collect();
    let minimum_y = world_vertices
        .iter()
        .map(|vertex| vertex.y)
        .fold(f32::INFINITY, f32::min);
    let contacts: Vec<Vec3> = world_vertices
        .into_iter()
        .filter(|vertex| vertex.y <= minimum_y + 0.0005)
        .collect();
    contacts.iter().copied().sum::<Vec3>() / contacts.len().max(1) as f32
}

pub(super) fn measure_contact_diagnostic(raised: bool) -> ContactDiagnostic {
    let support_height = if raised { 2.0 } else { 0.0 };
    let recipe = body_recipe(BodyId::Person);
    let parts = super::super::character::parts_for(&recipe, OutfitId::EverydayHoodie);
    let soles: Vec<Part> = parts
        .into_iter()
        .filter(|part| {
            matches!(part.feature, Feature::Sole)
                && matches!(part.anchor.joint, JointId::LeftFoot | JointId::RightFoot)
        })
        .collect();
    let mut first_contact = [None; 2];
    let mut max_horizontal_drift: f32 = 0.0;
    let mut max_vertical_error: f32 = 0.0;
    let mut walk_max_horizontal_drift: f32 = 0.0;
    let mut sprint_max_horizontal_drift: f32 = 0.0;
    let mut samples = 0;
    for tick in 60..=299 {
        let time = tick as f32 / 60.0;
        let person = moving_actors(time, raised)[0];
        let phase = person.walk_cycle;
        for part in &soles {
            let index = if part.anchor.joint == JointId::LeftFoot { 0 } else { 1 };
            let offset = if index == 0 { 0.0 } else { std::f32::consts::PI };
            if !foot_is_planted(phase + offset) {
                first_contact[index] = None;
                continue;
            }
            let landmark = sole_contact_landmark(person, *part);
            let baseline = first_contact[index].get_or_insert(landmark);
            let horizontal = Vec2::new(landmark.x - baseline.x, landmark.z - baseline.z).length();
            max_horizontal_drift = max_horizontal_drift.max(horizontal);
            if (180..300).contains(&tick) {
                sprint_max_horizontal_drift = sprint_max_horizontal_drift.max(horizontal);
            } else {
                walk_max_horizontal_drift = walk_max_horizontal_drift.max(horizontal);
            }
            let vertical_error = (landmark.y - support_height).abs();
            max_vertical_error = max_vertical_error.max(vertical_error);
            samples += 1;
        }
    }
    ContactDiagnostic {
        support_height,
        samples,
        max_horizontal_drift,
        max_vertical_error,
        walk_max_horizontal_drift,
        sprint_max_horizontal_drift,
    }
}

/// Save a ten-second, three-species motion review using live presentation.
pub fn capture_phase4_motion(
    output_dir: impl AsRef<Path>,
    config: CaptureConfig,
) -> Result<CaptureReport, String> {
    capture_phase4_motion_with_mode(output_dir, config, MotionCaptureMode::Staged)
}

pub fn capture_phase4_motion_with_mode(
    output_dir: impl AsRef<Path>,
    mut config: CaptureConfig,
    mode: MotionCaptureMode,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    config.avatar = CaptureAvatar::Magic;
    config.pose_time = 0.0;
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;
    let mut context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };
    let mut captures = Vec::with_capacity(301);
    for frame in 0..=300 {
        let mut frame_config = config;
        frame_config.pose_time = frame as f32 / 30.0;
        let scenario = match mode {
            MotionCaptureMode::Staged => Scenario::MotionLineup,
            MotionCaptureMode::Moving => Scenario::MotionMoving,
            MotionCaptureMode::MovingRaised => Scenario::MotionMovingRaised,
        };
        let mut capture = context.capture(output_dir, frame_config, scenario)?;
        let name = format!("motion-{frame:04}");
        let image = format!("{name}.png");
        fs::rename(output_dir.join(&capture.image), output_dir.join(&image))
            .map_err(|error| format!("save motion frame: {error}"))?;
        capture.name = name;
        capture.image = image;
        captures.push(capture);
    }
    let diagnostic = match mode {
        MotionCaptureMode::Staged => None,
        MotionCaptureMode::Moving => Some(measure_contact_diagnostic(false)),
        MotionCaptureMode::MovingRaised => Some(measure_contact_diagnostic(true)),
    };
    if let Some(diagnostic) = diagnostic {
        fs::write(
            output_dir.join("phase4_contact_diagnostic.json"),
            serde_json::to_vec_pretty(&diagnostic)
                .map_err(|error| format!("serialize contact diagnostic: {error}"))?,
        )
        .map_err(|error| format!("write contact diagnostic: {error}"))?;
    }
    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "phase-4-gameplay-presentation-motion".to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "301 frames at 30 Hz; presentation evaluates deterministic motion samples at 60 Hz.",
            "0–1s idle/start; 1–3s walk; 3–5s sprint; 5–6s jump/landing; 6–7s stop; 7–8s turn and walk; 8–8.5s blocked stop; 8.5–10s look around.",
            "Uses CharacterPresentationState and the production instanced renderer for all three species.",
            match mode {
                MotionCaptureMode::Staged => {
                    "Travel is staged in place; this reviews presentation, not physics, collision, or input latency."
                }
                MotionCaptureMode::Moving => {
                    "Moving review translates the roots through fixed world markers and reports sole-contact drift."
                }
                MotionCaptureMode::MovingRaised => {
                    "Moving raised review translates the roots over a fixed support plane at height 2.0 and reports sole-contact drift."
                }
            },
        ],
    };
    fs::write(
        output_dir.join("phase4_report.json"),
        serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("serialize motion report: {error}"))?,
    )
    .map_err(|error| format!("write motion report: {error}"))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_review_covers_events_and_settles() {
        assert_eq!(sample(300, 0).event, CharacterMotionEvent::Takeoff);
        assert_eq!(sample(360, 0).event, CharacterMotionEvent::Landing);
        assert_eq!(sample(420, 0).emote, CharacterEmote::Wave);
        assert!(
            actors(5.2)
                .iter()
                .all(|a| a.position[1] > 1.0 && a.secondary.spark_life > 0.0)
        );
        assert!(
            actors(6.1)
                .iter()
                .all(|a| a.position[1] == 0.0 && a.secondary.spark_life > 0.0)
        );
        assert!(actors(7.1).iter().all(|a| a.secondary.spark_life > 0.0));
        assert!(actors(8.5).iter().all(|a| a.secondary.spark_life == 0.0));
        for (a, b) in actors(5.2).iter().zip(actors(5.2)) {
            assert_eq!(a.pose, b.pose);
            assert_eq!(a.face, b.face);
            assert_eq!(a.secondary, b.secondary);
        }
    }

    #[test]
    fn moving_review_locks_generated_sole_contact_on_ground_and_raised_supports() {
        let travel_at_run_start = 2.0 * 6.4;
        assert!(
            (sample(180, 0).stride_phase - crate::player::walk_cycle_delta(travel_at_run_start))
                .abs()
                < 0.0001
        );
        assert!(!sample(360, 0).moving);
        assert!(sample(420, 0).moving);
        assert!(!sample(480, 0).moving);
        assert_eq!(sample(480, 0).position, sample(510, 0).position);

        let staged = actors(2.0)[0];
        let moving = moving_actors(2.0, false)[0];
        assert!((moving.position[2] - staged.position[2]).abs() > 5.0);

        let ground = measure_contact_diagnostic(false);
        assert!(ground.samples > 80, "{ground:?}");
        assert!(ground.max_horizontal_drift <= 0.01, "{ground:?}");
        assert!(ground.max_vertical_error < 0.001);
        assert!(ground.sprint_max_horizontal_drift <= 0.01, "{ground:?}");

        let raised = measure_contact_diagnostic(true);
        assert_eq!(raised.support_height, 2.0);
        assert!(raised.samples > 80, "{raised:?}");
        assert!(raised.max_horizontal_drift <= 0.01, "{raised:?}");
        assert!(raised.max_vertical_error < 0.001, "{raised:?}");
    }
}
