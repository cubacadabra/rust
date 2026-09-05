//! Immutable authored rigid pieces; routine animation changes transforms.
use super::AvatarStyle;
use super::character_material::Material;
#[cfg(all(feature = "dev-showcase", not(target_arch = "wasm32")))]
use crate::character::Pose;
use crate::character::{
    BodyId, BodyPart, BodyRecipe, FaceParameters, FacePreset, JointId, OutfitId, body_recipe,
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
#[derive(Clone, Copy)]
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
}
impl Tint {
    pub fn color(self, style: AvatarStyle, face: [f32; 4]) -> [f32; 4] {
        match self {
            Self::Skin => style.skin,
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
            Self::Skin | Self::Detail => Material::Toy,
            Self::Shirt => Material::Cloth,
            Self::Pants => Material::Denim,
            Self::Shoes => Material::Rubber,
            Self::Face => Material::Face,
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
}
#[derive(Clone, Copy)]
pub(super) enum Feature {
    None,
    Eye(f32),
    Brow(f32),
    Mouth,
    Ear(f32),
    Tail(f32),
    Wing(f32),
}
fn camera_anchors(body: BodyId) -> (Vec3, Vec3) {
    static ANCHORS: std::sync::OnceLock<[(Vec3, Vec3); 3]> = std::sync::OnceLock::new();
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
    std::mem::take(vertices)
}

pub(super) fn parts_for(recipe: &BodyRecipe, outfit: OutfitId) -> Vec<Part> {
    let mut vertices = base_parts(recipe);
    // Keeping this as a finite authored catalog prevents arbitrary runtime
    // geometry keys from entering the GPU cache.
    apply_outfit(&mut vertices, recipe, outfit);
    vertices
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
            add_box(
                vertices,
                torso * Mat4::from_translation(Vec3::new(0.0, 0.39, -0.12)),
                Vec3::new(0.08, 0.24, 0.035),
                0.015,
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
                Anchor::new(JointId::Head) * Mat4::from_translation(Vec3::new(0.0, 0.60, 0.04)),
                Vec3::new(0.55, 0.92, 0.55),
                0.16,
                Tint::Outer,
            );
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
        let eye = Mat4::from_translation(Vec3::new(
            side * anchors.eye_x + parameters.look.x,
            eye_y,
            anchors.face_z,
        ));
        let part_start = vertices.len();
        add_part(
            vertices,
            head * eye,
            // Graphic face pieces keep a tiny hard planar region so they do
            // not collapse when the shallow depth is below the fillet limit.
            BodyPart::new(Vec3::new(0.15, 0.21 * parameters.eye_opening, 0.055), 0.0),
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
            BodyPart::new(Vec3::new(0.21, 0.045, 0.035), 0.0),
            face_color,
        );
        vertices[part_start].feature = Feature::Brow(side);
    }

    let mouth_width = 0.24 + parameters.mouth_opening * 0.06;
    let mouth = Mat4::from_translation(Vec3::new(
        0.0,
        anchors.mouth_y + parameters.mouth_curve * 0.025,
        anchors.face_z - 0.018,
    )) * Mat4::from_quat(Quat::from_rotation_z(parameters.mouth_curve * 0.18));
    let part_start = vertices.len();
    add_part(
        vertices,
        head * mouth,
        BodyPart::new(
            Vec3::new(mouth_width, 0.045 + parameters.mouth_opening * 0.09, 0.025),
            0.0,
        ),
        face_color,
    );
    vertices[part_start].feature = Feature::Mouth;
}

fn add_species_parts(vertices: &mut Vec<Part>, root: Anchor, head: Anchor, recipe: &BodyRecipe) {
    let ink = Tint::Detail;
    if let Some(ear_size) = recipe.extras.ear_size {
        for side in [-1.0, 1.0] {
            let ear = Mat4::from_translation(Vec3::new(side * 0.30, 0.46, 0.01))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            let part_start = vertices.len();
            add_part(
                vertices,
                head * ear,
                BodyPart::new(ear_size, 0.07),
                Tint::Skin,
            );
            vertices[part_start].feature = Feature::Ear(side);
            let inner = Mat4::from_translation(Vec3::new(side * 0.30, 0.47, -0.145))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            add_part(
                vertices,
                head * inner,
                BodyPart::new(Vec3::new(0.13, 0.22, 0.025), 0.0),
                ink,
            );
        }
    }
    if let Some(muzzle_size) = recipe.extras.muzzle_size {
        add_part(
            vertices,
            head * Mat4::from_translation(Vec3::new(0.0, recipe.face.muzzle_y, -0.44)),
            BodyPart::new(muzzle_size, 0.09),
            Tint::Skin,
        );
        add_part(
            vertices,
            head * Mat4::from_translation(Vec3::new(0.0, recipe.face.muzzle_y + 0.01, -0.60)),
            BodyPart::new(Vec3::new(0.11, 0.07, 0.045), 0.0),
            ink,
        );
    }
    if recipe.extras.horns {
        for side in [-1.0, 1.0] {
            let horn = Mat4::from_translation(Vec3::new(side * 0.25, 0.48, 0.05))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.18));
            add_part(
                vertices,
                head * horn,
                BodyPart::new(Vec3::new(0.16, 0.36, 0.16), 0.055),
                Tint::Pants,
            );
        }
    }
    if recipe.extras.wings {
        for side in [-1.0, 1.0] {
            let wing = Mat4::from_translation(Vec3::new(side * 0.56, 1.72, 0.30))
                * Mat4::from_quat(Quat::from_rotation_z(side * 0.18));
            let part_start = vertices.len();
            add_part(
                vertices,
                root * wing,
                BodyPart::new(Vec3::new(0.16, 0.72, 0.42), 0.07),
                Tint::Shirt,
            );
            vertices[part_start].feature = Feature::Wing(side);
        }
    }
    if recipe.extras.tail_segments > 0 {
        let count = recipe.extras.tail_segments as usize;
        for index in 0..count {
            let progress = index as f32 / count as f32;
            let x = if recipe.id == BodyId::Cat {
                0.34 + progress * 0.18
            } else {
                0.0
            };
            let y = 0.98 + progress * 0.13;
            let z = 0.34 + progress * 0.43;
            let tail = Mat4::from_translation(Vec3::new(x, y, z))
                * Mat4::from_quat(Quat::from_rotation_x(-0.22 + progress * 0.25));
            let size = if recipe.id == BodyId::Cat {
                Vec3::new(0.25 - progress * 0.06, 0.32, 0.27 - progress * 0.05)
            } else {
                Vec3::new(0.34 - progress * 0.13, 0.36, 0.42 - progress * 0.15)
            };
            let part_start = vertices.len();
            add_part(
                vertices,
                root * tail,
                BodyPart::new(size, 0.07),
                Tint::Pants,
            );
            vertices[part_start].feature = Feature::Tail(progress);
        }
    }
}

