use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum FacePreset {
    Neutral,
    Happy,
    Surprised,
    Determined,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FaceParameters {
    pub(crate) eye_opening: f32,
    pub(crate) look: Vec2,
    pub(crate) brow_tilt: f32,
    pub(crate) mouth_curve: f32,
    pub(crate) mouth_opening: f32,
}

impl FaceParameters {
    pub(crate) fn preset(preset: FacePreset) -> Self {
        match preset {
            FacePreset::Neutral => Self {
                eye_opening: 1.0,
                look: Vec2::ZERO,
                brow_tilt: 0.0,
                mouth_curve: 0.0,
                mouth_opening: 0.0,
            },
            FacePreset::Happy => Self {
                eye_opening: 0.92,
                look: Vec2::new(0.0, 0.02),
                brow_tilt: 0.08,
                mouth_curve: 0.8,
                mouth_opening: 0.18,
            },
            FacePreset::Surprised => Self {
                eye_opening: 1.18,
                look: Vec2::ZERO,
                brow_tilt: 0.0,
                mouth_curve: 0.0,
                mouth_opening: 0.65,
            },
            FacePreset::Determined => Self {
                eye_opening: 0.82,
                look: Vec2::new(0.02, -0.01),
                brow_tilt: -0.28,
                mouth_curve: -0.15,
                mouth_opening: 0.0,
            },
        }
    }

    pub(crate) fn clamped(self) -> Self {
        Self {
            eye_opening: self.eye_opening.clamp(0.35, 1.25),
            look: self.look.clamp(Vec2::splat(-0.16), Vec2::splat(0.16)),
            brow_tilt: self.brow_tilt.clamp(-0.45, 0.45),
            mouth_curve: self.mouth_curve.clamp(-1.0, 1.0),
            mouth_opening: self.mouth_opening.clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FaceAnchors {
    /// Head-local coordinates. The face points toward local -Z.
    pub(crate) eye_y: f32,
    pub(crate) eye_x: f32,
    pub(crate) face_z: f32,
    pub(crate) brow_y: f32,
    pub(crate) mouth_y: f32,
    pub(crate) muzzle_y: f32,
}

impl Default for FaceAnchors {
    fn default() -> Self {
        Self {
            eye_y: 0.10,
            eye_x: 0.19,
            face_z: -0.455,
            brow_y: 0.27,
            mouth_y: -0.18,
            muzzle_y: -0.10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_parameters_are_bounded() {
        let face = FaceParameters {
            eye_opening: 4.0,
            look: Vec2::splat(4.0),
            brow_tilt: -4.0,
            mouth_curve: 4.0,
            mouth_opening: 4.0,
        }
        .clamped();
        assert_eq!(face.eye_opening, 1.25);
        assert_eq!(face.look, Vec2::splat(0.16));
        assert_eq!(face.brow_tilt, -0.45);
        assert_eq!(face.mouth_curve, 1.0);
        assert_eq!(face.mouth_opening, 1.0);
    }
}
