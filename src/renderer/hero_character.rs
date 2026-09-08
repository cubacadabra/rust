//! Casual person study. Shapes serve hair, clothing and anatomy; they are not
//! required to expose the joints or resemble manufactured toy components.
use super::character::{Anchor, Feature, Part, Tint};
use super::hero_geometry::Shape;
#[cfg(test)]
use crate::character::foot_is_planted;
use crate::character::{BodyPart, JointId, Pose, STANCE_ANKLE_HEIGHT, gait};
use glam::{Mat4, Quat, Vec3};

/// Review alternatives share the actual production meshes and lighting.
/// These are art studies, not new persistent appearance IDs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Study {
    #[default]
    Everyday,
    #[allow(dead_code)]
    LongerLegs,
    #[allow(dead_code)]
    SoftShoulders,
}

pub(super) const SLEEVE_SIZE: Vec3 = Vec3::new(0.46, 1.13, 0.49);
pub(super) const SLEEVE_CENTER: f32 = -0.40;
pub(super) const ELBOW: f32 = -0.52;

const SOLE_HEIGHT: f32 = 0.095;

fn sole_center_y() -> f32 {
    // The sole is attached to an ankle at STANCE_ANKLE_HEIGHT. Its local
    // center is therefore the offset that puts the generated minimum on the
    // support plane, including the normalized mesh's actual lower bound.
    -STANCE_ANKLE_HEIGHT - super::hero_geometry::SHOE_MIN_NORMALIZED_Y * SOLE_HEIGHT
}