fn add_seam_cores(vertices: &mut Vec<Part>, recipe: &BodyRecipe) {
    add_part(
        vertices,
        Anchor::new(JointId::Head)
            * Mat4::from_translation(Vec3::new(0.0, -recipe.head.size.y * 0.5 - 0.018, 0.0)),
        BodyPart::new(Vec3::splat(0.075), 0.075 * 0.28),
        Tint::Seam,
    );
    for joint in [
        JointId::LeftHand,
        JointId::RightHand,
        JointId::LeftFoot,
        JointId::RightFoot,
    ] {
        let size = if matches!(joint, JointId::LeftFoot | JointId::RightFoot) {
            0.065
        } else {
            0.055
        };
        add_part(
            vertices,
            Anchor::new(joint),
            BodyPart::new(Vec3::splat(size), size * 0.28),
            Tint::Seam,
        );
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
pub(super) fn mesh_recipe(spec: BodyPart) -> super::rounded_geometry::RoundedBoxRecipe {
    super::rounded_geometry::RoundedBoxRecipe::new(
        spec.size,
        spec.radius,
        2,
        super::rounded_geometry::TaperProfile {
            bottom: spec.taper.0,
            top: spec.taper.1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_anchors_are_body_defined() {
        assert_ne!(camera_anchor(BodyId::Person), camera_target(BodyId::Dragon));
        assert!(camera_anchor(BodyId::Cat).y > camera_target(BodyId::Cat).y);
    }

    #[test]
    fn compiled_parts_preserve_face_hand_and_foot_anchors() {
        for body in BodyId::ALL {
            let parts = parts_for(&body_recipe(body), OutfitId::EverydayHoodie);
            assert!(parts.len() <= 48);
            assert_eq!(
                parts
                    .iter()
                    .filter(|p| matches!(p.tint, Tint::Face))
                    .count(),
                5
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
        assert_ne!(hoodie.len(), wizard.len());
        assert_ne!(wizard.len(), knight.len());
        assert!(wizard.iter().any(|part| matches!(part.tint, Tint::Outer)));
        assert!(knight.iter().any(|part| matches!(part.tint, Tint::Armor)));
    }
}
