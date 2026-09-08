//! Immutable authored rigid pieces; routine animation changes transforms.
use super::AvatarStyle;
use super::character_material::Material;
use crate::character::{
    BodyId, BodyPart, BodyRecipe, FaceParameters, FacePreset, JointId, OutfitId, Pose, body_recipe,
};
use glam::{Mat4, Quat, Vec3};

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
fn camera_anchors(body: BodyId) -> (Vec3, Vec3) {
    static ANCHORS: std::sync::OnceLock<[(Vec3, Vec3); 6]> = std::sync::OnceLock::new();
    ANCHORS.get_or_init(|| {
        BodyId::ALL.map(|id| {
            let recipe = body_recipe(id);
            (recipe.first_person_anchor, recipe.third_person_target)
        })
    })[BodyId::ALL.iter().position(|id| *id == body).unwrap_or(0)]
}
pub(super) fn camera_anchor(body: BodyId) -> Vec3 {
    camera_anchors(body).0
}
pub(super) fn camera_target(body: BodyId) -> Vec3 {
    camera_anchors(body).1
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

fn base_parts(recipe: &BodyRecipe) -> Vec<Part> {
    let mut vertices = Vec::with_capacity(48);
    let vertices = &mut vertices;
    let root = Anchor::new(JointId::Root);
    let part = |vertices: &mut Vec<Part>, joint: JointId, spec: BodyPart, color: Tint| {
        add_part(vertices, Anchor::new(joint), spec, color);
    };
    part(vertices, JointId::Torso, recipe.torso, Tint::Shirt);
    part(vertices, JointId::Head, recipe.head, Tint::Skin);
    part(
        vertices,
        JointId::LeftUpperArm,
        recipe.upper_arm,
        Tint::Shirt,
    );
    part(
        vertices,
        JointId::LeftLowerArm,
        recipe.lower_arm,
        Tint::Shirt,
    );
    part(vertices, JointId::LeftHand, recipe.hand, Tint::Skin);
    part(
        vertices,
        JointId::RightUpperArm,
        recipe.upper_arm,
        Tint::Shirt,
    );
    part(
        vertices,
        JointId::RightLowerArm,
        recipe.lower_arm,
        Tint::Shirt,
    );
    part(vertices, JointId::RightHand, recipe.hand, Tint::Skin);
    part(
        vertices,
        JointId::LeftUpperLeg,
        recipe.upper_leg,
        Tint::Pants,
    );
    part(
        vertices,
        JointId::LeftLowerLeg,
        recipe.lower_leg,
        Tint::Pants,
    );
    part(
        vertices,
        JointId::RightUpperLeg,
        recipe.upper_leg,
        Tint::Pants,
    );
    part(
        vertices,
        JointId::RightLowerLeg,
        recipe.lower_leg,
        Tint::Pants,
    );
    part(vertices, JointId::LeftFoot, recipe.foot, Tint::Shoes);
    part(vertices, JointId::RightFoot, recipe.foot, Tint::Shoes);

    // Facial proportions are authored once in a neutral state. Expression
    // parameters scale/offset these same pieces at presentation time, so
    // twenty expressions do not multiply body geometry.
    let face = FaceParameters::preset(FacePreset::Neutral);
    add_face(
        vertices,
        Anchor::new(JointId::Head),
        &recipe,
        face,
        Tint::Face,
    );
    add_species_parts(vertices, root, Anchor::new(JointId::Head), &recipe);
    add_seam_cores(vertices, recipe);
    // Magic is an event/accessory accent, not exposed anatomy for people.
    if recipe.id.is_person() {
        vertices.retain(|part| !matches!(part.feature, Feature::Seam(_)));
    }
    std::mem::take(vertices)
}

pub(super) fn parts_for(recipe: &BodyRecipe, outfit: OutfitId) -> Vec<Part> {
    let mut vertices = base_parts(recipe);
    // Keeping this as a finite authored catalog prevents arbitrary runtime
    // geometry keys from entering the GPU cache.
    apply_outfit(&mut vertices, recipe, outfit);
    finish_outfit(&mut vertices, recipe, outfit);
    if recipe.id.is_person() && outfit == OutfitId::EverydayHoodie {
        super::hero_character::finish(&mut vertices, recipe.id);
    } else if recipe.id.is_person() && outfit != OutfitId::GlossyRaincoat {
        super::hero_character::replace_hair(&mut vertices, recipe.id);
    }
    vertices
}

// Small construction details share one immutable rounded mesh. Their scale
// belongs to the attachment, keeping the finite catalog and upload bounded.
fn detail(parts: &mut Vec<Part>, anchor: Anchor, position: Vec3, size: Vec3, tint: Tint) {
    add_part(
        parts,
        anchor * Mat4::from_translation(position) * Mat4::from_scale(size / 0.2),
        BodyPart::new(Vec3::splat(0.2), 0.04),
        tint,
    );
}

fn sole_detail(parts: &mut Vec<Part>, anchor: Anchor, position: Vec3, size: Vec3, tint: Tint) {
    let part_start = parts.len();
    detail(parts, anchor, position, size, tint);
    parts[part_start].feature = Feature::Sole;
}

fn finish_outfit(parts: &mut Vec<Part>, recipe: &BodyRecipe, outfit: OutfitId) {
    let torso = Anchor::new(JointId::Torso);
    // Garment materials apply to the garment itself, including sleeves.
    for part in parts.iter_mut() {
        if matches!(part.tint, Tint::Shirt) && !matches!(part.feature, Feature::Wing(_)) {
            part.tint = match outfit {
                OutfitId::GlossyRaincoat => Tint::Outer,
                OutfitId::ToyKnight if part.anchor.joint == JointId::Torso => Tint::Armor,
                OutfitId::FuzzyPajamas => Tint::Fuzz,
                _ => part.tint,
            };
        }
    }
    // The foot origin is near the floor. Raise its authored center so the
    // chunky toe and sole sit above the receiver instead of being buried.
    for joint in [JointId::LeftFoot, JointId::RightFoot] {
        if let Some(foot) = parts
            .iter_mut()
            .find(|p| p.anchor.joint == joint && matches!(p.tint, Tint::Shoes))
        {
            foot.anchor.local = Mat4::from_translation(Vec3::new(0.0, foot.spec.size.y * 0.5, 0.0));
            let size = foot.spec.size;
            sole_detail(
                parts,
                Anchor::new(joint),
                Vec3::new(0.0, 0.015, 0.0),
                Vec3::new(size.x * 1.02, 0.09, size.z * 1.02),
                Tint::Ivory,
            );
            if matches!(outfit, OutfitId::EverydayHoodie | OutfitId::PufferExplorer) {
                detail(
                    parts,
                    Anchor::new(joint),
                    Vec3::new(0.0, size.y, -0.16),
                    Vec3::new(0.28, 0.035, 0.16),
                    Tint::Ivory,
                );
            }
        }
    }
    match outfit {
        OutfitId::EverydayHoodie => {
            // A folded hood reads from behind; paired cords sit on the chest.
            detail(
                parts,
                torso,
                Vec3::new(0.0, 0.42, 0.29),
                Vec3::new(0.78, 0.34, 0.38),
                Tint::Shirt,
            );
            for side in [-1.0, 1.0] {
                detail(
                    parts,
                    torso,
                    Vec3::new(side * 0.15, 0.24, -0.395),
                    Vec3::new(0.035, 0.30, 0.04),
                    Tint::Ivory,
                );
            }
        }
        OutfitId::PufferExplorer => {
            for y in [-0.30, -0.03, 0.24] {
                detail(
                    parts,
                    torso,
                    Vec3::new(0.0, y, 0.0),
                    Vec3::new(1.27, 0.23, 0.88),
                    Tint::Shirt,
                );
            }
            detail(
                parts,
                torso,
                Vec3::new(0.0, 0.03, -0.465),
                Vec3::new(0.045, 0.80, 0.035),
                Tint::Ivory,
            );
        }
        OutfitId::GlossyRaincoat => {
            parts.retain(|part| !matches!(part.tint, Tint::Hair));
            let head = Anchor::new(JointId::Head);
            let size = recipe.head.size;
            detail(
                parts,
                head,
                Vec3::new(0.0, 0.0, size.z * 0.5),
                Vec3::new(size.x + 0.12, size.y + 0.10, 0.20),
                Tint::Outer,
            );
            for side in [-1.0, 1.0] {
                detail(
                    parts,
                    head,
                    Vec3::new(side * (size.x * 0.5 + 0.04), 0.0, 0.04),
                    Vec3::new(0.16, size.y + 0.10, size.z + 0.08),
                    Tint::Outer,
                );
            }
            for side in [-1.0, 1.0] {
                detail(
                    parts,
                    torso,
                    Vec3::new(side * 0.32, -0.20, -0.43),
                    Vec3::new(0.25, 0.17, 0.05),
                    Tint::Shirt,
                );
            }
            for y in [-0.22, 0.22] {
                detail(
                    parts,
                    torso,
                    Vec3::new(0.0, y, -0.43),
                    Vec3::new(0.06, 0.06, 0.04),
                    Tint::Ivory,
                );
            }
        }
        OutfitId::StarWizard => {
            let head = Anchor::new(JointId::Head);
            detail(
                parts,
                head,
                Vec3::new(0.0, 0.49, 0.03),
                Vec3::new(1.18, 0.12, 1.0),
                Tint::Outer,
            );
            for side in [-1.0, 1.0] {
                detail(
                    parts,
                    torso * Mat4::from_rotation_z(side * 0.12),
                    Vec3::new(side * 0.35, -0.12, -0.455),
                    Vec3::new(0.055, 0.95, 0.035),
                    Tint::Ivory,
                );
            }
        }
        OutfitId::ToyKnight => {
            detail(
                parts,
                torso,
                Vec3::new(0.0, 0.26, -0.44),
                Vec3::splat(0.09),
                Tint::Ivory,
            );
            detail(
                parts,
                torso,
                Vec3::new(0.0, -0.35, 0.0),
                Vec3::new(1.12, 0.12, 0.87),
                Tint::Detail,
            );
        }
        OutfitId::FuzzyPajamas => {
            for y in [-0.24, 0.0, 0.24] {
                detail(
                    parts,
                    torso,
                    Vec3::new(0.0, y, -0.39),
                    Vec3::splat(0.06),
                    Tint::Ivory,
                );
            }
            detail(
                parts,
                torso,
                Vec3::new(-0.28, 0.16, -0.39),
                Vec3::new(0.22, 0.20, 0.04),
                Tint::Ivory,
            );
        }
    }
    let torso_spec = parts
        .iter()
        .find(|p| p.anchor.joint == JointId::Torso)
        .map_or(recipe.torso, |p| p.spec);
    // Surface details follow the garment's depth and taper, including the
    // wider animal bodies. Keep their back face just inside the cloth.
    for part in parts
        .iter_mut()
        .filter(|p| p.anchor.joint == JointId::Torso)
    {
        if part.anchor.local.w_axis.z < -0.30 && matches!(part.feature, Feature::None) {
            let extent = part
                .anchor
                .local
                .transform_vector3(Vec3::Y * part.spec.size.y * 0.5)
                .y
                .abs();
            let y = part.anchor.local.w_axis.y;
            let taper_at = |y: f32| {
                let t = (y / torso_spec.size.y + 0.5).clamp(0.0, 1.0);
                torso_spec.taper.0 + (torso_spec.taper.1 - torso_spec.taper.0) * t
            };
            let depth = torso_spec.size.z * 0.5 * taper_at(y - extent).max(taper_at(y + extent));
            part.anchor.local.w_axis.z = part
                .anchor
                .local
                .w_axis
                .z
                .min(-depth - part.spec.size.z * 0.5 + 0.006);
        }
    }
    let torso_height = torso_spec.size.y;
    let head_bottom =
        recipe.rig.joints[JointId::Head.index()].rest.translation.y - recipe.head.size.y * 0.5;
    let lift = (torso_height * 0.5 + 0.04 - head_bottom).max(0.0);
    for part in parts.iter_mut().filter(|p| p.anchor.joint == JointId::Head) {
        part.anchor.local = Mat4::from_translation(Vec3::Y * lift) * part.anchor.local;
    }
}

fn apply_outfit(vertices: &mut Vec<Part>, recipe: &BodyRecipe, outfit: OutfitId) {
    let scale_joint = |vertices: &mut Vec<Part>, joint: JointId, scale: Vec3| {
        for part in vertices.iter_mut().filter(|part| {
            part.anchor.joint == joint
                && matches!(part.tint, Tint::Shirt | Tint::Pants | Tint::Shoes)
        }) {
            part.spec.size *= scale;
        }
    };
    let add_box =
        |vertices: &mut Vec<Part>, anchor: Anchor, size: Vec3, radius: f32, tint: Tint| {
            vertices.push(Part {
                anchor,
                spec: BodyPart::new(size, radius),
                tint,
                feature: Feature::None,
                shape: super::hero_geometry::Shape::Rounded,
            });
        };
    let torso = Anchor::new(JointId::Torso);
    match outfit {
        OutfitId::EverydayHoodie => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.10, 1.08, 1.04));
            scale_joint(vertices, JointId::LeftUpperArm, Vec3::new(1.12, 1.08, 1.08));
            scale_joint(
                vertices,
                JointId::RightUpperArm,
                Vec3::new(1.12, 1.08, 1.08),
            );
            scale_joint(vertices, JointId::LeftLowerArm, Vec3::new(1.14, 1.06, 1.10));
            scale_joint(
                vertices,
                JointId::RightLowerArm,
                Vec3::new(1.14, 1.06, 1.10),
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, -0.22, -0.39)),
                Vec3::new(0.48, 0.22, 0.035),
                0.02,
                Tint::Detail,
            );
        }
        OutfitId::PufferExplorer => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.23, 1.12, 1.16));
            scale_joint(vertices, JointId::LeftUpperArm, Vec3::new(1.24, 1.08, 1.20));
            scale_joint(
                vertices,
                JointId::RightUpperArm,
                Vec3::new(1.24, 1.08, 1.20),
            );
            scale_joint(vertices, JointId::LeftLowerArm, Vec3::new(1.17, 1.06, 1.12));
            scale_joint(
                vertices,
                JointId::RightLowerArm,
                Vec3::new(1.17, 1.06, 1.12),
            );
            scale_joint(vertices, JointId::LeftFoot, Vec3::new(1.20, 1.16, 1.22));
            scale_joint(vertices, JointId::RightFoot, Vec3::new(1.20, 1.16, 1.22));
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.48, -0.02)),
                Vec3::new(0.82, 0.18, 0.86),
                0.08,
                Tint::Outer,
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.43)),
                Vec3::new(0.06, 0.66, 0.035),
                0.015,
                Tint::Detail,
            );
        }
        OutfitId::GlossyRaincoat => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.20, 1.22, 1.08));
            scale_joint(vertices, JointId::LeftUpperArm, Vec3::new(1.14, 1.12, 1.08));
            scale_joint(
                vertices,
                JointId::RightUpperArm,
                Vec3::new(1.14, 1.12, 1.08),
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, -0.35, 0.0)),
                Vec3::new(1.26, 0.20, 0.86),
                0.09,
                Tint::Outer,
            );
            add_box(
                vertices,
                Anchor::new(JointId::Head) * Mat4::from_translation(Vec3::new(0.0, 0.38, 0.11)),
                Vec3::new(1.18, 0.18, 0.94),
                0.10,
                Tint::Outer,
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.42, -0.42)),
                Vec3::new(0.72, 0.045, 0.035),
                0.01,
                Tint::Detail,
            );
        }
        OutfitId::StarWizard => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.34, 1.28, 1.08));
            scale_joint(vertices, JointId::LeftLowerLeg, Vec3::new(1.14, 1.05, 1.10));
            scale_joint(
                vertices,
                JointId::RightLowerLeg,
                Vec3::new(1.14, 1.05, 1.10),
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, -0.42, 0.03)),
                Vec3::new(1.46, 0.26, 0.96),
                0.12,
                Tint::Outer,
            );
            add_box(
                vertices,
                Anchor::new(JointId::Head)
                    * Mat4::from_translation(Vec3::new(0.0, 0.88, 0.04))
                    * Mat4::from_rotation_z(0.12),
                Vec3::new(0.55, 0.92, 0.55),
                0.16,
                Tint::Outer,
            );
            vertices.last_mut().unwrap().spec.taper = (1.20, 0.20);
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.18, -0.48)),
                Vec3::new(0.18, 0.18, 0.035),
                0.01,
                Tint::Detail,
            );
        }
        OutfitId::ToyKnight => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.18, 1.10, 1.12));
            scale_joint(vertices, JointId::LeftFoot, Vec3::new(1.20, 1.18, 1.16));
            scale_joint(vertices, JointId::RightFoot, Vec3::new(1.20, 1.18, 1.16));
            add_box(
                vertices,
                Anchor::new(JointId::LeftUpperArm)
                    * Mat4::from_translation(Vec3::new(0.0, 0.10, 0.0)),
                Vec3::new(0.52, 0.34, 0.54),
                0.10,
                Tint::Armor,
            );
            add_box(
                vertices,
                Anchor::new(JointId::RightUpperArm)
                    * Mat4::from_translation(Vec3::new(0.0, 0.10, 0.0)),
                Vec3::new(0.52, 0.34, 0.54),
                0.10,
                Tint::Armor,
            );
            add_box(
                vertices,
                Anchor::new(JointId::LeftLowerArm)
                    * Mat4::from_translation(Vec3::new(0.0, -0.02, 0.0)),
                Vec3::new(0.42, 0.48, 0.46),
                0.08,
                Tint::Armor,
            );
            add_box(
                vertices,
                Anchor::new(JointId::RightLowerArm)
                    * Mat4::from_translation(Vec3::new(0.0, -0.02, 0.0)),
                Vec3::new(0.42, 0.48, 0.46),
                0.08,
                Tint::Armor,
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.43)),
                Vec3::new(0.56, 0.48, 0.045),
                0.015,
                Tint::Detail,
            );
        }
        OutfitId::FuzzyPajamas => {
            scale_joint(vertices, JointId::Torso, Vec3::new(1.10, 1.08, 1.04));
            scale_joint(vertices, JointId::LeftUpperLeg, Vec3::new(1.16, 1.10, 1.12));
            scale_joint(
                vertices,
                JointId::RightUpperLeg,
                Vec3::new(1.16, 1.10, 1.12),
            );
            scale_joint(vertices, JointId::LeftLowerLeg, Vec3::new(1.16, 1.08, 1.10));
            scale_joint(
                vertices,
                JointId::RightLowerLeg,
                Vec3::new(1.16, 1.08, 1.10),
            );
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, -0.30, 0.0)),
                Vec3::new(1.10, 0.15, 0.78),
                0.07,
                Tint::Fuzz,
            );
            add_box(
                vertices,
                Anchor::new(JointId::LeftFoot)
                    * Mat4::from_translation(Vec3::new(0.0, -0.04, -0.05)),
                Vec3::new(0.72, 0.20, 0.92),
                0.08,
                Tint::Fuzz,
            );
            add_box(
                vertices,
                Anchor::new(JointId::RightFoot)
                    * Mat4::from_translation(Vec3::new(0.0, -0.04, -0.05)),
                Vec3::new(0.72, 0.20, 0.92),
                0.08,
                Tint::Fuzz,
            );
        }
    }
    let _ = recipe;
}

