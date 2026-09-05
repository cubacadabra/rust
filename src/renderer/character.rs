//! Phase 2 shape-proof assembly.  Parts are rigid, parented pieces; the mesh
//! recipes are immutable and the pose only changes transforms.

use glam::{Mat4, Quat, Vec3};

use crate::character::{
    BodyId, BodyPart, BodyRecipe, FaceParameters, FacePreset, JointId, Pose, body_recipe,
};

use super::{AvatarStyle, RenderEntity, Vertex, rounded_geometry};

const SEAM_COLOR: [f32; 4] = [0.28, 0.95, 0.87, 0.92];

pub(super) fn camera_anchor(body: BodyId) -> Vec3 {
    body_recipe(body).first_person_anchor
}

pub(super) fn camera_target(body: BodyId) -> Vec3 {
    body_recipe(body).third_person_target
}

pub(super) fn add_character(
    vertices: &mut Vec<Vertex>,
    entity: RenderEntity,
    body: BodyId,
    style: AvatarStyle,
    face_color: [f32; 4],
    rounded_mesh_cache: &mut rounded_geometry::RoundedMeshCache,
) {
    let recipe = body_recipe(body);
    let pose = Pose::locomotion(
        &recipe.rig,
        entity.walk_cycle,
        entity.moving,
        entity.sprinting,
    );
    let joint_matrices = recipe.rig.world_matrices(&pose.transforms);
    let root = Mat4::from_translation(Vec3::from_array(entity.position))
        * Mat4::from_quat(Quat::from_rotation_y(entity.yaw));
    let mut part = |vertices: &mut Vec<Vertex>, joint: JointId, spec: BodyPart, color: [f32; 4]| {
        add_part(
            vertices,
            rounded_mesh_cache,
            root * joint_matrices[joint.index()],
            spec,
            color,
        );
    };

    part(vertices, JointId::Torso, recipe.torso, style.shirt);
    part(vertices, JointId::Head, recipe.head, style.skin);
    part(vertices, JointId::LeftUpperArm, recipe.upper_arm, style.shirt);
    part(vertices, JointId::LeftLowerArm, recipe.lower_arm, style.shirt);
    part(vertices, JointId::LeftHand, recipe.hand, style.skin);
    part(vertices, JointId::RightUpperArm, recipe.upper_arm, style.shirt);
    part(vertices, JointId::RightLowerArm, recipe.lower_arm, style.shirt);
    part(vertices, JointId::RightHand, recipe.hand, style.skin);
    part(vertices, JointId::LeftUpperLeg, recipe.upper_leg, style.pants);
    part(vertices, JointId::LeftLowerLeg, recipe.lower_leg, style.pants);
    part(vertices, JointId::RightUpperLeg, recipe.upper_leg, style.pants);
    part(vertices, JointId::RightLowerLeg, recipe.lower_leg, style.pants);
    part(vertices, JointId::LeftFoot, recipe.foot, style.shoes);
    part(vertices, JointId::RightFoot, recipe.foot, style.shoes);

    let face = match body {
        BodyId::Person => FaceParameters::preset(FacePreset::Happy),
        BodyId::Cat => FaceParameters::preset(FacePreset::Happy),
        BodyId::Dragon => FaceParameters::preset(FacePreset::Determined),
    };
    add_face(
        vertices,
        rounded_mesh_cache,
        root * joint_matrices[JointId::Head.index()],
        &recipe,
        face,
        face_color,
    );
    add_species_parts(
        vertices,
        rounded_mesh_cache,
        root,
        root * joint_matrices[JointId::Head.index()],
        &recipe,
        style,
    );
    add_seam_cores(vertices, rounded_mesh_cache, root, &recipe, &joint_matrices);
}

fn add_part(
    vertices: &mut Vec<Vertex>,
    cache: &mut rounded_geometry::RoundedMeshCache,
    transform: Mat4,
    spec: BodyPart,
    color: [f32; 4],
) {
    let recipe = rounded_geometry::RoundedBoxRecipe::new(
        spec.size,
        spec.radius,
        2,
        rounded_geometry::TaperProfile {
            bottom: spec.taper.0,
            top: spec.taper.1,
        },
    );
    let Ok(mesh) = cache.get_or_build(recipe) else {
        super::add_transformed_cuboid(vertices, transform, spec.size, color);
        return;
    };
    let normal_transform = transform.inverse().transpose();
    for &index in &mesh.indices {
        let source = mesh.vertices[index as usize];
        let position = transform.transform_point3(source.position);
        let normal = normal_transform
            .transform_vector3(source.normal)
            .normalize_or_zero();
        vertices.push(Vertex {
            position: position.to_array(),
            normal: normal.to_array(),
            color,
            tex_coords: source.uv,
            image_invert: 0.0,
        });
    }
}

