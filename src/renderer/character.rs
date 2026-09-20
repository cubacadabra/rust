//! Immutable authored rigid pieces; routine animation changes transforms.
use super::AvatarStyle;
use super::character_material::Material;
use crate::character::{
    BodyId, BodyPart, BodyRecipe, FaceParameters, FacePreset, JointId, OutfitId, Pose, body_recipe,
};
use glam::{Mat4, Quat, Vec3};

#[path = "character_parts.rs"]
mod character_parts;

#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(super) use character_parts::add_character;
pub(super) use character_parts::{bounds, mesh_recipe_with_subdivisions, parts_for};

const SEAM_COLOR: [f32; 4] = [0.28, 0.95, 0.87, 0.92];
#[derive(Clone, Copy)]
pub(super) struct Anchor {
    pub joint: JointId,
    pub local: Mat4,
}
impl Anchor {
    fn new(joint: JointId) -> Self {
        Self {
            joint,
            local: Mat4::IDENTITY,
        }
    }
}
impl std::ops::Mul<Mat4> for Anchor {
    type Output = Self;
    fn mul(self, rhs: Mat4) -> Self {
        Self {
            local: self.local * rhs,
            ..self
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tint {
    Skin,
    Shirt,
    Pants,
    Shoes,
    Face,
    Detail,
    Outer,
    Armor,
    Fuzz,
    Seam,
    Ivory,
    Hair,
    Muzzle,
    InnerEar,
    Blush,
}
impl Tint {
    pub fn color(self, style: AvatarStyle, face: [f32; 4]) -> [f32; 4] {
        match self {
            Self::Skin => style.skin,
            Self::Ivory => [0.96, 0.93, 0.84, 1.0],
            Self::Hair => style.body.hair_color(),
            Self::Muzzle => [
                style.skin[0] * 0.45 + 0.52,
                style.skin[1] * 0.45 + 0.49,
                style.skin[2] * 0.45 + 0.44,
                1.0,
            ],
            Self::InnerEar => [0.79, 0.43, 0.39, 1.0],
            Self::Blush => [
                (style.skin[0] * 0.35 + 0.58).min(1.0),
                (style.skin[1] * 0.30 + 0.23).min(1.0),
                (style.skin[2] * 0.28 + 0.25).min(1.0),
                1.0,
            ],
            Self::Shirt => style.shirt,
            Self::Pants => style.pants,
            Self::Shoes => style.shoes,
            Self::Face => face,
            Self::Detail => [
                style.shirt[0] * 0.55,
                style.shirt[1] * 0.55,
                style.shirt[2] * 0.55,
                1.0,
            ],
            Self::Outer => [
                style.shirt[0] * 0.82,
                style.shirt[1] * 0.82,
                style.shirt[2] * 0.82,
                1.0,
            ],
            Self::Armor => [
                (style.pants[0] * 1.12).min(1.0),
                (style.pants[1] * 1.12).min(1.0),
                (style.pants[2] * 1.12).min(1.0),
                1.0,
            ],
            Self::Fuzz => [
                (style.shirt[0] * 1.08).min(1.0),
                (style.shirt[1] * 1.08).min(1.0),
                (style.shirt[2] * 1.08).min(1.0),
                1.0,
            ],
            Self::Seam => SEAM_COLOR,
        }
    }
    pub fn material(self) -> Material {
        match self {
            Self::Skin | Self::Detail | Self::Hair | Self::Muzzle | Self::InnerEar => Material::Toy,
            Self::Ivory => Material::Rubber,
            Self::Shirt => Material::Cloth,
            Self::Pants => Material::Denim,
            Self::Shoes => Material::Rubber,
            Self::Face | Self::Blush => Material::Face,
            Self::Outer => Material::Coat,
            Self::Armor => Material::SoftMetal,
            Self::Fuzz => Material::Fuzz,
            Self::Seam => Material::Seam,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct Part {
    pub anchor: Anchor,
    pub spec: BodyPart,
    pub tint: Tint,
    pub feature: Feature,
    pub shape: super::hero_geometry::Shape,
}
#[derive(Clone, Copy)]
pub(super) enum Feature {
    None,
    /// Skin normally supplied by bundled sleeves; used beneath authored tops
    /// when no authored base is supplying the character's arms.
    GarmentUnderlayer,
    Sole,
    Eye(f32),
    Brow(f32),
    Mouth,
    /// A short continuation from one end of the main smile onto the cheek.
    MouthEdge(f32),
    Cheek(f32),
    Ear(f32),
    Tail(f32),
    Wing(f32),
    Seam(f32),
    Spark(f32),
    Cloth,
}
pub(super) fn world_label_height(body: BodyId) -> f32 {
    static HEIGHTS: std::sync::OnceLock<[f32; 6]> = std::sync::OnceLock::new();
    HEIGHTS.get_or_init(|| {
        BodyId::ALL.map(|id| {
            let recipe = body_recipe(id);
            if let Some(hair_top) = super::hair_geometry::top(id) {
                let pose = super::hero_character::fit_pose(
                    super::RenderEntity {
                        body: id,
                        pose: Pose::rest(&recipe.rig),
                        ..Default::default()
                    },
                    super::hero_character::Study::Everyday,
                    &recipe.rig,
                );
                let head = recipe.rig.world_matrices(&pose.transforms)[JointId::Head.index()];
                return head.transform_point3(Vec3::Y * hair_top).y + 0.10;
            }
            let head_center = recipe.rig.joints[JointId::Torso.index()].rest.translation.y
                + recipe.rig.joints[JointId::Head.index()].rest.translation.y;
            let head_top = head_center + recipe.head.size.y * 0.5;
            let feature_top = match id {
                BodyId::Person => head_center + 0.38 + 0.26 * 0.5,
                BodyId::PersonGirl => head_center + 0.55 + 0.28 * 0.5,
                BodyId::PersonNonbinary => head_center + 0.45 + 0.26 * 0.5,
                BodyId::Cat | BodyId::Wolf => {
                    let ear = recipe.extras.ear_size.unwrap_or(Vec3::ZERO);
                    head_center + 0.46 + ear.y * 0.5 * 0.22_f32.cos() + ear.x * 0.5 * 0.22_f32.sin()
                }
                BodyId::Dragon => {
                    head_center + 0.50 + 0.42 * 0.5 * 0.22_f32.cos() + 0.18 * 0.5 * 0.22_f32.sin()
                }
            };
            head_top.max(feature_top) + 0.10
        })
    })[BodyId::ALL.iter().position(|id| *id == body).unwrap_or(0)]
}

pub(super) fn garment_underlayer(body: BodyId) -> Vec<Part> {
    if !body.is_person() {
        return Vec::new();
    }
    [
        (JointId::LeftUpperArm, -0.27, Vec3::new(0.28, 0.66, 0.32)),
        (JointId::RightUpperArm, -0.27, Vec3::new(0.28, 0.66, 0.32)),
        (JointId::LeftLowerArm, -0.24, Vec3::new(0.25, 0.55, 0.30)),
        (JointId::RightLowerArm, -0.24, Vec3::new(0.25, 0.55, 0.30)),
    ]
    .into_iter()
    .map(|(joint, y, size)| Part {
        anchor: Anchor::new(joint) * Mat4::from_translation(Vec3::new(0.0, y, 0.0)),
        spec: BodyPart::new(size, 0.0),
        tint: Tint::Skin,
        feature: Feature::GarmentUnderlayer,
        shape: super::hero_geometry::Shape::Limb,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::hero_geometry::Shape;

    #[test]
    fn garment_skin_is_bounded_and_bound_to_arm_joints() {
        for body in BodyId::ALL {
            let parts = garment_underlayer(body);
            assert_eq!(parts.len(), if body.is_person() { 4 } else { 0 });
            for part in parts {
                assert!(matches!(part.feature, Feature::GarmentUnderlayer));
                assert!(part.tint == Tint::Skin);
                assert!(matches!(
                    part.anchor.joint,
                    JointId::LeftUpperArm
                        | JointId::RightUpperArm
                        | JointId::LeftLowerArm
                        | JointId::RightLowerArm
                ));
                assert!(part.spec.size.min_element() > 0.0);
                assert!(part.spec.size.max_element() < 0.7);
            }
        }
    }

    #[test]
    fn camera_anchors_are_body_defined() {
        let person = crate::character::definition::camera_anchors(BodyId::Person);
        let cat = crate::character::definition::camera_anchors(BodyId::Cat);
        let dragon = crate::character::definition::camera_anchors(BodyId::Dragon);
        assert_ne!(person.0, dragon.1);
        assert!(cat.0.y > cat.1.y);
    }

    #[test]
    fn compiled_parts_preserve_face_hand_and_foot_anchors() {
        for body in BodyId::ALL {
            let parts = parts_for(&body_recipe(body), OutfitId::EverydayHoodie);
            let rigid_parts = parts
                .iter()
                .filter(|part| !matches!(part.shape, Shape::HairCurve(_, _)))
                .count();
            assert!(rigid_parts <= 48);
            assert_eq!(
                parts
                    .iter()
                    .filter(|p| matches!(
                        p.feature,
                        Feature::Eye(_) | Feature::Brow(_) | Feature::Mouth
                    ))
                    .count(),
                5
            );
            assert_eq!(
                parts
                    .iter()
                    .filter(|p| matches!(p.feature, Feature::MouthEdge(_)))
                    .count(),
                if body.is_person() { 2 } else { 0 }
            );
            assert!(
                parts
                    .iter()
                    .filter(|p| matches!(p.tint, Tint::Face))
                    .all(|p| p.anchor.joint == JointId::Head)
            );
            for joint in [
                JointId::LeftHand,
                JointId::RightHand,
                JointId::LeftFoot,
                JointId::RightFoot,
            ] {
                assert!(
                    parts
                        .iter()
                        .any(|p| p.anchor.joint == joint && !matches!(p.tint, Tint::Seam))
                );
            }
        }
    }

    #[test]
    fn outfits_change_part_count_or_silhouette_recipe() {
        let recipe = body_recipe(BodyId::Dragon);
        let hoodie = parts_for(&recipe, OutfitId::EverydayHoodie);
        let wizard = parts_for(&recipe, OutfitId::StarWizard);
        let knight = parts_for(&recipe, OutfitId::ToyKnight);
        let torso = |parts: &[Part]| {
            parts
                .iter()
                .find(|p| p.anchor.joint == JointId::Torso)
                .unwrap()
                .spec
        };
        assert_ne!(torso(&hoodie), torso(&wizard));
        assert_ne!(torso(&wizard), torso(&knight));
        assert!(wizard.iter().any(|part| matches!(part.tint, Tint::Outer)));
        assert!(knight.iter().any(|part| matches!(part.tint, Tint::Armor)));
    }

    #[test]
    fn complete_catalog_stays_inside_the_rigid_part_budget() {
        for body in BodyId::ALL {
            for outfit in OutfitId::ALL {
                let parts = parts_for(&body_recipe(body), outfit);
                let rigid_parts = parts
                    .iter()
                    .filter(|part| !matches!(part.shape, Shape::HairCurve(_, _)))
                    .count();
                assert!(
                    rigid_parts <= 48,
                    "body={body:?} outfit={outfit:?} rigid_parts={rigid_parts} total_parts={}",
                    parts.len()
                );
            }
        }
    }
}