fn add_part(vertices: &mut Vec<Part>, anchor: Anchor, spec: BodyPart, tint: Tint) {
    vertices.push(Part {
        anchor,
        spec,
        tint,
        feature: Feature::None,
        shape: super::hero_geometry::Shape::Rounded,
    });
}

fn add_face(
    vertices: &mut Vec<Part>,
    head: Anchor,
    recipe: &BodyRecipe,
    parameters: FaceParameters,
    face_color: Tint,
) {
    let parameters = parameters.clamped();
    let anchors = recipe.face;
    let eye_y = anchors.eye_y + parameters.look.y;
    for side in [-1.0, 1.0] {
        let person_eye_wrap = if recipe.id.is_person() { 0.38 } else { 0.0 };
        let person_eye_spread = if recipe.id.is_person() { 0.035 } else { 0.0 };
        let eye = Mat4::from_translation(Vec3::new(
            side * (anchors.eye_x + person_eye_spread) + parameters.look.x,
            eye_y,
            anchors.face_z + person_eye_wrap * 0.035,
        )) * Mat4::from_rotation_y(-side * person_eye_wrap)
            * Mat4::from_quat(Quat::from_rotation_z(side * anchors.eye_tilt));
        let part_start = vertices.len();
        add_part(
            vertices,
            head * eye,
            // Graphic face pieces keep a tiny hard planar region so they do
            // not collapse when the shallow depth is below the fillet limit.
            BodyPart::new(
                Vec3::new(
                    anchors.eye_size.x,
                    anchors.eye_size.y * parameters.eye_opening,
                    0.025,
                ),
                0.0,
            ),
            face_color,
        );
        vertices[part_start].feature = Feature::Eye(side);
        let brow = Mat4::from_translation(Vec3::new(
            side * anchors.eye_x,
            anchors.brow_y,
            anchors.face_z - 0.012,
        )) * Mat4::from_quat(Quat::from_rotation_z(side * parameters.brow_tilt));
        let part_start = vertices.len();
        add_part(
            vertices,
            head * brow,
            BodyPart::new(Vec3::new(anchors.brow_width, 0.045, 0.035), 0.0),
            face_color,
        );
        vertices[part_start].feature = Feature::Brow(side);
    }

    let mouth_width = anchors.mouth_width + parameters.mouth_opening * 0.06;
    let mouth = Mat4::from_translation(Vec3::new(
        0.0,
        anchors.mouth_y + parameters.mouth_curve * 0.025,
        recipe
            .extras
            .muzzle_size
            .map_or(anchors.face_z - 0.018, |size| -0.44 - size.z * 0.5 - 0.018),
    )) * Mat4::from_quat(Quat::from_rotation_z(parameters.mouth_curve * 0.18));
    let part_start = vertices.len();
    add_part(
        vertices,
        head * mouth,
        BodyPart::new(Vec3::new(mouth_width, 0.18, 0.018), 0.0),
        face_color,
    );
    vertices[part_start].feature = Feature::Mouth;

    // A small blush mark gives the toy faces a readable emotional accent at
    // gameplay distance. Animal faces use their muzzle color and markings;
    // the person gets the small blush accent without overloading the part
    // budget for tails, ears, and clothing.
    if recipe.id.is_person() {
        let cheek_z = recipe
            .extras
            .muzzle_size
            .map_or(anchors.face_z - 0.014, |size| -0.44 - size.z * 0.5 - 0.022);
        for side in [-1.0, 1.0] {
            let part_start = vertices.len();
            add_part(
                vertices,
                head * Mat4::from_translation(Vec3::new(
                    side * (anchors.eye_x + 0.10),
                    anchors.mouth_y + 0.015,
                    cheek_z,
                )),
                BodyPart::new(Vec3::new(0.105, 0.055, 0.018), 0.0),
                Tint::Blush,
            );
            vertices[part_start].feature = Feature::Cheek(side);
        }

        // Continue each end of the smile onto the cheek. The pieces overlap
        // the main mouth at the front, so they read as one wrapped expression
        // rather than detached profile marks.
        let mouth_edge_x = anchors.mouth_width * 0.45;
        let mouth_edge_z = anchors.face_z + 0.010;
        let mouth_edge_yaw = 0.72;
        for side in [-1.0, 1.0] {
            let part_start = vertices.len();
            add_part(
                vertices,
                head * Mat4::from_translation(Vec3::new(
                    side * mouth_edge_x,
                    anchors.mouth_y,
                    mouth_edge_z,
                )) * Mat4::from_rotation_y(-side * mouth_edge_yaw),
                BodyPart::new(Vec3::new(anchors.mouth_width * 0.38, 0.090, 0.020), 0.0),
                face_color,
            );
            vertices[part_start].feature = Feature::MouthEdge(side);
        }
    }
}