/// Refit the shared hierarchy without changing collision, camera height, or
/// stored appearance. Keep animated offsets relative to the shared rest rig.
pub(super) fn fit_pose(
    entity: super::RenderEntity,
    study: Study,
    rest: &crate::character::rig::RigDefinition,
) -> Pose {
    use JointId::*;
    let mut pose = entity.pose;
    let (leg, hip, torso, head) = match study {
        Study::Everyday => (0.62, 1.235, 1.91, 1.04),
        Study::LongerLegs => (0.66, 1.315, 1.99, 1.02),
        Study::SoftShoulders => (0.58, 1.155, 1.83, 1.06),
    };
    let mut place = |joint: JointId, at: Vec3| {
        pose.transforms[joint.index()].translation +=
            at - rest.joints[joint.index()].rest.translation;
    };
    place(Torso, Vec3::new(0.0, torso, 0.0));
    place(Head, Vec3::new(0.0, head, 0.0));
    for (side, arm, elbow, hand, thigh, knee, foot) in [
        (
            -1.0,
            LeftUpperArm,
            LeftLowerArm,
            LeftHand,
            LeftUpperLeg,
            LeftLowerLeg,
            LeftFoot,
        ),
        (
            1.0,
            RightUpperArm,
            RightLowerArm,
            RightHand,
            RightUpperLeg,
            RightLowerLeg,
            RightFoot,
        ),
    ] {
        let (shoulder, drop) = match study {
            Study::Everyday => (0.49, 0.28),
            Study::LongerLegs => (0.49, 0.32),
            Study::SoftShoulders => (0.51, 0.27),
        };
        place(arm, Vec3::new(side * shoulder, drop, 0.0));
        place(elbow, Vec3::new(0.0, ELBOW, 0.0));
        place(hand, Vec3::new(0.0, -0.44, -0.01));
        place(thigh, Vec3::new(side * 0.28, hip, 0.0));
        place(knee, Vec3::new(0.0, -leg, 0.0));
        place(foot, Vec3::new(0.0, -leg, 0.0));
    }
    if let crate::types::CharacterSupport::Grounded { height } = entity.support {
        let blend = entity.secondary.stride_blend.clamp(0.0, 1.0);
        let run = entity.secondary.run_blend.clamp(0.0, 1.0);
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(entity.yaw),
            Vec3::from_array(entity.position),
        );
        let inverse_root = root.inverse();
        let support_y = height - entity.position[1];
        // The pelvis follows the chest vertically. Compressing the knees also
        // lowers the hoodie, rather than opening a gap at the waist.
        pose.transforms[Torso.index()].translation.y -= blend * (0.17 + run * 0.055);
        let pelvis_drop = pose.transforms[Torso.index()].translation.y - torso;
        let pelvis_sway = pose.transforms[Torso.index()].translation.x;
        for (offset, side, thigh, knee, foot, ankle_target, contact_target) in [
            (
                0.0,
                -1.0,
                LeftUpperLeg,
                LeftLowerLeg,
                LeftFoot,
                entity.secondary.left_ankle_target,
                entity.secondary.left_foot_target,
            ),
            (
                std::f32::consts::PI,
                1.0,
                RightUpperLeg,
                RightLowerLeg,
                RightFoot,
                entity.secondary.right_ankle_target,
                entity.secondary.right_foot_target,
            ),
        ] {
            let step = gait::footfall(entity.walk_cycle + offset, blend, run);
            let target_local = ankle_target.or(contact_target).and_then(|world| {
                let local = inverse_root.transform_point3(world);
                local.is_finite().then_some(local)
            });
            let hip_position = Vec3::new(
                side * gait::FOOT_SPACING + pelvis_sway,
                hip + pelvis_drop,
                0.0,
            );
            pose.transforms[thigh.index()].translation = hip_position;
            let mut ankle = target_local.unwrap_or(Vec3::new(
                side * gait::FOOT_SPACING,
                support_y + STANCE_ANKLE_HEIGHT + step.lift + step.pitch.sin().abs() * 0.38,
                step.travel,
            ));
            // Keep both chains anatomical during tight turns and world
            // corrections. Limit reach without changing either bone length.
            ankle.x = side * (side * ankle.x).max(0.13);
            ankle.y = ankle
                .y
                .max(support_y + STANCE_ANKLE_HEIGHT)
                .min(hip_position.y - 0.12);
            let down = hip_position.y - ankle.y;
            let reach = leg * 2.0 - 0.002;
            let max_planar = (reach * reach - down * down).max(0.0).sqrt();
            let planar =
                glam::Vec2::new(ankle.x - hip_position.x, ankle.z).clamp_length_max(max_planar);
            ankle.x = hip_position.x + planar.x;
            ankle.z = planar.y;
            let to_ankle = ankle - hip_position;
            let distance = to_ankle.length().clamp(0.001, reach);
            let axis = to_ankle.normalize_or_zero();
            let bend = (Vec3::NEG_Z - axis * axis.dot(Vec3::NEG_Z)).normalize_or_zero();
            let knee_position = hip_position
                + axis * distance * 0.5
                + bend * (leg * leg - distance * distance * 0.25).max(0.0).sqrt();
            let upper = Quat::from_rotation_arc(
                Vec3::NEG_Y,
                (knee_position - hip_position).normalize_or_zero(),
            );
            let lower =
                Quat::from_rotation_arc(Vec3::NEG_Y, (ankle - knee_position).normalize_or_zero());
            pose.transforms[thigh.index()].rotation = upper;
            pose.transforms[knee.index()].rotation = upper.conjugate() * lower;
            let clearance = ((ankle.y - support_y - STANCE_ANKLE_HEIGHT) / 0.12).clamp(0.0, 1.0);
            pose.transforms[foot.index()].rotation =
                lower.conjugate() * Quat::from_rotation_x(step.pitch * clearance);
        }
    }
    pose
}

pub(super) fn study_part(mut part: Part, study: Study) -> Part {
    if part.anchor.joint == JointId::Torso && study == Study::SoftShoulders {
        part.anchor.local = Mat4::from_scale(Vec3::new(1.09, 1.0, 1.03)) * part.anchor.local;
    }
    if part.anchor.joint == JointId::Head {
        let scale = match study {
            Study::Everyday => Vec3::ONE,
            Study::LongerLegs => Vec3::new(0.90, 0.92, 0.94),
            Study::SoftShoulders => Vec3::new(1.07, 1.03, 1.02),
        };
        part.anchor.local = Mat4::from_scale(scale) * part.anchor.local;
        if matches!(part.feature, Feature::Eye(_)) && study == Study::SoftShoulders {
            part.anchor.local *= Mat4::from_scale(Vec3::new(1.18, 1.12, 1.0));
        }
    }
    part
}

