//! One coordinated hero fit, authored against docs/art/green-hoodie/concept-v1.png.
use super::character::{Anchor, Feature, Part, Tint};
use super::hero_geometry::Shape;
use crate::character::{BodyPart, JointId};
use glam::{Mat4, Vec3};

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
        feature: if joint == JointId::Torso && position.z > 0.3 {
            Feature::Cloth
        } else {
            Feature::None
        },
    });
}

pub(super) fn finish(parts: &mut Vec<Part>) {
    use JointId::*;
    // Replace the old decorative boxes with garment surfaces. Hidden shoulder
    // cores give their part slots to visible cuffs and thumbs.
    parts.retain(|p| {
        !matches!(p.tint, Tint::Hair)
            && !(p.anchor.joint == Torso
                && matches!(p.feature, Feature::None)
                && p.anchor.local.w_axis.truncate() != Vec3::ZERO)
            && !(matches!(p.anchor.joint, LeftUpperArm | RightUpperArm)
                && matches!(p.feature, Feature::Seam(_)))
    });
    for p in parts.iter_mut() {
        let joint = p.anchor.joint;
        match (joint, p.tint, p.feature) {
            (Torso, Tint::Shirt, _) => {
                p.shape = Shape::Torso;
                p.spec = BodyPart::new(Vec3::new(1.18, 1.10, 0.84), 0.0);
            }
            (Head, Tint::Skin, Feature::None) if p.spec.size.x > 0.5 => {
                p.shape = Shape::Head;
                p.spec = BodyPart::new(Vec3::new(1.17, 1.04, 0.91), 0.0);
            }
            (Head, Tint::Skin, Feature::None) => {
                let side = p.anchor.local.w_axis.x.signum();
                p.shape = Shape::Pebble;
                p.spec = BodyPart::new(Vec3::new(0.19, 0.27, 0.24), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.55, 0.02, 0.0));
            }
            (LeftUpperArm | RightUpperArm, Tint::Shirt, _) => {
                p.shape = Shape::Sleeve;
                p.spec = BodyPart::new(Vec3::new(0.52, 0.78, 0.58), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.02, 0.0));
            }
            (LeftLowerArm | RightLowerArm, Tint::Shirt, _) => {
                p.shape = Shape::Sleeve;
                p.spec = BodyPart::new(Vec3::new(0.48, 0.63, 0.54), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.02, -0.015));
            }
            (LeftHand | RightHand, Tint::Skin, _) => {
                p.shape = Shape::Pebble;
                p.spec = BodyPart::new(Vec3::new(0.31, 0.34, 0.32), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.04, -0.025));
            }
            (LeftUpperLeg | RightUpperLeg, Tint::Pants, _) => {
                p.shape = Shape::Shorts;
                p.spec = BodyPart::new(Vec3::new(0.50, 0.49, 0.52), 0.0);
            }
            (LeftLowerLeg | RightLowerLeg, Tint::Pants, _) => {
                p.shape = Shape::Limb;
                p.tint = Tint::Skin;
                p.spec = BodyPart::new(Vec3::new(0.27, 0.53, 0.31), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.045, 0.0));
            }
            (LeftFoot | RightFoot, Tint::Shoes, _) => {
                p.shape = Shape::Shoe;
                p.spec = BodyPart::new(Vec3::new(0.59, 0.35, 0.83), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, 0.175, 0.0));
            }
            (LeftFoot | RightFoot, Tint::Ivory, _) => {
                let sole = p.anchor.local.w_axis.y < 0.1;
                p.shape = if sole { Shape::Shoe } else { Shape::Laces };
                p.spec = BodyPart::new(
                    if sole {
                        Vec3::new(0.60, 0.095, 0.84)
                    } else {
                        Vec3::new(0.31, 0.065, 0.24)
                    },
                    0.0,
                );
                p.anchor.local = Mat4::from_translation(if sole {
                    Vec3::new(0.0, 0.017, 0.0)
                } else {
                    Vec3::new(0.0, 0.285, -0.10)
                });
            }
            (_, _, Feature::Eye(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.29, 0.34, 0.025), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.225, 0.095, -0.452))
                    * Mat4::from_rotation_y(-side * 0.20);
            }
            (_, _, Feature::Brow(side)) => {
                p.spec = BodyPart::new(Vec3::new(0.29, 0.095, 0.025), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(side * 0.225, 0.325, -0.429));
            }
            (_, _, Feature::Mouth) => {
                p.spec = BodyPart::new(Vec3::new(0.37, 0.16, 0.02), 0.0);
                p.anchor.local = Mat4::from_translation(Vec3::new(0.0, -0.225, -0.424));
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
        Vec3::new(0.0, 0.59, 0.06),
        Vec3::new(1.10, 0.43, 0.91),
        Shape::Hood,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, 0.31, 0.36),
        Vec3::new(0.87, 0.65, 0.49),
        Shape::Pebble,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, -0.51, 0.0),
        Vec3::new(0.98, 0.14, 0.73),
        Shape::Rib,
        Tint::Shirt,
        0.0,
    );
    add(
        Torso,
        Vec3::new(0.0, -0.21, -0.37),
        Vec3::new(0.73, 0.37, 0.18),
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
            Vec3::new(0.0, -0.26, -0.015),
            Vec3::new(0.34, 0.13, 0.37),
            Shape::Rib,
            Tint::Shirt,
            0.0,
        );
        add(
            hand,
            Vec3::new(-side * 0.13, 0.095, -0.09),
            Vec3::new(0.13, 0.22, 0.15),
            Shape::Pebble,
            Tint::Skin,
            -side * 0.45,
        );
        add(
            Torso,
            Vec3::new(side * 0.155, 0.22, -0.381),
            Vec3::new(0.035, 0.40, 0.035),
            Shape::Cord,
            Tint::Ivory,
            side * 0.05,
        );
    }
    add(
        Head,
        Vec3::new(0.0, -0.08, -0.474),
        Vec3::new(0.13, 0.10, 0.115),
        Shape::Pebble,
        Tint::Skin,
        0.0,
    );
    add(
        Head,
        Vec3::new(0.0, 0.34, 0.10),
        Vec3::new(1.22, 0.72, 0.96),
        Shape::HairCap,
        Tint::Hair,
        0.0,
    );
    for (position, size, turn) in [
        (
            Vec3::new(-0.31, 0.39, -0.35),
            Vec3::new(0.40, 0.70, 0.34),
            -0.90,
        ),
        (
            Vec3::new(0.08, 0.49, -0.31),
            Vec3::new(0.38, 0.78, 0.38),
            -1.10,
        ),
        (
            Vec3::new(0.35, 0.48, -0.12),
            Vec3::new(0.34, 0.70, 0.42),
            -1.0,
        ),
    ] {
        add(Head, position, size, Shape::HairLock, Tint::Hair, turn);
    }
}
