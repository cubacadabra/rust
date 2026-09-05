//! Deterministic, CPU-only character presentation.
//!
//! The animation layer consumes typed motion samples. It never writes player
//! position, velocity, or gameplay state, which keeps cosmetic motion safe to
//! run at a different rate from simulation and rendering.

use super::{body_recipe, BodyId, FaceParameters, FacePreset, JointId, Pose};
use crate::math::damp;
use crate::types::{
    CharacterEmote, CharacterEntityKey, CharacterMotionEvent, CharacterMotionSample,
    CharacterSupport,
};
use glam::{EulerRot, Quat, Vec2};

const MAX_PRESENTATION_DELTA: f32 = 0.05;
const TELEPORT_DISTANCE: f32 = 5.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SecondaryMotion {
    pub(crate) tail_sway: f32,
    pub(crate) ear_tilt: f32,
    pub(crate) wing_flap: f32,
    /// A small, bounded multiplier used by seam cores to make joint gaps
    /// feel springy without moving the gameplay collider.
    pub(crate) gap_expansion: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AnimationOutput {
    pub(crate) pose: Pose,
    pub(crate) face: FaceParameters,
    pub(crate) secondary: SecondaryMotion,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CharacterPresentationState {
    key: CharacterEntityKey,
    body: BodyId,
    seed: u32,
    last_sequence: Option<u64>,
    last_time: f32,
    last_position: [f32; 3],
    locomotion_blend: f32,
    landing_timer: f32,
    gap_spring: f32,
    head_look: f32,
    face: FaceParameters,
    expression: FacePreset,
    blink_until: f32,
    blink_count: u32,
    next_blink: f32,
    wave_until: f32,
    output: Option<AnimationOutput>,
}

impl CharacterPresentationState {
    pub(crate) fn new(key: CharacterEntityKey, body: BodyId) -> Self {
        let seed = presentation_seed(key);
        let expression = default_expression(body);
        Self {
            key,
            body,
            seed,
            last_sequence: None,
            last_time: 0.0,
            last_position: [0.0; 3],
            locomotion_blend: 0.0,
            landing_timer: 0.0,
            gap_spring: 0.0,
            head_look: 0.0,
            face: FaceParameters::preset(expression),
            expression,
            blink_until: 0.0,
            blink_count: 0,
            next_blink: next_blink(seed, 0),
            wave_until: 0.0,
            output: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_expression(&mut self, expression: FacePreset) {
        self.expression = expression;
    }

    pub(crate) fn reset(&mut self, body: BodyId) {
        let key = self.key;
        *self = Self::new(key, body);
    }

    /// Evaluate at most once per motion sequence. This is important because a
    /// host may call sync and draw more than once for the same engine tick.
    pub(crate) fn evaluate(
        &mut self,
        sample: CharacterMotionSample,
        body: BodyId,
        reduced_effects: bool,
    ) -> AnimationOutput {
        if self.key != sample.key || self.body != body {
            self.key = sample.key;
            self.reset(body);
        }
        if self.last_sequence == Some(sample.sequence) {
            return self.output.expect("presentation output initialized");
        }

        let position_delta = if self.last_sequence.is_some() {
            Vec2::new(
                sample.position[0] - self.last_position[0],
                sample.position[2] - self.last_position[2],
            )
            .length()
        } else {
            0.0
        };
        let position_delta = if position_delta.is_finite() {
            position_delta
        } else {
            f32::INFINITY
        };
        let discontinuity = self.last_sequence.is_some()
            && (position_delta > TELEPORT_DISTANCE
                || sample.sequence < self.last_sequence.unwrap());
        let delta = if self.last_sequence.is_some() && !discontinuity {
            (sample.time - self.last_time).clamp(0.0, MAX_PRESENTATION_DELTA)
        } else {
            0.0
        };
        if discontinuity {
            self.locomotion_blend = 0.0;
            self.landing_timer = 0.0;
            self.gap_spring = 0.0;
            self.head_look = 0.0;
        }

        let time = if sample.time.is_finite() {
            sample.time.max(0.0)
        } else {
            0.0
        };
        let estimated_speed = if delta > 0.0001 {
            position_delta / delta
        } else {
            0.0
        };
        let speed = sample
            .planar_velocity
            .map(|velocity| velocity[0].hypot(velocity[1]))
            .filter(|speed| speed.is_finite())
            .unwrap_or_else(|| {
                if self.last_sequence.is_some() {
                    estimated_speed.max(0.0)
                } else if sample.moving {
                    if sample.sprinting {
                        11.5
                    } else {
                        6.4
                    }
                } else {
                    0.0
                }
            });
        let speed_factor = (speed / 11.5).clamp(0.0, 1.0);
        let target_blend = if sample.moving {
            if sample.planar_velocity.is_some() {
                speed_factor
            } else {
                speed_factor.max(if sample.sprinting { 0.72 } else { 0.2 })
            }
        } else {
            0.0
        };
        self.locomotion_blend = damp(self.locomotion_blend, target_blend, 16.0, delta);

        if sample.event == CharacterMotionEvent::Landing {
            self.landing_timer = 0.24;
        } else if sample.event == CharacterMotionEvent::Takeoff {
            self.landing_timer = 0.0;
        } else {
            self.landing_timer = (self.landing_timer - delta).max(0.0);
        }

        let vertical_velocity = sample
            .vertical_velocity
            .unwrap_or_else(|| {
                if delta > 0.0001 {
                    ((sample.position[1] - self.last_position[1]) / delta).clamp(-20.0, 20.0)
                } else {
                    0.0
                }
            })
            .finite_or_zero();
        let look_delta = if (sample.look_yaw - sample.facing_yaw).is_finite() {
            shortest_angle(sample.look_yaw - sample.facing_yaw).clamp(-0.9, 0.9)
        } else {
            0.0
        };
        self.head_look = damp(self.head_look, look_delta, 12.0, delta);
        let gap_target: f32 = if sample.moving { 0.18 } else { 0.0 }
            + if matches!(sample.support, CharacterSupport::Airborne) {
                0.24
            } else {
                0.0
            }
            + if matches!(
                sample.event,
                CharacterMotionEvent::Takeoff | CharacterMotionEvent::Landing
            ) {
                0.30
            } else {
                0.0
            };
        let spring_rate = if reduced_effects { 20.0 } else { 13.0 };
        self.gap_spring = damp(
            self.gap_spring,
            gap_target.clamp(0.0, 0.72),
            spring_rate,
            delta,
        );

        let mut pose = Pose::rest(&body_recipe(body).rig);
        let phase = if sample.stride_phase.is_finite() {
            sample.stride_phase
        } else {
            0.0
        };
        let run = if sample.sprinting { 1.0 } else { 0.0 };
        let swing = phase.sin() * (0.34 + run * 0.22) * self.locomotion_blend;
        rotate(&mut pose, JointId::LeftUpperArm, swing * 0.72, 0.0, 0.0);
        rotate(&mut pose, JointId::RightUpperArm, -swing * 0.72, 0.0, 0.0);
        rotate(
            &mut pose,
            JointId::LeftUpperLeg,
            -swing * (1.0 + run * 0.15),
            0.0,
            0.0,
        );
        rotate(
            &mut pose,
            JointId::RightUpperLeg,
            swing * (1.0 + run * 0.15),
            0.0,
            0.0,
        );
        rotate(
            &mut pose,
            JointId::LeftLowerLeg,
            swing.abs() * 0.24,
            0.0,
            0.0,
        );
        rotate(
            &mut pose,
            JointId::RightLowerLeg,
            -swing.abs() * 0.24,
            0.0,
            0.0,
        );

        // Breathing is deliberately a torso/head rotation, not root motion.
        let breath = (time * 1.75 + seed_unit(self.seed) * 2.0).sin()
            * 0.018
            * (1.0 - self.locomotion_blend * 0.35);
        rotate(&mut pose, JointId::Torso, breath, 0.0, 0.0);
        rotate(
            &mut pose,
            JointId::Torso,
            -run * 0.075 * self.locomotion_blend,
            0.0,
            0.0,
        );
        rotate(
            &mut pose,
            JointId::Head,
            -breath * 1.5,
            self.head_look * 0.72,
            0.0,
        );
        rotate(&mut pose, JointId::Torso, 0.0, self.head_look * 0.14, 0.0);

        // Air poses are driven by explicit support/vertical velocity. A raised
        // block therefore reads as ground, while a ledge fall still animates.
        if !matches!(sample.support, CharacterSupport::Grounded { .. })
            || vertical_velocity.abs() > 0.5
        {
            let rising = (vertical_velocity / 10.5).clamp(-1.0, 1.0);
            rotate(&mut pose, JointId::Torso, -rising * 0.10, 0.0, 0.0);
            rotate(
                &mut pose,
                JointId::LeftUpperArm,
                -0.22 - rising * 0.22,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::RightUpperArm,
                -0.22 - rising * 0.22,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::LeftUpperLeg,
                0.18 + rising * 0.16,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::RightUpperLeg,
                0.18 + rising * 0.16,
                0.0,
                0.0,
            );
        }
        if self.landing_timer > 0.0 {
            let compression = (self.landing_timer / 0.24).smoothstep(0.0, 1.0);
            rotate(&mut pose, JointId::Torso, compression * 0.15, 0.0, 0.0);
            rotate(
                &mut pose,
                JointId::LeftUpperLeg,
                -compression * 0.20,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::RightUpperLeg,
                -compression * 0.20,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::LeftUpperArm,
                compression * 0.15,
                0.0,
                0.0,
            );
            rotate(
                &mut pose,
                JointId::RightUpperArm,
                compression * 0.15,
                0.0,
                0.0,
            );
        }

        if sample.emote == CharacterEmote::Wave {
            self.wave_until = time + 0.85;
        }
        let waving = time < self.wave_until;
        if waving {
            let wave = (time * 9.0).sin() * 0.16;
            rotate(&mut pose, JointId::RightUpperArm, -1.0 + wave, 0.0, -0.16);
            rotate(
                &mut pose,
                JointId::RightLowerArm,
                -0.55 + wave * 0.5,
                0.0,
                0.0,
            );
            rotate(&mut pose, JointId::Head, 0.0, self.head_look * 0.5, 0.08);
        }

        let secondary_scale = if reduced_effects { 0.5 } else { 1.0 };
        let secondary = SecondaryMotion {
            tail_sway: (time * 2.3 + seed_unit(self.seed) * 5.0).sin() * 0.16 * secondary_scale,
            ear_tilt: (time * 1.7 + 1.0).sin() * 0.07 * secondary_scale,
            wing_flap: (time * 2.0 + 2.0).sin() * 0.10 * secondary_scale,
            gap_expansion: self.gap_spring,
        };

        let target_face = FaceParameters::preset(self.expression).clamped();
        self.face = blend_face(self.face, target_face, delta);
        if time >= self.next_blink && time >= self.blink_until {
            self.blink_count = self.blink_count.wrapping_add(1);
            self.blink_until = time + 0.12;
            self.next_blink = time + next_blink(self.seed, self.blink_count);
        }
        let mut face = self.face;
        if time < self.blink_until {
            face.eye_opening = 0.06;
        }
        let look_idle = (1.0 - self.locomotion_blend).clamp(0.0, 1.0);
        face.look.x = (self.head_look * 0.10
            + face.look.x
            + (time * 0.73 + seed_unit(self.seed) * 6.0).sin() * 0.025 * look_idle)
            .clamp(-0.16, 0.16);
        face.look.y = (face.look.y
            + (time * 0.51 + seed_unit(self.seed) * 3.0).cos() * 0.012 * look_idle)
            .clamp(-0.16, 0.16);
        face = face.clamped();

        let output = AnimationOutput {
            pose,
            face,
            secondary,
        };
        self.last_sequence = Some(sample.sequence);
        self.last_time = time;
        self.last_position = sample.position;
        self.output = Some(output);
        output
    }
}

fn rotate(pose: &mut Pose, joint: JointId, x: f32, y: f32, z: f32) {
    let turn = Quat::from_euler(EulerRot::XYZ, x, y, z);
    pose.transforms[joint.index()].rotation = turn * pose.transforms[joint.index()].rotation;
}

fn blend_face(current: FaceParameters, target: FaceParameters, delta: f32) -> FaceParameters {
    let amount = (delta * 14.0).clamp(0.0, 1.0);
    let mix = |a: f32, b: f32| a + (b - a) * amount;
    FaceParameters {
        eye_opening: mix(current.eye_opening, target.eye_opening),
        look: Vec2::new(
            mix(current.look.x, target.look.x),
            mix(current.look.y, target.look.y),
        ),
        brow_tilt: mix(current.brow_tilt, target.brow_tilt),
        mouth_curve: mix(current.mouth_curve, target.mouth_curve),
        mouth_opening: mix(current.mouth_opening, target.mouth_opening),
    }
}

fn default_expression(body: BodyId) -> FacePreset {
    match body {
        BodyId::Person => FacePreset::Happy,
        BodyId::Cat => FacePreset::Curious,
        BodyId::Dragon => FacePreset::Determined,
    }
}

fn shortest_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn presentation_seed(key: CharacterEntityKey) -> u32 {
    let kind = match key.kind {
        crate::types::CharacterEntityKind::LocalPlayer => 1,
        crate::types::CharacterEntityKind::LocalNpc => 2,
        crate::types::CharacterEntityKind::RemotePlayer => 3,
    };
    let mut value = key.slot as u32 ^ key.generation.rotate_left(11) ^ kind * 0x9e37_79b9;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    value.max(1)
}

fn seed_unit(seed: u32) -> f32 {
    seed as f32 / u32::MAX as f32
}

fn next_blink(seed: u32, count: u32) -> f32 {
    let mut value = seed.wrapping_add(count.wrapping_mul(0x6d2b_79f5));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    2.2 + value as f32 / u32::MAX as f32 * 2.8
}

trait Smoothstep {
    fn smoothstep(self, edge0: f32, edge1: f32) -> f32;
}

trait FiniteOrZero {
    fn finite_or_zero(self) -> f32;
}

impl FiniteOrZero for f32 {
    fn finite_or_zero(self) -> f32 {
        if self.is_finite() {
            self
        } else {
            0.0
        }
    }
}

impl Smoothstep for f32 {
    fn smoothstep(self, edge0: f32, edge1: f32) -> f32 {
        let t = ((self - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CharacterEntityKind, CharacterMotionSource};

    fn sample(sequence: u64, time: f32) -> CharacterMotionSample {
        CharacterMotionSample {
            key: CharacterEntityKey {
                kind: CharacterEntityKind::LocalPlayer,
                slot: 0,
                generation: 1,
            },
            sequence,
            time,
            position: [0.0, 0.0, -time],
            facing_yaw: std::f32::consts::PI - 0.01,
            look_yaw: -std::f32::consts::PI + 0.01,
            planar_velocity: Some([0.0, 6.4]),
            vertical_velocity: Some(0.0),
            support: CharacterSupport::Grounded { height: 0.0 },
            stride_phase: time * 4.0,
            moving: true,
            sprinting: false,
            source: CharacterMotionSource::Simulation,
            event: CharacterMotionEvent::None,
            emote: CharacterEmote::None,
        }
    }

    #[test]
    fn repeated_sequence_does_not_advance_presentation() {
        let mut state = CharacterPresentationState::new(sample(1, 0.0).key, BodyId::Person);
        let first = state.evaluate(sample(1, 0.0), BodyId::Person, false);
        let repeated = state.evaluate(sample(1, 1.0), BodyId::Person, false);
        assert_eq!(first, repeated);
    }

    #[test]
    fn yaw_wrap_uses_shortest_turn() {
        let mut state = CharacterPresentationState::new(sample(1, 0.0).key, BodyId::Person);
        let output = state.evaluate(sample(1, 0.0), BodyId::Person, false);
        assert!(output.pose.transforms[JointId::Head.index()]
            .rotation
            .is_normalized());
    }

    #[test]
    fn all_expression_presets_are_authored() {
        assert!(FacePreset::ALL.len() >= 20);
        assert!(FacePreset::ALL
            .iter()
            .all(|preset| !preset.stable_id().is_empty()));
    }
}