fn add_face(
    vertices: &mut Vec<Vertex>,
    cache: &mut rounded_geometry::RoundedMeshCache,
    head: Mat4,
    recipe: &BodyRecipe,
    parameters: FaceParameters,
    face_color: [f32; 4],
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
        add_part(
            vertices,
            cache,
            head * eye,
            // Graphic face pieces keep a tiny hard planar region so they do
            // not collapse when the shallow depth is below the fillet limit.
            BodyPart::new(Vec3::new(0.15, 0.21 * parameters.eye_opening, 0.055), 0.0),
            face_color,
        );
        let brow = Mat4::from_translation(Vec3::new(
            side * anchors.eye_x,
            anchors.brow_y,
            anchors.face_z - 0.012,
        )) * Mat4::from_quat(Quat::from_rotation_z(side * parameters.brow_tilt));
        add_part(
            vertices,
            cache,
            head * brow,
            BodyPart::new(Vec3::new(0.21, 0.045, 0.035), 0.0),
            face_color,
        );
    }

    let mouth_width = 0.24 + parameters.mouth_opening * 0.06;
    let mouth = Mat4::from_translation(Vec3::new(
        0.0,
        anchors.mouth_y + parameters.mouth_curve * 0.025,
        anchors.face_z - 0.018,
    )) * Mat4::from_quat(Quat::from_rotation_z(parameters.mouth_curve * 0.18));
    add_part(
        vertices,
        cache,
        head * mouth,
        BodyPart::new(
            Vec3::new(mouth_width, 0.045 + parameters.mouth_opening * 0.09, 0.025),
            0.0,
        ),
        face_color,
    );
}

fn add_species_parts(
    vertices: &mut Vec<Vertex>,
    cache: &mut rounded_geometry::RoundedMeshCache,
    root: Mat4,
    head: Mat4,
    recipe: &BodyRecipe,
    style: AvatarStyle,
) {
    let ink = [style.shirt[0] * 0.55, style.shirt[1] * 0.55, style.shirt[2] * 0.55, 1.0];
    if let Some(ear_size) = recipe.extras.ear_size {
        for side in [-1.0, 1.0] {
            let ear = Mat4::from_translation(Vec3::new(side * 0.30, 0.46, 0.01))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            add_part(vertices, cache, head * ear, BodyPart::new(ear_size, 0.07), style.skin);
            let inner = Mat4::from_translation(Vec3::new(side * 0.30, 0.47, -0.145))
                * Mat4::from_quat(Quat::from_rotation_z(-side * 0.22));
            add_part(
                vertices,
                cache,
                head * inner,
                BodyPart::new(Vec3::new(0.13, 0.22, 0.025), 0.0),
                ink,
            );
        }
    }
    if let Some(muzzle_size) = recipe.extras.muzzle_size {
        add_part(
            vertices,
            cache,
            head * Mat4::from_translation(Vec3::new(0.0, recipe.face.muzzle_y, -0.44)),
            BodyPart::new(muzzle_size, 0.09),
            style.skin,
        );
        add_part(
            vertices,
            cache,
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
                cache,
                head * horn,
                BodyPart::new(Vec3::new(0.16, 0.36, 0.16), 0.055),
                style.pants,
            );
        }
    }
    if recipe.extras.wings {
        for side in [-1.0, 1.0] {
            let wing = Mat4::from_translation(Vec3::new(side * 0.56, 1.72, 0.30))
                * Mat4::from_quat(Quat::from_rotation_z(side * 0.18));
            add_part(
                vertices,
                cache,
                root * wing,
                BodyPart::new(Vec3::new(0.16, 0.72, 0.42), 0.07),
                style.shirt,
            );
        }
    }
    if recipe.extras.tail_segments > 0 {
        let count = recipe.extras.tail_segments as usize;
        for index in 0..count {
            let progress = index as f32 / count as f32;
            let x = if recipe.id == BodyId::Cat { 0.34 + progress * 0.18 } else { 0.0 };
            let y = 0.98 + progress * 0.13;
            let z = 0.34 + progress * 0.43;
            let tail = Mat4::from_translation(Vec3::new(x, y, z))
                * Mat4::from_quat(Quat::from_rotation_x(-0.22 + progress * 0.25));
            let size = if recipe.id == BodyId::Cat {
                Vec3::new(0.25 - progress * 0.06, 0.32, 0.27 - progress * 0.05)
            } else {
                Vec3::new(0.34 - progress * 0.13, 0.36, 0.42 - progress * 0.15)
            };
            add_part(
                vertices,
                cache,
                root * tail,
                BodyPart::new(size, 0.07),
                style.pants,
            );
        }
    }
}

fn add_seam_cores(
    vertices: &mut Vec<Vertex>,
    cache: &mut rounded_geometry::RoundedMeshCache,
    root: Mat4,
    recipe: &BodyRecipe,
    joints: &[Mat4],
) {
    // These sit in the authored clearances, not at arbitrary part centers. The
    // existing depth-tested world pipeline makes them disappear naturally
    // behind a nearer body piece.
    let mut seam = |vertices: &mut Vec<Vertex>, local: Mat4, size: Vec3| {
        add_part(
            vertices,
            cache,
            root * local,
            BodyPart::new(size, size.min_element() * 0.28),
            SEAM_COLOR,
        );
    };
    seam(
        vertices,
        joints[JointId::Head.index()]
            * Mat4::from_translation(Vec3::new(0.0, -recipe.head.size.y * 0.5 - 0.018, 0.0)),
        Vec3::splat(0.075),
    );
    for joint in [
        JointId::LeftHand,
        JointId::RightHand,
        JointId::LeftFoot,
        JointId::RightFoot,
    ] {
        seam(
            vertices,
            joints[joint.index()],
            Vec3::splat(if matches!(joint, JointId::LeftFoot | JointId::RightFoot) {
                0.065
            } else {
                0.055
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_anchors_are_body_defined() {
        assert_ne!(camera_anchor(BodyId::Person), camera_target(BodyId::Dragon));
        assert!(camera_anchor(BodyId::Cat).y > camera_target(BodyId::Cat).y);
    }
}