fn add_species_parts(vertices: &mut Vec<Part>, root: Anchor, head: Anchor, recipe: &BodyRecipe) {
    let ink = Tint::Face;
    if recipe.id.is_person() {
        // A sculpted cap and asymmetric swept fringe retain the cube head.
        detail(
            vertices,
            head,
            Vec3::new(0.0, 0.38, 0.04),
            Vec3::new(1.08, 0.26, 0.85),
            Tint::Hair,
        );
        detail(
            vertices,
            head * Mat4::from_rotation_z(-0.16),
            Vec3::new(-0.19, 0.39, -0.37),
            Vec3::new(0.67, 0.20, 0.18),
            Tint::Hair,
        );
        for side in [-1.0, 1.0] {
            detail(
                vertices,
                head,
                Vec3::new(side * 0.51, -0.02, 0.0),
                Vec3::new(0.16, 0.25, 0.22),
                Tint::Skin,
            );
        }
    }
    if let Some(ear_size) = recipe.extras.ear_size {
        let ear_x = recipe.head.size.x * 0.36;
        for side in [-1.0, 1.0] {
            let ear = Mat4::from_translation(Vec3::new(side * ear_x, 0.46, 0.01))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            let part_start = vertices.len();
            add_part(
                vertices,
                head * ear,
                BodyPart::new(ear_size, 0.07),
                Tint::Skin,
            );
            vertices[part_start].feature = Feature::Ear(side);
            vertices[part_start].spec.taper = (1.0, 0.20);
            let inner = Mat4::from_translation(Vec3::new(side * ear_x, 0.47, -0.145))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            add_part(
                vertices,
                head * inner,
                BodyPart::new(Vec3::new(0.13, 0.22, 0.025), 0.0),
                Tint::InnerEar,
            );
            vertices.last_mut().unwrap().feature = Feature::Ear(side);
            vertices.last_mut().unwrap().spec.taper = (1.0, 0.20);
        }
    }
    if let Some(muzzle_size) = recipe.extras.muzzle_size {
        add_part(
            vertices,
            head * Mat4::from_translation(Vec3::new(0.0, recipe.face.muzzle_y, -0.44)),
            BodyPart::new(muzzle_size, 0.09),
            Tint::Muzzle,
        );
        add_part(
            vertices,
            head * Mat4::from_translation(Vec3::new(
                0.0,
                recipe.face.muzzle_y + 0.01,
                -0.44 - muzzle_size.z * 0.5 - 0.022,
            )),
            BodyPart::new(Vec3::new(0.11, 0.07, 0.045), 0.0),
            ink,
        );
    }
    if recipe.extras.horns {
        // Short swept horns and a center crest make the dragon read in
        // profile even when its wings are hidden behind another character.
        for side in [-1.0, 1.0] {
            let horn = Mat4::from_translation(Vec3::new(side * 0.30, 0.50, 0.05))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22))
                * Mat4::from_quat(Quat::from_rotation_y(side * 0.16));
            add_part(
                vertices,
                head * horn,
                BodyPart::new(Vec3::new(0.18, 0.42, 0.18), 0.06),
                Tint::Ivory,
            );
            vertices.last_mut().unwrap().spec.taper = (1.0, 0.35);
        }
        detail(
            vertices,
            head,
            Vec3::new(0.0, 0.49, 0.18),
            Vec3::new(0.20, 0.32, 0.17),
            Tint::Skin,
        );
    }
    if recipe.extras.wings {
        for side in [-1.0, 1.0] {
            let wing = Mat4::from_translation(Vec3::new(side * 0.82, 1.90, 0.35))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.38));
            let part_start = vertices.len();
            add_part(
                vertices,
                root * wing,
                BodyPart::new(Vec3::new(0.64, 1.00, 0.18), 0.08),
                Tint::Skin,
            );
            vertices[part_start].feature = Feature::Wing(side);
            vertices[part_start].spec.taper = (1.0, 0.35);
        }
    }
    if recipe.extras.tail_segments > 0 {
        let count = recipe.extras.tail_segments as usize;
        for index in 0..count {
            let progress = index as f32 / count as f32;
            let x = if recipe.id == BodyId::Cat {
                0.52 + progress * 0.42
            } else {
                0.18 + progress * 0.30
            };
            let y = 1.02
                + progress
                    * if recipe.id == BodyId::Cat {
                        0.48
                    } else {
                        -0.26
                    };
            let z = 0.40
                + progress
                    * if recipe.id == BodyId::Cat {
                        0.58
                    } else {
                        0.86
                    };
            let tail = Mat4::from_translation(Vec3::new(x, y, z))
                * Mat4::from_quat(Quat::from_rotation_x(-0.22 + progress * 0.25));
            let size = if recipe.id == BodyId::Cat {
                Vec3::new(0.25 - progress * 0.06, 0.32, 0.27 - progress * 0.05)
            } else {
                Vec3::new(0.34 - progress * 0.13, 0.36, 0.42 - progress * 0.15)
            };
            let part_start = vertices.len();
            add_part(vertices, root * tail, BodyPart::new(size, 0.07), Tint::Skin);
            vertices[part_start].feature = Feature::Tail(progress);
        }
    }
}