fn piece(
    parts: &mut Vec<Part>,
    joint: JointId,
    position: Vec3,
    size: Vec3,
    shape: Shape,
    tint: Tint,
    turn: f32,
) {
    parts.push(Part {
        anchor: Anchor {
            joint,
            local: Mat4::from_translation(position) * Mat4::from_rotation_z(turn),
        },
        spec: BodyPart::new(size, 0.0),
        tint,
        shape,
        feature: if joint == JointId::Torso && position.z >= 0.3 {
            Feature::Cloth
        } else {
            Feature::None
        },
    });
}

/// Add one chunky lock with an explicit buried root and free tip. Hair is
/// authored in head-local coordinates so its attachment remains legible when
/// the cap or face is reviewed from another camera.
fn hair_lock(parts: &mut Vec<Part>, root: Vec3, tip: Vec3, width: f32, depth: f32) {
    let direction = tip - root;
    let length = direction.length();
    if !direction.is_finite() || !length.is_finite() || length <= 0.0001 {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, direction / length);
    parts.push(Part {
        anchor: Anchor {
            joint: JointId::Head,
            local: Mat4::from_translation((root + tip) * 0.5) * Mat4::from_quat(rotation),
        },
        spec: BodyPart::new(Vec3::new(width, length, depth), 0.0),
        tint: Tint::Hair,
        feature: Feature::None,
        shape: Shape::HairLock,
    });
}

fn hair_cap(parts: &mut Vec<Part>, position: Vec3, size: Vec3) {
    piece(
        parts,
        JointId::Head,
        position,
        size,
        Shape::HairCap,
        Tint::Hair,
        0.0,
    );
}

fn boy_hair(parts: &mut Vec<Part>) {
    hair_cap(
        parts,
        Vec3::new(0.0, 0.18, 0.065),
        Vec3::new(1.10, 0.83, 0.96),
    );
    for (root, tip, width, depth) in [
        (
            Vec3::new(-0.23, 0.46, -0.26),
            Vec3::new(0.28, 0.22, -0.47),
            0.36,
            0.29,
        ),
        (
            Vec3::new(-0.19, 0.43, -0.25),
            Vec3::new(-0.41, 0.12, -0.43),
            0.27,
            0.24,
        ),
        (
            Vec3::new(0.09, 0.48, -0.21),
            Vec3::new(0.44, 0.11, -0.36),
            0.28,
            0.26,
        ),
        (
            Vec3::new(-0.38, 0.37, 0.02),
            Vec3::new(-0.51, -0.045, 0.18),
            0.24,
            0.28,
        ),
        (
            Vec3::new(0.38, 0.36, 0.05),
            Vec3::new(0.52, -0.015, 0.21),
            0.24,
            0.27,
        ),
        (
            Vec3::new(-0.13, 0.49, 0.03),
            Vec3::new(0.28, 0.40, -0.25),
            0.32,
            0.27,
        ),
        (
            Vec3::new(-0.27, 0.40, 0.29),
            Vec3::new(-0.34, -0.10, 0.44),
            0.26,
            0.24,
        ),
        (
            Vec3::new(0.0, 0.43, 0.35),
            Vec3::new(0.09, -0.15, 0.49),
            0.32,
            0.25,
        ),
        (
            Vec3::new(0.26, 0.38, 0.29),
            Vec3::new(0.38, -0.08, 0.42),
            0.25,
            0.24,
        ),
    ] {
        hair_lock(parts, root, tip, width, depth);
    }
}

fn add_hair(parts: &mut Vec<Part>, body: crate::character::BodyId) {
    if !super::hair_geometry::add_parts(parts, body) && body == crate::character::BodyId::Person {
        boy_hair(parts);
    }
}

pub(super) fn replace_hair(parts: &mut Vec<Part>, body: crate::character::BodyId) {
    parts.retain(|part| !matches!(part.tint, Tint::Hair));
    add_hair(parts, body);
}

