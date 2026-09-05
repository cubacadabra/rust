use glam::Vec3;

use super::face::FaceAnchors;
use super::rig::{RigDefinition, common_rest_rig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BodyId {
    Person,
    Cat,
    Dragon,
}

impl BodyId {
    pub(crate) const ALL: [Self; 3] = [Self::Person, Self::Cat, Self::Dragon];

    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::Person => "cuba:person.v1",
            Self::Cat => "cuba:cat.v1",
            Self::Dragon => "cuba:dragon.v1",
        }
    }
}

impl Default for BodyId {
    fn default() -> Self {
        Self::Person
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharacterColors {
    pub(crate) skin: [f32; 4],
    pub(crate) primary: [f32; 4],
    pub(crate) secondary: [f32; 4],
    pub(crate) sole: [f32; 4],
}

impl Default for CharacterColors {
    fn default() -> Self {
        Self {
            skin: [0.91, 0.55, 0.39, 1.0],
            primary: [0.18, 0.40, 0.39, 1.0],
            secondary: [0.33, 0.42, 0.56, 1.0],
            sole: [0.96, 0.93, 0.84, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharacterAppearance {
    pub(crate) version: u16,
    pub(crate) body: BodyId,
    pub(crate) colors: CharacterColors,
}

impl Default for CharacterAppearance {
    fn default() -> Self {
        Self {
            version: 1,
            body: BodyId::Person,
            colors: CharacterColors::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BodyPart {
    pub(crate) size: Vec3,
    pub(crate) radius: f32,
    pub(crate) taper: (f32, f32),
}

impl BodyPart {
    pub(crate) const fn new(size: Vec3, radius: f32) -> Self {
        Self {
            size,
            radius,
            taper: (1.0, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpeciesExtras {
    pub(crate) ear_size: Option<Vec3>,
    pub(crate) muzzle_size: Option<Vec3>,
    pub(crate) tail_segments: u8,
    pub(crate) horns: bool,
    pub(crate) wings: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BodyRecipe {
    pub(crate) id: BodyId,
    pub(crate) rig: RigDefinition,
    pub(crate) torso: BodyPart,
    pub(crate) head: BodyPart,
    pub(crate) upper_arm: BodyPart,
    pub(crate) lower_arm: BodyPart,
    pub(crate) hand: BodyPart,
    pub(crate) upper_leg: BodyPart,
    pub(crate) lower_leg: BodyPart,
    pub(crate) foot: BodyPart,
    pub(crate) face: FaceAnchors,
    pub(crate) first_person_anchor: Vec3,
    pub(crate) third_person_target: Vec3,
    pub(crate) extras: SpeciesExtras,
}

pub(crate) fn body_recipe(id: BodyId) -> BodyRecipe {
    let rig = common_rest_rig();
    rig.validate().expect("built-in character rig must validate");
    let default_appearance = CharacterAppearance {
        version: 1,
        body: id,
        colors: CharacterColors::default(),
    };
    debug_assert!(
        default_appearance.version == 1
            && default_appearance.body == id
            && default_appearance.colors.skin[3] > 0.0
            && !id.stable_id().is_empty()
    );
    let common = (
        BodyPart::new(Vec3::new(0.98, 1.02, 0.70), 0.13),
        BodyPart::new(Vec3::new(1.02, 0.86, 0.78), 0.16),
        BodyPart::new(Vec3::new(0.34, 0.58, 0.42), 0.08),
        BodyPart::new(Vec3::new(0.34, 0.55, 0.40), 0.08),
        BodyPart::new(Vec3::new(0.42, 0.27, 0.40), 0.10),
        BodyPart::new(Vec3::new(0.48, 0.64, 0.48), 0.10),
        BodyPart::new(Vec3::new(0.45, 0.56, 0.45), 0.09),
        BodyPart::new(Vec3::new(0.62, 0.30, 0.84), 0.10),
    );
    let (mut torso, head, upper_arm, lower_arm, hand, upper_leg, lower_leg, foot) = common;
    // A narrow waist and dropped shoulder line are part of the silhouette,
    // not a material effect. The rounded builder applies this profile before
    // recomputing normals.
    torso.taper = (0.88, 1.0);
    match id {
        BodyId::Person => BodyRecipe {
            id,
            rig,
            torso,
            head,
            upper_arm,
            lower_arm,
            hand,
            upper_leg,
            lower_leg,
            foot,
            face: FaceAnchors::default(),
            first_person_anchor: Vec3::new(0.0, 2.88, -0.03),
            third_person_target: Vec3::new(0.0, 1.62, 0.0),
            extras: SpeciesExtras {
                ear_size: None,
                muzzle_size: None,
                tail_segments: 0,
                horns: false,
                wings: false,
            },
        },
        BodyId::Cat => BodyRecipe {
            id,
            rig,
            torso: BodyPart::new(Vec3::new(1.02, 0.98, 0.72), 0.14),
            head: BodyPart::new(Vec3::new(1.02, 0.84, 0.78), 0.17),
            upper_arm,
            lower_arm,
            hand,
            upper_leg,
            lower_leg,
            foot,
            face: FaceAnchors {
                eye_y: 0.12,
                eye_x: 0.20,
                face_z: -0.45,
                brow_y: 0.28,
                mouth_y: -0.18,
                muzzle_y: -0.12,
            },
            first_person_anchor: Vec3::new(0.0, 2.88, -0.04),
            third_person_target: Vec3::new(0.0, 1.60, 0.0),
            extras: SpeciesExtras {
                ear_size: Some(Vec3::new(0.28, 0.40, 0.28)),
                muzzle_size: Some(Vec3::new(0.45, 0.25, 0.27)),
                tail_segments: 3,
                horns: false,
                wings: false,
            },
        },
        BodyId::Dragon => BodyRecipe {
            id,
            rig,
            torso: BodyPart::new(Vec3::new(1.04, 1.04, 0.76), 0.15),
            head: BodyPart::new(Vec3::new(1.04, 0.82, 0.84), 0.17),
            upper_arm,
            lower_arm,
            hand,
            upper_leg,
            lower_leg,
            foot: BodyPart::new(Vec3::new(0.66, 0.32, 0.88), 0.11),
            face: FaceAnchors {
                eye_y: 0.11,
                eye_x: 0.20,
                face_z: -0.48,
                brow_y: 0.28,
                mouth_y: -0.17,
                muzzle_y: -0.09,
            },
            first_person_anchor: Vec3::new(0.0, 2.86, -0.05),
            third_person_target: Vec3::new(0.0, 1.64, 0.0),
            extras: SpeciesExtras {
                ear_size: None,
                muzzle_size: Some(Vec3::new(0.52, 0.27, 0.32)),
                tail_segments: 4,
                horns: true,
                wings: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::rig::JointId;

    #[test]
    fn all_body_recipes_share_the_valid_rig_and_camera_scale() {
        for body in BodyId::ALL {
            let recipe = body_recipe(body);
            recipe.rig.validate().expect("body rig should validate");
            assert!(recipe.first_person_anchor.y > 2.5);
            assert!(recipe.third_person_target.y > 1.0);
            assert!(recipe.rig.joints[JointId::Head.index()].clearance > 0.0);
        }
    }

    #[test]
    fn species_have_distinct_silhouette_features() {
        let person = body_recipe(BodyId::Person);
        let cat = body_recipe(BodyId::Cat);
        let dragon = body_recipe(BodyId::Dragon);
        assert!(person.extras.ear_size.is_none());
        assert!(cat.extras.ear_size.is_some());
        assert!(dragon.extras.horns && dragon.extras.wings);
    }
}