fn add_seam_cores(vertices: &mut Vec<Part>, recipe: &BodyRecipe) {
    let seam = |vertices: &mut Vec<Part>, anchor: Anchor, position: Vec3, size: f32, phase: f32| {
        let part_start = vertices.len();
        add_part(
            vertices,
            anchor * Mat4::from_translation(position),
            BodyPart::new(Vec3::splat(size), size * 0.28),
            Tint::Seam,
        );
        vertices[part_start].feature = Feature::Seam(phase);
    };
    seam(
        vertices,
        Anchor::new(JointId::Head),
        Vec3::new(0.0, -recipe.head.size.y * 0.5 - 0.018, 0.0),
        0.075,
        0.0,
    );
    seam(
        vertices,
        Anchor::new(JointId::Torso),
        Vec3::new(0.0, -recipe.torso.size.y * 0.5 - 0.018, 0.0),
        0.064,
        0.35,
    );
    for (side, joint) in [(-1.0, JointId::LeftUpperArm), (1.0, JointId::RightUpperArm)] {
        seam(vertices, Anchor::new(joint), Vec3::ZERO, 0.058, side);
    }
    // Brief event sparks emerge beside the shoulder seams, clear of shoes
    // and heavy garments. The renderer omits them outside the burst lifetime.
    for (side, position) in [
        (-1.0, Vec3::new(-0.74, 1.90, -0.48)),
        (1.0, Vec3::new(0.74, 1.96, -0.48)),
    ] {
        let part_start = vertices.len();
        add_part(
            vertices,
            Anchor::new(JointId::Root) * Mat4::from_translation(position),
            BodyPart::new(Vec3::new(0.055, 0.13, 0.055), 0.018),
            Tint::Seam,
        );
        vertices[part_start].feature = Feature::Spark(side);
    }
}

