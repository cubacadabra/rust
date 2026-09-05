use glam::Vec3;
use std::collections::BTreeMap;

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

    pub(crate) fn from_stable_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.stable_id() == value)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OutfitId {
    EverydayHoodie,
    PufferExplorer,
    GlossyRaincoat,
    StarWizard,
    ToyKnight,
    FuzzyPajamas,
}

impl OutfitId {
    pub(crate) const ALL: [Self; 6] = [
        Self::EverydayHoodie,
        Self::PufferExplorer,
        Self::GlossyRaincoat,
        Self::StarWizard,
        Self::ToyKnight,
        Self::FuzzyPajamas,
    ];

    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::EverydayHoodie => "cuba:everyday-hoodie.v1",
            Self::PufferExplorer => "cuba:puffer-explorer.v1",
            Self::GlossyRaincoat => "cuba:glossy-raincoat.v1",
            Self::StarWizard => "cuba:star-wizard.v1",
            Self::ToyKnight => "cuba:toy-knight.v1",
            Self::FuzzyPajamas => "cuba:fuzzy-pajamas.v1",
        }
    }

    pub(crate) fn from_stable_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.stable_id() == value)
    }

    /// The deliberately small Phase 5 fit matrix. Hoodie and raincoat prove
    /// common-garment reuse; the four hero outfits are authored fits.
    pub(crate) const fn supported_by(self, body: BodyId) -> bool {
        match self {
            Self::EverydayHoodie | Self::GlossyRaincoat => true,
            Self::PufferExplorer => matches!(body, BodyId::Cat),
            Self::StarWizard | Self::ToyKnight => matches!(body, BodyId::Dragon),
            Self::FuzzyPajamas => matches!(body, BodyId::Person),
        }
    }

    pub(crate) const fn fallback() -> Self {
        Self::EverydayHoodie
    }

    #[allow(dead_code)]
    pub(crate) const fn material_family(self) -> &'static str {
        match self {
            Self::EverydayHoodie => "cloth-denim-rubber",
            Self::PufferExplorer => "quilted-cloth-rubber",
            Self::GlossyRaincoat => "waterproof-gloss-rubber",
            Self::StarWizard => "cloth-trim-emission",
            Self::ToyKnight => "soft-metal-padded-cloth",
            Self::FuzzyPajamas => "fuzz-cloth-rubber",
        }
    }
}

