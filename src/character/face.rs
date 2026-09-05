use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum FacePreset {
    Neutral,
    Happy,
    Surprised,
    Determined,
    Sad,
    Laughing,
    Smile,
    Grin,
    Curious,
    Amazed,
    Angry,
    Crying,
    Worried,
    Embarrassed,
    Sleepy,
    Squinting,
    Wink,
    Smirk,
    Confused,
    Excited,
    Unimpressed,
}

#[allow(dead_code)]
impl FacePreset {
    pub(crate) const ALL: [Self; 21] = [
        Self::Neutral, Self::Happy, Self::Surprised, Self::Determined,
        Self::Sad, Self::Laughing, Self::Smile, Self::Grin, Self::Curious,
        Self::Amazed, Self::Angry, Self::Crying, Self::Worried, Self::Embarrassed,
        Self::Sleepy, Self::Squinting, Self::Wink, Self::Smirk, Self::Confused,
        Self::Excited, Self::Unimpressed,
    ];

    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral", Self::Happy => "happy", Self::Surprised => "surprised",
            Self::Determined => "determined", Self::Sad => "sad", Self::Laughing => "laughing",
            Self::Smile => "smile", Self::Grin => "grin", Self::Curious => "curious",
            Self::Amazed => "amazed", Self::Angry => "angry", Self::Crying => "crying",
            Self::Worried => "worried", Self::Embarrassed => "embarrassed", Self::Sleepy => "sleepy",
            Self::Squinting => "squinting", Self::Wink => "wink", Self::Smirk => "smirk",
            Self::Confused => "confused", Self::Excited => "excited", Self::Unimpressed => "unimpressed",
        }
    }

}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FaceParameters {
    pub(crate) eye_opening: f32,
    pub(crate) look: Vec2,
    pub(crate) brow_tilt: f32,
    pub(crate) mouth_curve: f32,
    pub(crate) mouth_opening: f32,
}

impl Default for FaceParameters {
    fn default() -> Self {
        Self::preset(FacePreset::Neutral)
    }
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
            FacePreset::Sad => Self { eye_opening: 0.88, look: Vec2::new(0.0, -0.03), brow_tilt: 0.32, mouth_curve: -0.75, mouth_opening: 0.04 },
            FacePreset::Laughing => Self { eye_opening: 0.48, look: Vec2::ZERO, brow_tilt: 0.08, mouth_curve: 0.8, mouth_opening: 0.8 },
            FacePreset::Smile => Self { eye_opening: 0.95, look: Vec2::ZERO, brow_tilt: 0.04, mouth_curve: 0.55, mouth_opening: 0.05 },
            FacePreset::Grin => Self { eye_opening: 0.9, look: Vec2::ZERO, brow_tilt: 0.0, mouth_curve: 0.9, mouth_opening: 0.34 },
            FacePreset::Curious => Self { eye_opening: 1.08, look: Vec2::new(0.08, 0.03), brow_tilt: 0.22, mouth_curve: 0.12, mouth_opening: 0.08 },
            FacePreset::Amazed => Self { eye_opening: 1.2, look: Vec2::new(0.0, 0.02), brow_tilt: 0.18, mouth_curve: 0.0, mouth_opening: 0.82 },
            FacePreset::Angry => Self { eye_opening: 0.78, look: Vec2::ZERO, brow_tilt: -0.42, mouth_curve: -0.2, mouth_opening: 0.1 },
            FacePreset::Crying => Self { eye_opening: 0.45, look: Vec2::new(0.0, -0.04), brow_tilt: 0.36, mouth_curve: -0.65, mouth_opening: 0.2 },
            FacePreset::Worried => Self { eye_opening: 0.92, look: Vec2::new(0.0, -0.02), brow_tilt: 0.4, mouth_curve: -0.3, mouth_opening: 0.08 },
            FacePreset::Embarrassed => Self { eye_opening: 0.62, look: Vec2::new(0.1, -0.06), brow_tilt: 0.16, mouth_curve: 0.2, mouth_opening: 0.0 },
            FacePreset::Sleepy => Self { eye_opening: 0.4, look: Vec2::new(0.0, -0.08), brow_tilt: 0.0, mouth_curve: 0.0, mouth_opening: 0.0 },
            FacePreset::Squinting => Self { eye_opening: 0.38, look: Vec2::ZERO, brow_tilt: -0.08, mouth_curve: 0.1, mouth_opening: 0.0 },
            FacePreset::Wink => Self { eye_opening: 0.58, look: Vec2::new(-0.04, 0.0), brow_tilt: 0.08, mouth_curve: 0.45, mouth_opening: 0.0 },
            FacePreset::Smirk => Self { eye_opening: 0.85, look: Vec2::new(0.05, 0.0), brow_tilt: -0.12, mouth_curve: 0.42, mouth_opening: 0.03 },
            FacePreset::Confused => Self { eye_opening: 1.0, look: Vec2::new(-0.07, 0.02), brow_tilt: 0.3, mouth_curve: -0.08, mouth_opening: 0.12 },
            FacePreset::Excited => Self { eye_opening: 1.18, look: Vec2::new(0.0, 0.04), brow_tilt: 0.18, mouth_curve: 0.65, mouth_opening: 0.48 },
            FacePreset::Unimpressed => Self { eye_opening: 0.68, look: Vec2::new(0.0, -0.01), brow_tilt: -0.05, mouth_curve: -0.04, mouth_opening: 0.0 },
        }
    }

    pub(crate) fn clamped(self) -> Self {
        Self {
            eye_opening: self.eye_opening.clamp(0.05, 1.25),
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