/// Temporary CPU expansion exists only for the before-image capture tool.
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(super) fn add_character(
    vertices: &mut Vec<super::Vertex>,
    entity: super::RenderEntity,
    body: BodyId,
    style: AvatarStyle,
    face_color: [f32; 4],
    cache: &mut super::rounded_geometry::RoundedMeshCache,
) {
    add_character_with_outfit(
        vertices,
        entity,
        body,
        OutfitId::EverydayHoodie,
        style,
        face_color,
        cache,
    );
}

#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
pub(super) fn add_character_with_outfit(
    vertices: &mut Vec<super::Vertex>,
    entity: super::RenderEntity,
    body: BodyId,
    outfit: OutfitId,
    style: AvatarStyle,
    face_color: [f32; 4],
    cache: &mut super::rounded_geometry::RoundedMeshCache,
) {
    let recipe = body_recipe(body);
    let pose = Pose::locomotion(
        &recipe.rig,
        entity.walk_cycle,
        entity.moving,
        entity.sprinting,
    );
    let joints = recipe.rig.world_matrices(&pose.transforms);
    let root = Mat4::from_rotation_translation(
        Quat::from_rotation_y(entity.yaw),
        Vec3::from_array(entity.position),
    );
    for part in parts_for(&recipe, outfit) {
        let transform = root * joints[part.anchor.joint.index()] * part.anchor.local;
        let mesh = cache
            .get_or_build(mesh_recipe(part.spec))
            .expect("bundled body part");
        let normal = transform.inverse().transpose();
        for &index in &mesh.indices {
            let v = mesh.vertices[index as usize];
            vertices.push(super::Vertex {
                position: transform.transform_point3(v.position).to_array(),
                normal: normal
                    .transform_vector3(v.normal)
                    .normalize_or_zero()
                    .to_array(),
                color: part.tint.color(style, face_color),
                tex_coords: v.uv,
                image_invert: 0.0,
            });
        }
    }
}
#[allow(dead_code)]
pub(super) fn mesh_recipe(spec: BodyPart) -> super::rounded_geometry::RoundedBoxRecipe {
    mesh_recipe_with_subdivisions(spec, 2)
}