impl Default for OutfitId {
    fn default() -> Self {
        Self::EverydayHoodie
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EquipmentSlot {
    Hat,
    Glasses,
    EarAccessory,
    Neck,
    Back,
    Waist,
    LeftHand,
    RightHand,
    Tail,
    Wings,
}

impl EquipmentSlot {
    #[allow(dead_code)]
    pub(crate) const ALL: [Self; 10] = [
        Self::Hat,
        Self::Glasses,
        Self::EarAccessory,
        Self::Neck,
        Self::Back,
        Self::Waist,
        Self::LeftHand,
        Self::RightHand,
        Self::Tail,
        Self::Wings,
    ];

    pub(crate) fn from_id(value: &str) -> Option<Self> {
        match value {
            "hat" => Some(Self::Hat),
            "glasses" => Some(Self::Glasses),
            "ear-accessory" | "earAccessory" => Some(Self::EarAccessory),
            "neck" => Some(Self::Neck),
            "back" => Some(Self::Back),
            "waist" => Some(Self::Waist),
            "left-hand" | "leftHand" => Some(Self::LeftHand),
            "right-hand" | "rightHand" => Some(Self::RightHand),
            "tail" => Some(Self::Tail),
            "wings" => Some(Self::Wings),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EquipmentItem {
    pub(crate) slot: EquipmentSlot,
    pub(crate) asset_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CharacterAppearance {
    pub(crate) version: u16,
    pub(crate) body: BodyId,
    pub(crate) face: crate::character::FacePreset,
    pub(crate) outfit: OutfitId,
    pub(crate) equipment: Vec<EquipmentItem>,
    pub(crate) colors: CharacterColors,
    pub(crate) revision: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AppearanceIssue {
    UnsupportedVersion(u16),
    InvalidBody,
    InvalidFace,
    InvalidOutfit,
    InvalidColor(String),
    UnsupportedFit { outfit: OutfitId, body: BodyId },
    InvalidEquipmentSlot(String),
    InvalidAssetId(String),
    TooManyEquipment,
    TooManyColors,
    AppearanceTooLarge,
}

pub(crate) struct AppearanceInput<'a> {
    pub(crate) version: Option<u16>,
    pub(crate) body: Option<&'a str>,
    pub(crate) face: Option<&'a str>,
    pub(crate) outfit: Option<&'a str>,
    pub(crate) equipment: &'a BTreeMap<String, String>,
    pub(crate) colors: &'a BTreeMap<String, String>,
    pub(crate) legacy_colors: CharacterColors,
    pub(crate) revision: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppearanceResolution {
    pub(crate) appearance: CharacterAppearance,
    pub(crate) issues: Vec<AppearanceIssue>,
}

pub(crate) fn resolve_appearance(input: AppearanceInput<'_>) -> AppearanceResolution {
    let mut issues = Vec::new();
    let version = input.version.unwrap_or(1);
    let supported_version = version == 1;
    if !supported_version {
        issues.push(AppearanceIssue::UnsupportedVersion(version));
    }
    let body = if supported_version {
        input
            .body
            .and_then(BodyId::from_stable_id)
            .unwrap_or_else(|| {
                if input.body.is_some() {
                    issues.push(AppearanceIssue::InvalidBody);
                }
                BodyId::Person
            })
    } else {
        BodyId::Person
    };
    let face = if supported_version {
        input.face.and_then(face_preset_from_id).unwrap_or_else(|| {
            if input.face.is_some() {
                issues.push(AppearanceIssue::InvalidFace);
            }
            crate::character::FacePreset::Happy
        })
    } else {
        crate::character::FacePreset::Happy
    };
    let requested_outfit = supported_version
        .then(|| input.outfit.and_then(OutfitId::from_stable_id))
        .flatten();
    if supported_version && input.outfit.is_some() && requested_outfit.is_none() {
        issues.push(AppearanceIssue::InvalidOutfit);
    }
    let outfit = requested_outfit
        .filter(|outfit| outfit.supported_by(body))
        .unwrap_or_else(|| {
            if let Some(requested) = requested_outfit {
                if !requested.supported_by(body) {
                    issues.push(AppearanceIssue::UnsupportedFit {
                        outfit: requested,
                        body,
                    });
                }
            }
            OutfitId::fallback()
        });

    let mut equipment = Vec::new();
    if input.equipment.len() > 32 {
        issues.push(AppearanceIssue::TooManyEquipment);
    }
    for (slot_id, asset_id) in input.equipment.iter().take(32) {
        let Some(slot) = EquipmentSlot::from_id(slot_id) else {
            issues.push(AppearanceIssue::InvalidEquipmentSlot(slot_id.clone()));
            continue;
        };
        if !valid_asset_id(asset_id) {
            issues.push(AppearanceIssue::InvalidAssetId(asset_id.clone()));
            continue;
        }
        equipment.push(EquipmentItem {
            slot,
            asset_id: asset_id.clone(),
        });
    }
    if input.colors.len() > 16 {
        issues.push(AppearanceIssue::TooManyColors);
    }
    let mut colors = input.legacy_colors;
    // Named channels intentionally override legacy colors only when valid;
    // an invalid channel cannot erase the rest of an otherwise valid look.
    if supported_version {
        for (channel, value) in input.colors {
            if !matches!(channel.as_str(), "skin" | "primary" | "secondary" | "sole")
                || parse_color(value).is_none()
            {
                issues.push(AppearanceIssue::InvalidColor(channel.clone()));
            }
        }
        apply_color(&mut colors.skin, input.colors.get("skin"));
        apply_color(&mut colors.primary, input.colors.get("primary"));
        apply_color(&mut colors.secondary, input.colors.get("secondary"));
        apply_color(&mut colors.sole, input.colors.get("sole"));
    } else {
        equipment.clear();
    }

    let approximate_size = input
        .body
        .unwrap_or("")
        .len()
        .saturating_add(input.face.unwrap_or("").len())
        .saturating_add(input.outfit.unwrap_or("").len())
        .saturating_add(
            input
                .equipment
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>(),
        )
        .saturating_add(
            input
                .colors
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>(),
        );
    if approximate_size > 4096 {
        issues.push(AppearanceIssue::AppearanceTooLarge);
    }

    let appearance = CharacterAppearance {
        version: if supported_version { version } else { 1 },
        body,
        face,
        outfit,
        equipment,
        colors,
        revision: input.revision,
    };
    AppearanceResolution { appearance, issues }
}

fn valid_asset_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'-' | b'_' | b'/')
        })
}

fn apply_color(target: &mut [f32; 4], value: Option<&String>) {
    let Some(value) = value.and_then(|value| parse_color(value)) else {
        return;
    };
    *target = value;
}

fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(value, 16).ok()?;
    Some([
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        1.0,
    ])
}

fn face_preset_from_id(value: &str) -> Option<crate::character::FacePreset> {
    crate::character::FacePreset::ALL
        .into_iter()
        .find(|preset| preset.stable_id() == value)
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

impl Default for CharacterAppearance {
    fn default() -> Self {
        resolve_appearance(AppearanceInput {
            version: Some(1),
            body: Some(BodyId::Person.stable_id()),
            face: Some("happy"),
            outfit: Some(OutfitId::EverydayHoodie.stable_id()),
            equipment: &BTreeMap::new(),
            colors: &BTreeMap::new(),
            legacy_colors: CharacterColors::default(),
            revision: 0,
        })
        .appearance
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
    rig.validate()
        .expect("built-in character rig must validate");
    let default_appearance = CharacterAppearance {
        version: 1,
        body: id,
        face: crate::character::FacePreset::Happy,
        outfit: OutfitId::EverydayHoodie,
        equipment: Vec::new(),
        colors: CharacterColors::default(),
        revision: 0,
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

    #[test]
    fn outfit_matrix_keeps_common_fits_and_rejects_hero_mismatches() {
        assert!(OutfitId::EverydayHoodie.supported_by(BodyId::Person));
        assert!(OutfitId::EverydayHoodie.supported_by(BodyId::Cat));
        assert!(OutfitId::GlossyRaincoat.supported_by(BodyId::Dragon));
        assert!(OutfitId::PufferExplorer.supported_by(BodyId::Cat));
        assert!(!OutfitId::PufferExplorer.supported_by(BodyId::Person));
        assert!(!OutfitId::ToyKnight.supported_by(BodyId::Cat));
    }

    #[test]
    fn appearance_resolution_is_bounded_and_atomic_on_bad_fit() {
        let mut equipment = BTreeMap::new();
        equipment.insert("hat".to_owned(), "cuba:star-cap.v1".to_owned());
        let mut colors = BTreeMap::new();
        colors.insert("primary".to_owned(), "#336699".to_owned());
        let result = resolve_appearance(AppearanceInput {
            version: Some(1),
            body: Some("cuba:person.v1"),
            face: Some("happy"),
            outfit: Some("cuba:toy-knight.v1"),
            equipment: &equipment,
            colors: &colors,
            legacy_colors: CharacterColors::default(),
            revision: 4,
        });
        assert_eq!(result.appearance.outfit, OutfitId::EverydayHoodie);
        assert_eq!(result.appearance.revision, 4);
        assert_eq!(result.appearance.equipment.len(), 1);
        assert_eq!(result.appearance.colors.primary, [0.2, 0.4, 0.6, 1.0]);
        assert!(
            result
                .issues
                .iter()
                .any(|issue| matches!(issue, AppearanceIssue::UnsupportedFit { .. }))
        );
    }

    #[test]
    fn unsupported_schema_version_uses_legacy_defaults() {
        let colors = BTreeMap::new();
        let result = resolve_appearance(AppearanceInput {
            version: Some(99),
            body: Some("cuba:dragon.v1"),
            face: Some("angry"),
            outfit: Some("cuba:star-wizard.v1"),
            equipment: &BTreeMap::new(),
            colors: &colors,
            legacy_colors: CharacterColors::default(),
            revision: 0,
        });
        assert_eq!(result.appearance.body, BodyId::Person);
        assert_eq!(result.appearance.outfit, OutfitId::EverydayHoodie);
        assert!(
            result
                .issues
                .iter()
                .any(|issue| matches!(issue, AppearanceIssue::UnsupportedVersion(99)))
        );
    }
}
