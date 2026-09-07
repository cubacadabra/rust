//! Coordinated hero review: the production model, materials, and animator.
use super::super::hero_character::Study;
use super::*;
use crate::character::{BodyId, CharacterPresentationState, FacePreset};
use crate::types::{
    CharacterEmote, CharacterEntityKey, CharacterEntityKind, CharacterMotionEvent,
    CharacterMotionSample, CharacterMotionSource, CharacterSupport,
};

pub(super) fn actor(time: f32) -> RenderEntity {
    let key = CharacterEntityKey {
        kind: CharacterEntityKind::LocalPlayer,
        slot: 0,
        generation: 1,
        identity: 0,
    };
    let mut state = CharacterPresentationState::new(key, BodyId::Person);
    let mut result = RenderEntity::default();
    for tick in 0..=(time.clamp(0.0, 10.0) * 60.0).round() as u64 {
        let t = tick as f32 / 60.0;
        let expression = if t < 1.6 {
            FacePreset::Curious
        } else if (5.5..6.1).contains(&t) {
            FacePreset::Wink
        } else if (4.0..5.5).contains(&t) {
            FacePreset::Grin
        } else {
            FacePreset::Happy
        };
        state.set_expression(expression);
        let output = state.evaluate(
            CharacterMotionSample {
                key,
                sequence: tick + 1,
                time: t,
                position: [0.0; 3],
                facing_yaw: 0.0,
                look_yaw: if t < 1.6 { -0.65 } else { -0.27 },
                planar_velocity: Some([0.0; 2]),
                vertical_velocity: Some(0.0),
                support: CharacterSupport::Grounded { height: 0.0 },
                stride_phase: 0.0,
                moving: false,
                sprinting: false,
                source: CharacterMotionSource::Simulation,
                event: CharacterMotionEvent::None,
                emote: if tick == 180 {
                    CharacterEmote::Wave
                } else {
                    CharacterEmote::None
                },
                emote_sequence: u64::from(tick >= 180),
                appearance_revision: 0,
            },
            BodyId::Person,
            false,
        );
        result.pose = output.pose;
        result.face = output.face;
        result.secondary = output.secondary;
        result.support = CharacterSupport::Grounded { height: 0.0 };
    }
    result
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeroCaptureSet {
    #[default]
    Full,
    Stills,
    Motion,
    Review,
    /// One image: girl and nonbinary heads, front/side/back, at one size.
    Hair,
}

impl HeroCaptureSet {
    pub const fn expected_captures(self) -> usize {
        match self {
            Self::Full => 632,
            Self::Stills => 30,
            Self::Motion => 602,
            Self::Review => 8,
            Self::Hair => 1,
        }
    }

    fn includes_stills(self) -> bool {
        matches!(self, Self::Full | Self::Stills | Self::Review)
    }

    fn includes_motion(self) -> bool {
        matches!(self, Self::Full | Self::Motion)
    }

    fn includes_static(self, name: &str) -> bool {
        match self {
            Self::Full | Self::Stills => true,
            Self::Motion | Self::Hair => false,
            Self::Review => matches!(
                name,
                "hero-front"
                    | "hero-three-quarter"
                    | "hero-side"
                    | "hero-back"
                    | "hero-face"
                    | "hero-wave"
                    | "hero-wave-silhouette"
                    | "hero-study-gameplay"
            ),
        }
    }
}

pub fn capture_phase9_hero(
    output_dir: impl AsRef<Path>,
    config: CaptureConfig,
) -> Result<CaptureReport, String> {
    capture_phase9_hero_with_set(output_dir, config, HeroCaptureSet::Full)
}

pub fn capture_phase9_hero_with_set(
    output_dir: impl AsRef<Path>,
    mut config: CaptureConfig,
    capture_set: HeroCaptureSet,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|e| format!("create hero directory: {e}"))?;
    config.avatar = CaptureAvatar::Magic;
    let mut context = HeadlessContext::new_with_quality(true)?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: false,
    };
    let mut captures = Vec::new();
    if capture_set == HeroCaptureSet::Hair {
        captures.push(context.capture(output_dir, config, Scenario::HairReview)?);
    }
    if capture_set.includes_stills() {
        for (name, yaw, pitch, distance, time) in [
            ("hero-front", std::f32::consts::PI, 0.18, 4.8, 2.4),
            ("hero-three-quarter", 2.70, 0.22, 4.8, 2.4),
            ("hero-side", std::f32::consts::FRAC_PI_2, 0.12, 4.8, 2.4),
            ("hero-back", 0.0, 0.16, 4.8, 2.4),
            ("hero-face", 2.90, 0.06, 2.3, 2.4),
            ("hero-curious", 2.90, 0.06, 2.3, 1.0),
            ("hero-wink", 2.90, 0.06, 2.3, 5.9),
            ("hero-wave", 2.70, 0.22, 4.8, 3.4),
            ("hero-wave-silhouette", 2.70, 0.22, 4.8, 3.4),
        ] {
            if !capture_set.includes_static(name) {
                continue;
            }
            let mut frame = config;
            frame.pose_time = time;
            captures.push(context.capture(
                output_dir,
                frame,
                Scenario::Hero {
                    name,
                    yaw,
                    pitch,
                    distance,
                    study: Study::Everyday,
                    silhouette: name == "hero-wave-silhouette",
                    motion: false,
                },
            )?);
        }
    }
    if capture_set.includes_stills() {
        // Comparable views reopen proportion/face review without multiplying
        // wardrobe IDs or substituting generated illustration for engine evidence.
        for (study, names) in [
            (
                Study::Everyday,
                [
                    "hero-study-front",
                    "hero-study-side",
                    "hero-study-back",
                    "hero-study-three-quarter",
                    "hero-study-face",
                    "hero-study-silhouette",
                    "hero-study-gameplay",
                ],
            ),
            (
                Study::LongerLegs,
                [
                    "longer-front",
                    "longer-side",
                    "longer-back",
                    "longer-three-quarter",
                    "longer-face",
                    "longer-silhouette",
                    "longer-gameplay",
                ],
            ),
            (
                Study::SoftShoulders,
                [
                    "soft-front",
                    "soft-side",
                    "soft-back",
                    "soft-three-quarter",
                    "soft-face",
                    "soft-silhouette",
                    "soft-gameplay",
                ],
            ),
        ] {
            for (index, (yaw, pitch, distance)) in [
                (std::f32::consts::PI, 0.18, 4.8),
                (std::f32::consts::FRAC_PI_2, 0.12, 4.8),
                (0.0, 0.16, 4.8),
                (2.70, 0.22, 4.8),
                (2.90, 0.06, 2.3),
                (2.70, 0.22, 4.8),
                (0.0, 0.22, 9.0),
            ]
            .into_iter()
            .enumerate()
            {
                if !capture_set.includes_static(names[index]) {
                    continue;
                }
                let mut settings = config;
                settings.pose_time = 2.4;
                captures.push(context.capture(
                    output_dir,
                    settings,
                    Scenario::Hero {
                        name: names[index],
                        yaw,
                        pitch,
                        distance,
                        study,
                        silhouette: index == 5,
                        motion: false,
                    },
                )?);
            }
        }
    }
    if capture_set.includes_motion() {
        for motion in [false, true] {
            for frame in 0..=300 {
                let mut settings = config;
                settings.pose_time = frame as f32 / 30.0;
                let mut capture = context.capture(
                    output_dir,
                    settings,
                    Scenario::Hero {
                        name: "hero-motion",
                        yaw: if motion {
                            std::f32::consts::FRAC_PI_2
                        } else {
                            2.70
                        },
                        pitch: 0.22,
                        distance: if motion { 7.2 } else { 4.8 },
                        study: Study::Everyday,
                        silhouette: false,
                        motion,
                    },
                )?;
                let name = format!("{}-{frame:04}", if motion { "gait" } else { "hero" });
                let image = format!("{name}.png");
                fs::rename(output_dir.join(&capture.image), output_dir.join(&image))
                    .map_err(|e| format!("save hero frame: {e}"))?;
                capture.name = name;
                capture.image = image;
                captures.push(capture);
            }
        }
    }
    let capture_note = match capture_set {
        HeroCaptureSet::Full => {
            "30 static views plus two 301-frame timelines (greeting and side-on gait) at 30 fps; presentation evaluates at 60 Hz."
        }
        HeroCaptureSet::Stills => {
            "Phase 9 stills capture set: 30 static/review views; animation timelines omitted."
        }
        HeroCaptureSet::Motion => {
            "Phase 9 motion capture set: two 301-frame timelines; static/review views omitted."
        }
        HeroCaptureSet::Review => {
            "Phase 9 review capture set: 8 Everyday-person static views; animation timelines and alternate studies omitted."
        }
        HeroCaptureSet::Hair => {
            "One hair review image: girl above nonbinary; front, side and back from left to right."
        }
    };
    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "person-direction-studies-v2".to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            capture_note,
            "0–1.6s notice; 1.6–3s turn and smile; 3s wave; 4–5.5s grin; 5.5–6.1s wink; settle through 10s.",
            "Studio palette and camera are review-only; geometry, materials, face controls, animation and soft-shadow mesh also run in gameplay.",
            "All views are engine renders. Everyday is the working default, not an approved final design. LongerLegs and SoftShoulders are capture-only comparisons.",
        ],
    };
    fs::write(
        output_dir.join("phase9_report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| format!("serialize hero report: {e}"))?,
    )
    .map_err(|e| format!("write hero report: {e}"))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_capture_sets_have_stable_counts_without_gpu_setup() {
        assert_eq!(HeroCaptureSet::Full.expected_captures(), 632);
        assert_eq!(HeroCaptureSet::Stills.expected_captures(), 30);
        assert_eq!(HeroCaptureSet::Motion.expected_captures(), 602);
        assert_eq!(HeroCaptureSet::Review.expected_captures(), 8);
        assert!(HeroCaptureSet::default().includes_stills());
        assert!(HeroCaptureSet::default().includes_motion());
        assert!(!HeroCaptureSet::Review.includes_motion());
        assert!(HeroCaptureSet::Review.includes_static("hero-wave"));
        assert!(!HeroCaptureSet::Review.includes_static("hero-curious"));
        assert!(HeroCaptureSet::Stills.includes_static("soft-gameplay"));
    }

    #[test]
    fn hero_uses_the_common_animator_and_settles_after_the_greeting() {
        assert!(actor(3.1).secondary.spark_life > 0.0);
        assert_eq!(actor(4.5).secondary.spark_life, 0.0);
        assert!(actor(5.9).face.eye_asymmetry > 0.3);
        let rig = body_recipe(BodyId::Person).rig;
        let hand = crate::character::JointId::RightHand.index();
        let idle = rig.world_matrices(&actor(2.4).pose.transforms)[hand]
            .w_axis
            .y;
        let waving = rig.world_matrices(&actor(3.4).pose.transforms)[hand]
            .w_axis
            .y;
        assert!(waving > idle + 0.5);
        assert!(actor(10.0).face.eye_asymmetry < 0.001);
    }
}
