use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum JointId {
    Root = 0,
    Torso,
    Head,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
}

impl JointId {
    pub(crate) const ALL: [Self; 15] = [
        Self::Root,
        Self::Torso,
        Self::Head,
        Self::LeftUpperArm,
        Self::LeftLowerArm,
        Self::LeftHand,
        Self::RightUpperArm,
        Self::RightLowerArm,
        Self::RightHand,
        Self::LeftUpperLeg,
        Self::LeftLowerLeg,
        Self::LeftFoot,
        Self::RightUpperLeg,
        Self::RightLowerLeg,
        Self::RightFoot,
    ];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JointTransform {
    pub(crate) translation: Vec3,
    pub(crate) rotation: Quat,
}

impl JointTransform {
    pub(crate) const fn new(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
        }
    }

    fn matrix(self) -> Mat4 {
        Mat4::from_translation(self.translation) * Mat4::from_quat(self.rotation)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JointDefinition {
    pub(crate) id: JointId,
    pub(crate) parent: Option<JointId>,
    pub(crate) rest: JointTransform,
    /// The intended visible clearance around this joint in engine units.
    pub(crate) clearance: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct RigDefinition {
    pub(crate) joints: Vec<JointDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RigError {
    WrongJointCount,
    MissingJoint(JointId),
    InvalidParent(JointId),
    Cycle(JointId),
    InvalidClearance(JointId),
}

impl RigDefinition {
    pub(crate) fn validate(&self) -> Result<(), RigError> {
        if self.joints.len() != JointId::ALL.len() {
            return Err(RigError::WrongJointCount);
        }
        for id in JointId::ALL {
            let Some(joint) = self.joints.iter().find(|joint| joint.id == id) else {
                return Err(RigError::MissingJoint(id));
            };
            if !joint.clearance.is_finite() || joint.clearance < 0.0 {
                return Err(RigError::InvalidClearance(id));
            }
            if joint.parent == Some(id) {
                return Err(RigError::Cycle(id));
            }
            if let Some(parent) = joint.parent
                && !self.joints.iter().any(|candidate| candidate.id == parent)
            {
                return Err(RigError::InvalidParent(id));
            }
        }
        for id in JointId::ALL {
            let mut current = Some(id);
            for _ in 0..JointId::ALL.len() {
                current = current.and_then(|child| {
                    self.joints
                        .iter()
                        .find(|joint| joint.id == child)
                        .and_then(|joint| joint.parent)
                });
                if current.is_none() {
                    break;
                }
            }
            if current.is_some() {
                return Err(RigError::Cycle(id));
            }
        }
        Ok(())
    }

    pub(crate) fn rest_transforms(&self) -> Vec<JointTransform> {
        let mut transforms = vec![JointTransform::new(Vec3::ZERO); JointId::ALL.len()];
        for joint in &self.joints {
            transforms[joint.id.index()] = joint.rest;
        }
        transforms
    }

    pub(crate) fn world_matrices(&self, local: &[JointTransform]) -> Vec<Mat4> {
        assert_eq!(local.len(), self.joints.len());
        let mut world = vec![Mat4::IDENTITY; self.joints.len()];
        for joint in &self.joints {
            world[joint.id.index()] = joint
                .parent
                .map_or(Mat4::IDENTITY, |parent| world[parent.index()])
                * local[joint.id.index()].matrix();
        }
        world
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub(crate) transforms: [JointTransform; 15],
}

impl Pose {
    pub(crate) fn rest(rig: &RigDefinition) -> Self {
        let mut transforms = [JointTransform::new(Vec3::ZERO); 15];
        for transform in rig.rest_transforms().into_iter().enumerate() {
            transforms[transform.0] = transform.1;
        }
        Self { transforms }
    }

    pub(crate) fn locomotion(rig: &RigDefinition, phase: f32, moving: bool, sprinting: bool) -> Self {
        let mut pose = Self::rest(rig);
        if !moving {
            return pose;
        }
        let amplitude = if sprinting { 0.62 } else { 0.42 };
        let swing = phase.sin() * amplitude;
        pose.rotate(JointId::LeftUpperArm, swing * 0.65);
        pose.rotate(JointId::RightUpperArm, -swing * 0.65);
        pose.rotate(JointId::LeftUpperLeg, -swing);
        pose.rotate(JointId::RightUpperLeg, swing);
        pose.rotate(JointId::LeftLowerLeg, swing.abs() * 0.28);
        pose.rotate(JointId::RightLowerLeg, -swing.abs() * 0.28);
        pose
    }

    fn rotate(&mut self, joint: JointId, angle: f32) {
        self.transforms[joint.index()].rotation = Quat::from_rotation_x(angle);
    }
}

pub(crate) fn common_rest_rig() -> RigDefinition {
    use JointId::*;
    let joint = |id, parent, translation, clearance| JointDefinition {
        id,
        parent,
        rest: JointTransform::new(translation),
        clearance,
    };
    RigDefinition {
        joints: vec![
            joint(Root, None, Vec3::ZERO, 0.0),
            joint(Torso, Some(Root), Vec3::new(0.0, 1.72, 0.0), 0.025),
            joint(Head, Some(Torso), Vec3::new(0.0, 1.12, 0.0), 0.04),
            joint(LeftUpperArm, Some(Torso), Vec3::new(-0.66, 0.05, 0.0), 0.035),
            joint(LeftLowerArm, Some(LeftUpperArm), Vec3::new(0.0, -0.58, 0.0), 0.035),
            joint(LeftHand, Some(LeftLowerArm), Vec3::new(0.0, -0.48, -0.01), 0.04),
            joint(RightUpperArm, Some(Torso), Vec3::new(0.66, 0.05, 0.0), 0.035),
            joint(RightLowerArm, Some(RightUpperArm), Vec3::new(0.0, -0.58, 0.0), 0.035),
            joint(RightHand, Some(RightLowerArm), Vec3::new(0.0, -0.48, -0.01), 0.04),
            joint(LeftUpperLeg, Some(Root), Vec3::new(-0.25, 0.91, 0.0), 0.035),
            joint(LeftLowerLeg, Some(LeftUpperLeg), Vec3::new(0.0, -0.56, 0.0), 0.035),
            joint(LeftFoot, Some(LeftLowerLeg), Vec3::new(0.0, -0.30, -0.10), 0.045),
            joint(RightUpperLeg, Some(Root), Vec3::new(0.25, 0.91, 0.0), 0.035),
            joint(RightLowerLeg, Some(RightUpperLeg), Vec3::new(0.0, -0.56, 0.0), 0.035),
            joint(RightFoot, Some(RightLowerLeg), Vec3::new(0.0, -0.30, -0.10), 0.045),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_rig_is_acyclic_and_has_authored_gaps() {
        let rig = common_rest_rig();
        rig.validate().expect("common rig should validate");
        assert!(rig.joints.iter().any(|joint| joint.clearance > 0.0));
    }

    #[test]
    fn child_world_transform_follows_parent() {
        let rig = common_rest_rig();
        let mut pose = Pose::rest(&rig);
        pose.transforms[JointId::Torso.index()].rotation = Quat::from_rotation_z(0.2);
        let world = rig.world_matrices(&pose.transforms);
        let head = world[JointId::Head.index()].transform_point3(Vec3::ZERO);
        assert!(head.x.abs() > 0.01);
    }
}