pub(super) fn finish(parts: &mut Vec<Part>, body: crate::character::BodyId) {
    use JointId::*;
    // Replace the old decorative boxes with garment surfaces. Hidden shoulder
    // cores give their part slots to visible cuffs and thumbs.
    parts.retain(|p| {
        !matches!(p.tint, Tint::Hair)
            && !(p.anchor.joint == Torso
                && matches!(p.feature, Feature::None)
                && p.anchor.local.w_axis.truncate() != Vec3::ZERO)
            && !matches!(p.feature, Feature::Seam(_) | Feature::Cheek(_))
            && !(matches!(p.anchor.joint, LeftLowerArm | RightLowerArm)
                && matches!(p.tint, Tint::Shirt))
    });
    for p in parts.iter_mut() {
        let joint = p.anchor.joint;
        match (joint, p.tint, p.feature) {
            (Torso, Tint::Shirt, _) => {
                p.shape = Shape::Torso;
                p.spec = BodyPart::new(Vec3::new(1.10, 1.04, 0.73), 0.0);
            }
            (Head, Tint::Skin, Feature::None) if p.spec.size.x > 0.5 => {
                p.shape = Shape::Head;
                p.spec = BodyPart::new(Vec3::new(1.02, 0.94, 0.85), 0.0);
            }
            (Head, Tint::Skin, Feature::None) => {
                let side = p.anchor.local.w_axis.x.signum();
                p.shape = Shape::Pebble;
                p.spec = BodyPart::new(Vec3::new(0.15, 0.23, 0.20), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.49, -0.01, 0.0));
            }
            (LeftUpperArm | RightUpperArm, Tint::Shirt, _) => {
                p.shape = Shape::Sleeve;
                p.spec = BodyPart::new(SLEEVE_SIZE, 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, SLEEVE_CENTER, 0.0));
            }
            (LeftHand | RightHand, Tint::Skin, _) => {
                p.shape = Shape::Pebble;
                p.spec = BodyPart::new(Vec3::new(0.26, 0.32, 0.23), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.055, -0.025));
            }
            (LeftUpperLeg | RightUpperLeg, Tint::Pants, _) => {
                p.shape = Shape::Shorts;
                p.spec = BodyPart::new(Vec3::new(0.46, 0.66, 0.47), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.15, 0.0));
            }
            (LeftLowerLeg | RightLowerLeg, Tint::Pants, _) => {
                p.shape = Shape::Limb;
                p.tint = Tint::Skin;
                p.spec = BodyPart::new(Vec3::new(0.28, 0.76, 0.30), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0));
            }
            (LeftFoot | RightFoot, Tint::Shoes, _) => {
                p.shape = Shape::Shoe;
                p.spec = BodyPart::new(Vec3::new(0.50, 0.29, 0.76), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.155, -0.09));
            }
            (LeftFoot | RightFoot, Tint::Ivory, Feature::Sole) => {
                p.shape = Shape::Shoe;
                p.spec = BodyPart::new(Vec3::new(0.51, SOLE_HEIGHT, 0.77), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, sole_center_y(), -0.09));
            }
            (LeftFoot | RightFoot, Tint::Ivory, _) => {
                p.shape = Shape::Laces;
                p.spec = BodyPart::new(Vec3::new(0.31, 0.065, 0.24), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.26, -0.24));
            }
            (_, _, Feature::Eye(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.115, 0.170, 0.025), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.245, 0.015, -0.408))
                    * Mat4::from_rotation_y(-side * 0.42);
            }
            (_, _, Feature::Brow(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.17, 0.035, 0.025), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.205, 0.20, -0.414));
            }
            (_, _, Feature::Mouth) => {
                p.spec = BodyPart::new(Vec3::new(0.34, 0.16, 0.02), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.19, -0.405));
            }
            (_, _, Feature::MouthEdge(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.13, 0.090, 0.020), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.15, -0.19, -0.395))
                    * Mat4::from_rotation_y(-side * 0.72);
            }
            (_, _, Feature::Cheek(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.11, 0.045, 0.02), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.36, -0.13, -0.443));
            }
            _ => {}
        }
    }
    let mut add =
        |joint, pos, size, shape, tint, turn| piece(parts, joint, pos, size, shape, tint, turn);
    add(
        Torso,
        Vec3::new(0.0, 0.45, 0.12),
        Vec3::new(0.73, 0.21, 0.72),
        Shape::Hood,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, 0.30, 0.30),
        Vec3::new(0.69, 0.60, 0.38),
        Shape::Pebble,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, -0.51, 0.0),
        Vec3::new(0.89, 0.12, 0.64),
        Shape::Rib,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, -0.21, -0.33),
        Vec3::new(0.66, 0.32, 0.11),
        Shape::Pocket,
        Tint::Shirt,
        0.0,
    );
    for (side, forearm, hand) in [
        (-1.0, LeftLowerArm, LeftHand),
        (1.0, RightLowerArm, RightHand),
    ] {
        add(
            forearm,
            Vec3::new(0.0, -0.40, 0.0),
            Vec3::new(0.31, 0.11, 0.32),
            Shape::Rib,
            Tint::Shirt,
            0.0,
        );
        add(
            hand,
            Vec3::new(-side * 0.125, 0.025, -0.04),
            Vec3::new(0.13, 0.20, 0.14),
            Shape::Pebble,
            Tint::Skin,
            -side * 0.45,
        );
        add(
            Torso,
            Vec3::new(side * 0.135, 0.20, -0.345),
            Vec3::new(0.027, 0.39, 0.027),
            Shape::Cord,
            Tint::Ivory,
            side * 0.05,
        );
    }
    add(
        Torso,
        Vec3::new(0.0, 0.56, 0.0),
        Vec3::new(0.29, 0.34, 0.30),
        Shape::Limb,
        Tint::Skin,
        0.0,
    );
    add_hair(parts, body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{AnimationOutput, BodyId, CharacterPresentationState, body_recipe};
    use crate::types::{
        CharacterEmote, CharacterEntityKey, CharacterEntityKind, CharacterMotionEvent,
        CharacterMotionSample, CharacterMotionSource, CharacterSupport,
    };

    fn sample(
        key: CharacterEntityKey,
        sequence: u64,
        time: f32,
        support_height: f32,
        event: CharacterMotionEvent,
    ) -> CharacterMotionSample {
        CharacterMotionSample {
            key,
            sequence,
            time,
            position: [0.0, support_height, 0.0],
            facing_yaw: 0.0,
            look_yaw: 0.0,
            planar_velocity: Some([0.0, 0.0]),
            vertical_velocity: Some(0.0),
            support: CharacterSupport::Grounded {
                height: support_height,
            },
            stride_phase: 0.0,
            moving: false,
            sprinting: false,
            source: CharacterMotionSource::Simulation,
            event,
            emote: CharacterEmote::None,
            emote_sequence: 0,
            appearance_revision: 0,
        }
    }

    fn render_entity(
        animation: AnimationOutput,
        support_height: f32,
    ) -> super::super::RenderEntity {
        super::super::RenderEntity {
            body: BodyId::Person,
            outfit: crate::character::OutfitId::EverydayHoodie,
            position: [0.0, support_height, 0.0],
            pose: animation.pose,
            secondary: animation.secondary,
            support: CharacterSupport::Grounded {
                height: support_height,
            },
            ..Default::default()
        }
    }

    fn actual_sole_min_y(
        entity: super::super::RenderEntity,
        study: Study,
        recipe: &crate::character::BodyRecipe,
        part: Part,
        lod: super::super::character_quality::CharacterLod,
    ) -> f32 {
        let pose = fit_pose(entity, study, &recipe.rig);
        let joints = recipe.rig.world_matrices(&pose.transforms);
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(entity.yaw),
            Vec3::from_array(entity.position),
        );
        let transform = root
            * joints[part.anchor.joint.index()]
            * part.anchor.local
            * Mat4::from_scale(part.spec.size);
        assert!(
            transform
                .to_cols_array()
                .iter()
                .all(|value| value.is_finite())
        );
        let mesh = super::super::hero_geometry::build(part.shape, Vec3::ONE, lod.subdivisions());
        mesh.vertices
            .iter()
            .map(|vertex| transform.transform_point3(vertex.position).y)
            .fold(f32::INFINITY, f32::min)
    }

    #[test]
    fn authored_hair_locks_have_bounded_roots_and_finite_transforms() {
        let recipe = body_recipe(BodyId::Person);
        let parts =
            super::super::character::parts_for(&recipe, crate::character::OutfitId::EverydayHoodie);
        let cap = parts
            .iter()
            .find(|part| part.shape == Shape::HairCap)
            .copied()
            .expect("person hair cap");
        let locks: Vec<_> = parts
            .iter()
            .copied()
            .filter(|part| part.shape == Shape::HairLock)
            .collect();
        assert_eq!(locks.len(), 9);

        let cap_center = cap.anchor.local.transform_point3(Vec3::ZERO);
        let cap_half = cap.spec.size * 0.5;
        for lock in locks {
            assert!(lock.spec.size.is_finite());
            assert!(lock.spec.size.y > 0.0001);
            assert!(
                lock.anchor
                    .local
                    .to_cols_array()
                    .iter()
                    .all(|value| value.is_finite())
            );
            let root =
                lock.anchor
                    .local
                    .transform_point3(Vec3::new(0.0, -lock.spec.size.y * 0.5, 0.0));
            let tip =
                lock.anchor
                    .local
                    .transform_point3(Vec3::new(0.0, lock.spec.size.y * 0.5, 0.0));
            assert!(root.is_finite() && tip.is_finite());
            assert!((tip - root).length() > 0.0001);
            let root_offset = (root - cap_center).abs();
            assert!(root_offset.x <= cap_half.x + 0.02);
            assert!(root_offset.y <= cap_half.y + 0.02);
            assert!(root_offset.z <= cap_half.z + 0.02);
            assert!((root - cap_center).length() < (tip - cap_center).length());
            assert!(root.length() < 1.0 && tip.length() < 1.0);

            let transform = lock.anchor.local * Mat4::from_scale(lock.spec.size);
            for lod in super::super::character_quality::CharacterLod::ALL {
                let mesh = super::super::hero_geometry::build(
                    Shape::HairLock,
                    Vec3::ONE,
                    lod.subdivisions(),
                );
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|vertex| { transform.transform_point3(vertex.position).is_finite() })
                );
            }
        }
    }

    #[test]
    fn fitted_stance_joints_are_level_and_separated_through_the_stride() {
        let recipe = body_recipe(BodyId::Person);
        for study in [Study::Everyday, Study::LongerLegs, Study::SoftShoulders] {
            for step in 0..120 {
                let mut entity = super::super::RenderEntity {
                    pose: Pose::rest(&recipe.rig),
                    support: CharacterSupport::Grounded { height: 0.0 },
                    walk_cycle: step as f32 * std::f32::consts::TAU / 120.0,
                    moving: true,
                    ..Default::default()
                };
                entity.secondary.stride_blend = 0.8;
                let pose = fit_pose(entity, study, &recipe.rig);
                let world = recipe.rig.world_matrices(&pose.transforms);
                let left = world[JointId::LeftFoot.index()];
                let right = world[JointId::RightFoot.index()];
                assert!(right.w_axis.x - left.w_axis.x > 0.54);
                for (offset, foot, knee) in [
                    (0.0, JointId::LeftFoot, JointId::LeftLowerLeg),
                    (
                        std::f32::consts::PI,
                        JointId::RightFoot,
                        JointId::RightLowerLeg,
                    ),
                ] {
                    let matrix = world[foot.index()];
                    if foot_is_planted(entity.walk_cycle + offset) {
                        assert!(matrix.transform_vector3(Vec3::Y).distance(Vec3::Y) < 0.0001);
                    }
                    let height = matrix.w_axis.y;
                    assert!(height >= 0.0499);
                    if foot_is_planted(entity.walk_cycle + offset) {
                        assert!(
                            (height - STANCE_ANKLE_HEIGHT).abs() < 0.0001,
                            "stance ankle must stay at its fitted target"
                        );
                    }
                    assert!(pose.transforms[knee.index()].rotation.to_scaled_axis().x <= 0.0);
                }
            }
        }
    }

    #[test]
    fn generated_soles_touch_support_through_all_person_studies_lods_and_strides() {
        let recipe = body_recipe(BodyId::Person);
        let parts =
            super::super::character::parts_for(&recipe, crate::character::OutfitId::EverydayHoodie);
        let soles: Vec<_> = parts
            .iter()
            .copied()
            .filter(|part| {
                matches!(part.feature, Feature::Sole)
                    && matches!(part.anchor.joint, JointId::LeftFoot | JointId::RightFoot)
            })
            .collect();
        assert_eq!(soles.len(), 2);

        for study in [Study::Everyday, Study::LongerLegs, Study::SoftShoulders] {
            for lod in super::super::character_quality::CharacterLod::ALL {
                for (moving, sprinting, stride_blend) in [
                    (false, false, 0.0),
                    (true, false, 6.4 / 11.5),
                    (true, true, 1.0),
                ] {
                    for support_height in [0.0, 2.0] {
                        for step in 0..120 {
                            let phase = step as f32 * std::f32::consts::TAU / 120.0;
                            let mut entity = super::super::RenderEntity {
                                body: BodyId::Person,
                                outfit: crate::character::OutfitId::EverydayHoodie,
                                pose: Pose::rest(&recipe.rig),
                                position: [0.0, support_height, 0.0],
                                support: CharacterSupport::Grounded {
                                    height: support_height,
                                },
                                walk_cycle: phase,
                                moving,
                                sprinting,
                                ..Default::default()
                            };
                            entity.secondary.stride_blend = stride_blend;
                            for part in &soles {
                                let offset = if part.anchor.joint == JointId::LeftFoot {
                                    0.0
                                } else {
                                    std::f32::consts::PI
                                };
                                let minimum = actual_sole_min_y(entity, study, &recipe, *part, lod);
                                assert!(
                                    minimum >= support_height - 0.001,
                                    "sole penetrates support: study={study:?} lod={lod:?} moving={moving} sprinting={sprinting} height={support_height} phase={phase} minimum={minimum}"
                                );
                                if stride_blend == 0.0 || foot_is_planted(phase + offset) {
                                    assert!(
                                        (minimum - support_height).abs() < 0.001,
                                        "stance sole misses support: study={study:?} lod={lod:?} moving={moving} sprinting={sprinting} height={support_height} phase={phase} minimum={minimum}"
                                    );
                                }
                            }
                        }

                        if stride_blend > 0.0 {
                            for part in &soles {
                                let offset = if part.anchor.joint == JointId::LeftFoot {
                                    0.0
                                } else {
                                    std::f32::consts::PI
                                };
                                let mut entity = super::super::RenderEntity {
                                    body: BodyId::Person,
                                    outfit: crate::character::OutfitId::EverydayHoodie,
                                    pose: Pose::rest(&recipe.rig),
                                    position: [0.0, support_height, 0.0],
                                    support: CharacterSupport::Grounded {
                                        height: support_height,
                                    },
                                    walk_cycle: 1.5 * std::f32::consts::PI - offset,
                                    moving: true,
                                    sprinting: stride_blend == 1.0,
                                    ..Default::default()
                                };
                                entity.secondary.stride_blend = stride_blend;
                                let minimum = actual_sole_min_y(entity, study, &recipe, *part, lod);
                                assert!(
                                    minimum > support_height + 0.001,
                                    "mid-swing sole must clear support: study={study:?} lod={lod:?} height={support_height} minimum={minimum}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn landing_presentation_compresses_grounded_hero_and_keeps_soles_planted() {
        let recipe = body_recipe(BodyId::Person);
        let parts =
            super::super::character::parts_for(&recipe, crate::character::OutfitId::EverydayHoodie);
        let soles: Vec<_> = parts
            .iter()
            .copied()
            .filter(|part| {
                matches!(part.feature, Feature::Sole)
                    && matches!(part.anchor.joint, JointId::LeftFoot | JointId::RightFoot)
            })
            .collect();
        assert_eq!(soles.len(), 2);

        for support_height in [0.0, 2.0] {
            let key = CharacterEntityKey {
                kind: CharacterEntityKind::LocalPlayer,
                slot: 0,
                generation: support_height as u32 + 1,
                identity: 17,
            };
            let mut presentation = CharacterPresentationState::new(key, BodyId::Person);
            presentation.evaluate(
                sample(key, 0, 0.0, support_height, CharacterMotionEvent::None),
                BodyId::Person,
                false,
            );
            let peak = presentation.evaluate(
                sample(
                    key,
                    1,
                    1.0 / 60.0,
                    support_height,
                    CharacterMotionEvent::Landing,
                ),
                BodyId::Person,
                false,
            );
            assert!(peak.secondary.landing_compression > 0.0);
            assert!(peak.secondary.landing_compression <= 1.0);

            let recovery = presentation.evaluate(
                sample(
                    key,
                    7,
                    7.0 / 60.0,
                    support_height,
                    CharacterMotionEvent::None,
                ),
                BodyId::Person,
                false,
            );
            assert!(recovery.secondary.landing_compression > 0.0);
            assert!(recovery.secondary.landing_compression < peak.secondary.landing_compression);

            let mut settled = recovery;
            for sequence in 8..=30 {
                settled = presentation.evaluate(
                    sample(
                        key,
                        sequence,
                        sequence as f32 / 60.0,
                        support_height,
                        CharacterMotionEvent::None,
                    ),
                    BodyId::Person,
                    false,
                );
            }
            assert_eq!(settled.secondary.landing_compression, 0.0);

            let peak_entity = render_entity(peak, support_height);
            let recovery_entity = render_entity(recovery, support_height);
            let settled_entity = render_entity(settled, support_height);
            let peak_pose = fit_pose(peak_entity, Study::Everyday, &recipe.rig);
            let recovery_pose = fit_pose(recovery_entity, Study::Everyday, &recipe.rig);
            let settled_pose = fit_pose(settled_entity, Study::Everyday, &recipe.rig);
            let peak_world = recipe.rig.world_matrices(&peak_pose.transforms);
            let recovery_world = recipe.rig.world_matrices(&recovery_pose.transforms);
            let settled_world = recipe.rig.world_matrices(&settled_pose.transforms);
            let peak_hip = peak_world[JointId::LeftUpperLeg.index()].w_axis.y;
            let recovery_hip = recovery_world[JointId::LeftUpperLeg.index()].w_axis.y;
            let settled_hip = settled_world[JointId::LeftUpperLeg.index()].w_axis.y;
            assert!((settled_hip - peak_hip - 0.075).abs() < 0.0001);
            assert!(peak_hip < recovery_hip && recovery_hip < settled_hip);
            assert!(
                peak_pose.transforms[JointId::LeftLowerLeg.index()]
                    .rotation
                    .to_scaled_axis()
                    .x
                    < settled_pose.transforms[JointId::LeftLowerLeg.index()]
                        .rotation
                        .to_scaled_axis()
                        .x
            );

            for entity in [peak_entity, recovery_entity, settled_entity] {
                for lod in super::super::character_quality::CharacterLod::ALL {
                    for part in &soles {
                        let minimum =
                            actual_sole_min_y(entity, Study::Everyday, &recipe, *part, lod);
                        assert!(
                            (minimum - support_height).abs() < 0.001,
                            "landing sole misses support: lod={lod:?} height={support_height} minimum={minimum}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn person_has_continuous_sleeves_and_event_only_magic() {
        let parts = super::super::character::parts_for(
            &body_recipe(BodyId::Person),
            crate::character::OutfitId::EverydayHoodie,
        );
        assert_eq!(parts.iter().filter(|p| p.shape == Shape::Sleeve).count(), 2);
        let mouth = parts
            .iter()
            .find(|part| matches!(part.feature, Feature::Mouth))
            .expect("person recipe has a mouth feature");
        assert_eq!(mouth.spec.size, Vec3::new(0.34, 0.16, 0.02));
        assert!(!parts.iter().any(|p| matches!(p.feature, Feature::Seam(_))));
        assert!(parts.iter().any(|p| matches!(p.feature, Feature::Spark(_))));
    }
}