pub(super) fn mesh_recipe_with_subdivisions(
    spec: BodyPart,
    subdivisions: u32,
) -> super::rounded_geometry::RoundedBoxRecipe {
    super::rounded_geometry::RoundedBoxRecipe::new(
        spec.size,
        spec.radius,
        subdivisions,
        super::rounded_geometry::TaperProfile {
            bottom: spec.taper.0,
            top: spec.taper.1,
        },
    )
}

/// Conservative rest-pose sphere for frustum culling. It is derived from
/// every authored part and padded for secondary motion, not from gameplay
/// collision dimensions.
pub(super) fn bounds(body: BodyId, outfit: OutfitId) -> (Vec3, f32) {
    let recipe = body_recipe(body);
    let mut pose = Pose::rest(&recipe.rig);
    if body.is_person() && outfit == OutfitId::EverydayHoodie {
        pose = super::hero_character::fit_pose(
            super::RenderEntity {
                body,
                outfit,
                pose,
                ..Default::default()
            },
            super::hero_character::Study::Everyday,
            &recipe.rig,
        );
    }
    let joints = recipe.rig.world_matrices(&pose.transforms);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in parts_for(&recipe, outfit) {
        let transform = joints[part.anchor.joint.index()] * part.anchor.local;
        let taper = part.spec.taper.0.max(part.spec.taper.1).max(1.0);
        let half = part.spec.size * Vec3::new(taper, 1.0, taper) * 0.5;
        let (local_min, local_max) =
            if let super::hero_geometry::Shape::HairCurve(style, index) = part.shape {
                super::hair_geometry::bounds(style, index)
            } else {
                (-half, half)
            };
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let corner = transform.transform_point3(Vec3::new(
                        if x < 0.0 { local_min.x } else { local_max.x },
                        if y < 0.0 { local_min.y } else { local_max.y },
                        if z < 0.0 { local_min.z } else { local_max.z },
                    ));
                    min = min.min(corner);
                    max = max.max(corner);
                }
            }
        }
    }
    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length() + 0.42;
    (center, radius.max(0.5))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::hero_geometry::Shape;

    #[test]
    fn camera_anchors_are_body_defined() {
        assert_ne!(camera_anchor(BodyId::Person), camera_target(BodyId::Dragon));
        assert!(camera_anchor(BodyId::Cat).y > camera_target(BodyId::Cat).y);
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
