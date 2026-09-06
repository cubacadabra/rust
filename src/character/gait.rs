//! Shared stride timing: contact, recovery and arm swing use the same phase.
use super::STANCE_ANKLE_HEIGHT;
use crate::types::{CharacterMotionSample, CharacterSupport};
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

pub(crate) const WALK_DISTANCE: f32 = 2.65;
pub(crate) const RUN_DISTANCE: f32 = 3.70;
pub(crate) const FOOT_SPACING: f32 = 0.28;

pub(crate) fn run_amount(speed: f32) -> f32 {
    ((speed - crate::engine::WALK_SPEED) / (crate::engine::RUN_SPEED - crate::engine::WALK_SPEED))
        .clamp(0.0, 1.0)
}

pub(crate) fn cycle_distance(run: f32) -> f32 {
    WALK_DISTANCE + (RUN_DISTANCE - WALK_DISTANCE) * run.clamp(0.0, 1.0)
}

pub(crate) fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy)]
pub(crate) struct Footfall {
    pub(crate) planted: bool,
    pub(crate) recovery: f32,
    /// Local +Z is behind the character; contact travels front to back.
    pub(crate) travel: f32,
    pub(crate) lift: f32,
    pub(crate) pitch: f32,
}

pub(crate) fn footfall(phase: f32, amount: f32, run: f32) -> Footfall {
    let run = run.clamp(0.0, 1.0);
    let amount = amount.clamp(0.0, 1.0);
    let cycle = phase.rem_euclid(TAU) / TAU;
    // Walking transfers weight directly between feet. Running has a short
    // flight between contacts and a longer stride, rather than faster shuffling.
    let stance = 0.50 - 0.16 * run;
    let reach = cycle_distance(run) * stance * 0.5 * amount;
    if cycle < stance {
        return Footfall {
            planted: true,
            recovery: 0.0,
            travel: reach * (2.0 * cycle / stance - 1.0) - 0.04,
            lift: 0.0,
            pitch: 0.0,
        };
    }
    let t = (cycle - stance) / (1.0 - stance);
    let arc = (t * PI).sin().max(0.0);
    // The recovery starts with the same backward velocity as contact, then
    // passes under the hips and reaches ahead for the next footfall.
    let tangent = cycle_distance(run) * (1.0 - stance) * amount;
    let travel = reach * (1.0 - 2.0 * ease(t)) + tangent * t * (1.0 - t) * (1.0 - 2.0 * t);
    Footfall {
        planted: false,
        recovery: t,
        travel: travel - 0.04,
        lift: arc.powf(1.5) * (0.20 + 0.20 * run) * amount,
        pitch: -arc * (1.0 - 2.0 * t) * (0.65 + 0.65 * run) * amount,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FootPlant {
    pub(crate) contact: Option<Vec3>,
    pub(crate) ankle: Option<Vec3>,
    departure: Option<Vec3>,
}

impl FootPlant {
    pub(crate) fn update(
        &mut self,
        sample: &CharacterMotionSample,
        step: Footfall,
        side: f32,
        travelling: bool,
        delta: f32,
    ) {
        let CharacterSupport::Grounded { height } = sample.support else {
            *self = Self::default();
            return;
        };
        let origin = Vec3::from_array(sample.position);
        if !origin.is_finite() || !sample.facing_yaw.is_finite() || !height.is_finite() {
            *self = Self::default();
            return;
        }
        let facing = Quat::from_rotation_y(sample.facing_yaw);
        let nominal = origin + facing * Vec3::new(side * FOOT_SPACING, 0.0, step.travel);
        let floor = height + STANCE_ANKLE_HEIGHT;
        if !travelling {
            // Finish the last step after stick release instead of freezing a
            // raised foot, snapping the phase, or leaving a permanent split stance.
            let home = origin + facing * Vec3::new(side * FOOT_SPACING, 0.0, -0.04);
            let home = Vec3::new(home.x, floor, home.z);
            let settled = self
                .ankle
                .map_or(home, |ankle| ankle.lerp(home, 1.0 - (-15.0 * delta).exp()));
            self.ankle = Some(settled);
            self.contact = None;
            self.departure = None;
        } else if step.planted {
            let desired = Vec3::new(nominal.x, floor, nominal.z);
            // A sharp reversal can move the old contact beyond the leg's
            // reach. Release that plant rather than stretching the shin.
            if self.contact.is_some_and(|contact| {
                let local = facing.conjugate() * (contact - origin);
                (local.x - side * FOOT_SPACING).hypot(local.z) > 0.76
            }) {
                self.contact = None;
            }
            let contact = *self.contact.get_or_insert(desired);
            self.ankle = Some(contact);
            self.departure = Some(contact);
        } else {
            self.contact = None;
            let horizontal = self.departure.map_or(nominal, |departure| {
                departure.lerp(nominal, ease(step.recovery / 0.80))
            });
            // Clearance includes the pitched toe/heel, not just the ankle.
            self.ankle = Some(Vec3::new(
                horizontal.x,
                floor + step.lift + step.pitch.sin().abs() * 0.38,
                horizontal.z,
            ));
        }
    }
}
