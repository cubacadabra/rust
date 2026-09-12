use glam::{Vec2, Vec3};
use std::collections::BTreeMap;

use super::face::FaceAnchors;
use super::rig::{JointId, RigDefinition, common_rest_rig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BodyId {
    Person,
    PersonGirl,
    PersonNonbinary,
    Cat,
    Wolf,
    Dragon,
}

impl BodyId {
    pub(crate) const ALL: [Self; 6] = [
        Self::Person,
        Self::PersonGirl,
        Self::PersonNonbinary,
        Self::Cat,
        Self::Wolf,
        Self::Dragon,
    ];

    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::Person => "cuba:person.v1",
            Self::PersonGirl => "cuba:person-girl.v1",
            Self::PersonNonbinary => "cuba:person-nb.v1",
            Self::Cat => "cuba:cat.v1",
            Self::Wolf => "cuba:wolf.v1",
            Self::Dragon => "cuba:dragon.v1",
        }
    }

    pub(crate) const fn is_person(self) -> bool {
        matches!(
            self,
            Self::Person | Self::PersonGirl | Self::PersonNonbinary
        )
    }

    pub(crate) fn hair_color(self) -> [f32; 4] {
        if let Some((_, style)) = super::hair::for_body(self) {
            let [r, g, b] = style.color;
            return [r, g, b, 1.0];
        }
        match self {
            Self::Person => [0.30, 0.155, 0.085, 1.0],
            _ => [0.22, 0.12, 0.075, 1.0],
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

// These colors are deliberately spaced across the hue wheel so a crowd is
// easy to tell apart while remaining bright enough to read on the game world.
pub(crate) const VIBRANT_HOODIE_COLORS: [[f32; 4]; 18] = [
    [0.937, 0.278, 0.435, 1.0], // coral pink
    [0.067, 0.533, 0.698, 1.0], // ocean blue
    [0.961, 0.612, 0.102, 1.0], // amber
    [0.165, 0.616, 0.561, 1.0], // teal
    [0.482, 0.173, 0.737, 1.0], // violet
    [0.937, 0.373, 0.196, 1.0], // tangerine
    [0.263, 0.380, 0.933, 1.0], // royal blue
    [0.831, 0.208, 0.522, 1.0], // magenta
    [0.000, 0.659, 0.471, 1.0], // emerald
    [0.906, 0.216, 0.247, 1.0], // red
    [0.380, 0.271, 0.812, 1.0], // indigo
    [0.902, 0.361, 0.024, 1.0], // orange
    [0.518, 0.800, 0.086, 1.0], // lime
    [0.024, 0.714, 0.659, 1.0], // sea green
    [0.055, 0.647, 0.914, 1.0], // sky blue
    [0.659, 0.333, 0.969, 1.0], // purple
    [0.925, 0.282, 0.600, 1.0], // hot pink
    [0.957, 0.247, 0.369, 1.0], // strawberry
];

pub(crate) fn vibrant_hoodie_color(index: usize) -> [f32; 4] {
    // A coprime step gives a shuffled palette order while guaranteeing that
    // the first full palette cycle never repeats a color.
    let palette_index = index.wrapping_mul(7).wrapping_add(3) % VIBRANT_HOODIE_COLORS.len();
    VIBRANT_HOODIE_COLORS[palette_index]
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
            Self::PufferExplorer => matches!(body, BodyId::Cat | BodyId::Wolf),
            Self::StarWizard | Self::ToyKnight => matches!(body, BodyId::Dragon),
            Self::FuzzyPajamas => body.is_person(),
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
    Base,
    Hair,
    EarDevice,
    Shirt,
    Pants,
    Shoes,
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
    pub(crate) const ALL: &'static [Self] = &[
        Self::Base,
        Self::Hair,
        Self::EarDevice,
        Self::Shirt,
        Self::Pants,
        Self::Shoes,
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
            "base" => Some(Self::Base),
            "hair" => Some(Self::Hair),
            "ear-device" => Some(Self::EarDevice),
            "shirt" => Some(Self::Shirt),
            "pants" => Some(Self::Pants),
            "shoes" => Some(Self::Shoes),
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
    cubacadabra_morphs::MorphAssetId::parse(value).is_ok()
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
    let mut rig = common_rest_rig();
    let set_joint = |rig: &mut RigDefinition, joint: JointId, translation: Vec3| {
        rig.joints[joint.index()].rest.translation = translation;
    };

    // The common hierarchy keeps outfits and animation portable. Rest
    // positions give each species its own stance and distribution of mass.
    match id {
        BodyId::Person | BodyId::PersonGirl | BodyId::PersonNonbinary => {
            set_joint(&mut rig, JointId::Torso, Vec3::new(0.0, 1.68, 0.0));
            set_joint(&mut rig, JointId::Head, Vec3::new(0.0, 1.02, 0.0));
            set_joint(
                &mut rig,
                JointId::LeftUpperArm,
                Vec3::new(-0.59, -0.01, 0.0),
            );
            set_joint(
                &mut rig,
                JointId::RightUpperArm,
                Vec3::new(0.59, -0.01, 0.0),
            );
            set_joint(&mut rig, JointId::LeftUpperLeg, Vec3::new(-0.22, 0.87, 0.0));
            set_joint(&mut rig, JointId::RightUpperLeg, Vec3::new(0.22, 0.87, 0.0));
            set_joint(&mut rig, JointId::LeftLowerLeg, Vec3::new(0.0, -0.53, 0.0));
            set_joint(&mut rig, JointId::RightLowerLeg, Vec3::new(0.0, -0.53, 0.0));
            set_joint(&mut rig, JointId::LeftFoot, Vec3::new(0.0, -0.29, -0.13));
            set_joint(&mut rig, JointId::RightFoot, Vec3::new(0.0, -0.29, -0.13));
        }
        BodyId::Cat | BodyId::Wolf => {
            set_joint(&mut rig, JointId::Torso, Vec3::new(0.0, 1.64, 0.015));
            set_joint(&mut rig, JointId::Head, Vec3::new(0.0, 1.00, -0.015));
            set_joint(
                &mut rig,
                JointId::LeftUpperArm,
                Vec3::new(-0.62, -0.03, 0.0),
            );
            set_joint(
                &mut rig,
                JointId::RightUpperArm,
                Vec3::new(0.62, -0.03, 0.0),
            );
            set_joint(&mut rig, JointId::LeftLowerArm, Vec3::new(0.0, -0.54, 0.0));
            set_joint(&mut rig, JointId::RightLowerArm, Vec3::new(0.0, -0.54, 0.0));
            set_joint(&mut rig, JointId::LeftHand, Vec3::new(0.0, -0.45, -0.02));
            set_joint(&mut rig, JointId::RightHand, Vec3::new(0.0, -0.45, -0.02));
            set_joint(&mut rig, JointId::LeftUpperLeg, Vec3::new(-0.29, 0.84, 0.0));
            set_joint(&mut rig, JointId::RightUpperLeg, Vec3::new(0.29, 0.84, 0.0));
            set_joint(&mut rig, JointId::LeftLowerLeg, Vec3::new(0.0, -0.50, 0.0));
            set_joint(&mut rig, JointId::RightLowerLeg, Vec3::new(0.0, -0.50, 0.0));
            set_joint(&mut rig, JointId::LeftFoot, Vec3::new(0.0, -0.29, -0.08));
            set_joint(&mut rig, JointId::RightFoot, Vec3::new(0.0, -0.29, -0.08));
        }
        BodyId::Dragon => {
            set_joint(&mut rig, JointId::Torso, Vec3::new(0.0, 1.68, 0.025));
            set_joint(&mut rig, JointId::Head, Vec3::new(0.0, 1.00, -0.04));
            set_joint(&mut rig, JointId::LeftUpperArm, Vec3::new(-0.69, 0.07, 0.0));
            set_joint(&mut rig, JointId::RightUpperArm, Vec3::new(0.69, 0.07, 0.0));
            set_joint(&mut rig, JointId::LeftHand, Vec3::new(0.0, -0.47, -0.03));
            set_joint(&mut rig, JointId::RightHand, Vec3::new(0.0, -0.47, -0.03));
            set_joint(
                &mut rig,
                JointId::LeftUpperLeg,
                Vec3::new(-0.31, 0.88, 0.02),
            );
            set_joint(
                &mut rig,
                JointId::RightUpperLeg,
                Vec3::new(0.31, 0.88, 0.02),
            );
            set_joint(&mut rig, JointId::LeftLowerLeg, Vec3::new(0.0, -0.55, 0.0));
            set_joint(&mut rig, JointId::RightLowerLeg, Vec3::new(0.0, -0.55, 0.0));
            set_joint(&mut rig, JointId::LeftFoot, Vec3::new(0.0, -0.28, -0.16));
            set_joint(&mut rig, JointId::RightFoot, Vec3::new(0.0, -0.28, -0.16));
        }
    }
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
    let mut recipe = match id {
        BodyId::Person | BodyId::PersonGirl | BodyId::PersonNonbinary => {
            let mut torso = BodyPart::new(Vec3::new(0.90, 1.06, 0.66), 0.15);
            torso.taper = (0.80, 1.0);
            BodyRecipe {
                id,
                rig,
                torso,
                head: BodyPart::new(Vec3::new(1.10, 0.92, 0.78), 0.19),
                upper_arm: BodyPart::new(Vec3::new(0.31, 0.56, 0.39), 0.09),
                lower_arm: BodyPart::new(Vec3::new(0.30, 0.52, 0.37), 0.09),
                hand: BodyPart::new(Vec3::new(0.44, 0.29, 0.41), 0.12),
                upper_leg: BodyPart::new(Vec3::new(0.42, 0.61, 0.45), 0.11),
                lower_leg: BodyPart::new(Vec3::new(0.40, 0.52, 0.42), 0.10),
                foot: BodyPart::new(Vec3::new(0.65, 0.31, 0.88), 0.12),
                face: FaceAnchors::default(),
                first_person_anchor: Vec3::new(0.0, 2.76, -0.04),
                third_person_target: Vec3::new(0.0, 1.58, 0.0),
                extras: SpeciesExtras {
                    ear_size: None,
                    muzzle_size: None,
                    tail_segments: 0,
                    horns: false,
                    wings: false,
                },
            }
        }
        BodyId::Cat | BodyId::Wolf => {
            let mut torso = BodyPart::new(Vec3::new(1.04, 0.98, 0.74), 0.17);
            torso.taper = (1.08, 0.82);
            BodyRecipe {
                id,
                rig,
                torso,
                head: BodyPart::new(Vec3::new(1.18, 0.82, 0.76), 0.20),
                upper_arm: BodyPart::new(Vec3::new(0.37, 0.55, 0.43), 0.11),
                lower_arm: BodyPart::new(Vec3::new(0.36, 0.48, 0.41), 0.11),
                hand: BodyPart::new(Vec3::new(0.47, 0.30, 0.44), 0.13),
                upper_leg: BodyPart::new(Vec3::new(0.50, 0.56, 0.50), 0.13),
                lower_leg: BodyPart::new(Vec3::new(0.48, 0.48, 0.47), 0.12),
                foot: BodyPart::new(Vec3::new(0.62, 0.29, 0.74), 0.13),
                face: FaceAnchors {
                    eye_y: 0.11,
                    eye_x: 0.22,
                    eye_size: Vec2::new(0.19, 0.25),
                    eye_tilt: 0.055,
                    face_z: -0.387,
                    brow_y: 0.27,
                    brow_width: 0.20,
                    mouth_y: -0.17,
                    mouth_width: 0.28,
                    muzzle_y: -0.12,
                },
                first_person_anchor: Vec3::new(0.0, 2.70, -0.04),
                third_person_target: Vec3::new(0.0, 1.55, 0.0),
                extras: SpeciesExtras {
                    ear_size: Some(Vec3::new(0.34, 0.50, 0.30)),
                    muzzle_size: Some(Vec3::new(0.48, 0.24, 0.26)),
                    tail_segments: 3,
                    horns: false,
                    wings: false,
                },
            }
        }
        BodyId::Dragon => {
            let mut torso = BodyPart::new(Vec3::new(1.16, 1.08, 0.84), 0.18);
            torso.taper = (0.92, 1.08);
            BodyRecipe {
                id,
                rig,
                torso,
                head: BodyPart::new(Vec3::new(1.10, 0.86, 0.92), 0.19),
                upper_arm: BodyPart::new(Vec3::new(0.40, 0.58, 0.47), 0.12),
                lower_arm: BodyPart::new(Vec3::new(0.39, 0.53, 0.45), 0.11),
                hand: BodyPart::new(Vec3::new(0.50, 0.31, 0.47), 0.13),
                upper_leg: BodyPart::new(Vec3::new(0.54, 0.62, 0.54), 0.13),
                lower_leg: BodyPart::new(Vec3::new(0.51, 0.51, 0.50), 0.12),
                foot: BodyPart::new(Vec3::new(0.74, 0.35, 0.98), 0.14),
                face: FaceAnchors {
                    eye_y: 0.11,
                    eye_x: 0.21,
                    eye_size: Vec2::new(0.20, 0.23),
                    eye_tilt: -0.09,
                    face_z: -0.467,
                    brow_y: 0.27,
                    brow_width: 0.23,
                    mouth_y: -0.16,
                    mouth_width: 0.31,
                    muzzle_y: -0.09,
                },
                first_person_anchor: Vec3::new(0.0, 2.74, -0.07),
                third_person_target: Vec3::new(0.0, 1.62, 0.02),
                extras: SpeciesExtras {
                    ear_size: None,
                    muzzle_size: Some(Vec3::new(0.56, 0.29, 0.36)),
                    tail_segments: 4,
                    horns: true,
                    wings: true,
                },
            }
        }
    };
    if id == BodyId::Wolf {
        // Wolves share the creature rig, but carry a longer muzzle, broader
        // shoulders, and a lower, fuller tail than the compact cat body.
        recipe.torso = BodyPart::new(Vec3::new(1.18, 0.94, 0.86), 0.18);
        recipe.torso.taper = (1.12, 0.86);
        recipe.head = BodyPart::new(Vec3::new(1.30, 0.86, 0.88), 0.21);
        recipe.upper_leg = BodyPart::new(Vec3::new(0.54, 0.60, 0.54), 0.14);
        recipe.lower_leg = BodyPart::new(Vec3::new(0.51, 0.51, 0.50), 0.13);
        recipe.foot = BodyPart::new(Vec3::new(0.68, 0.30, 0.82), 0.14);
        recipe.face.muzzle_y = -0.15;
        recipe.extras.ear_size = Some(Vec3::new(0.30, 0.56, 0.28));
        recipe.extras.muzzle_size = Some(Vec3::new(0.60, 0.27, 0.34));
        recipe.extras.tail_segments = 4;
    }
    recipe
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
        let wolf = body_recipe(BodyId::Wolf);
        let dragon = body_recipe(BodyId::Dragon);
        assert!(person.extras.ear_size.is_none());
        assert!(cat.extras.ear_size.is_some());
        assert!(wolf.extras.muzzle_size.unwrap().x > cat.extras.muzzle_size.unwrap().x);
        assert_ne!(wolf.head.size, cat.head.size);
        assert!(dragon.extras.horns && dragon.extras.wings);
        assert!(cat.head.size.x > person.head.size.x);
        assert!(dragon.torso.size.x > person.torso.size.x);
        assert!(
            person.rig.joints[JointId::LeftUpperArm.index()]
                .rest
                .translation
                .x
                .abs()
                < cat.rig.joints[JointId::LeftUpperArm.index()]
                    .rest
                    .translation
                    .x
                    .abs()
        );
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
