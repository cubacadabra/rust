//! Deterministic presentation review: simulate at 60 Hz, save at 30 Hz.
use super::*;
use crate::character::{BodyId, CharacterPresentationState, OutfitId};
use crate::types::{
    CharacterEmote, CharacterEntityKey, CharacterEntityKind, CharacterMotionEvent,
    CharacterMotionSample, CharacterMotionSource, CharacterSupport,
};

pub(super) const CAMERA_YAW: f32 = 2.75;

fn sample(tick: u64, slot: usize) -> CharacterMotionSample {
    let time = tick as f32 / 60.0;
    let speed = if (60..180).contains(&tick) {
        6.4
    } else if (180..300).contains(&tick) {
        11.5
    } else if (300..360).contains(&tick) {
        6.4
    } else {
        0.0
    };
    let travel = (time - 1.0).clamp(0.0, 2.0) * 6.4
        + (time - 3.0).clamp(0.0, 2.0) * 11.5
        + (time - 5.0).clamp(0.0, 1.0) * 6.4;
    let airborne = (300..360).contains(&tick);
    let jump_age = time - 5.0;
    CharacterMotionSample {
        key: CharacterEntityKey {
            kind: CharacterEntityKind::LocalNpc,
            slot,
            generation: 1,
            identity: 0,
        },
        sequence: tick + 1,
        time,
        position: [
            0.0,
            if airborne {
                10.5 * jump_age * (1.0 - jump_age)
            } else {
                0.0
            },
            -travel,
        ],
        facing_yaw: 0.0,
        look_yaw: if time >= 8.5 {
            ((time - 8.5) * 2.0).sin() * 0.7
        } else {
            0.0
        },
        planar_velocity: Some([0.0, -speed]),
        vertical_velocity: Some(if airborne {
            10.5 - 21.0 * jump_age
        } else {
            0.0
        }),
        support: if airborne {
            CharacterSupport::Airborne
        } else {
            CharacterSupport::Grounded { height: 0.0 }
        },
        stride_phase: travel * 1.8,
        moving: speed > 0.0,
        sprinting: (180..300).contains(&tick),
        source: CharacterMotionSource::Simulation,
        event: match tick {
            300 => CharacterMotionEvent::Takeoff,
            360 => CharacterMotionEvent::Landing,
            _ => CharacterMotionEvent::None,
        },
        emote: if tick == 420 {
            CharacterEmote::Wave
        } else {
            CharacterEmote::None
        },
        emote_sequence: u64::from(tick >= 420),
        appearance_revision: 0,
    }
}

pub(super) fn actors(time: f32) -> Vec<RenderEntity> {
    let tick = (time.clamp(0.0, 10.0) * 60.0).round() as u64;
    BodyId::ALL
        .into_iter()
        .enumerate()
        .map(|(slot, body)| {
            let mut state = CharacterPresentationState::new(sample(0, slot).key, body);
            let mut output = state.evaluate(sample(0, slot), body, false);
            for step in 1..=tick {
                output = state.evaluate(sample(step, slot), body, false);
            }
            let motion = sample(tick, slot);
            // Stage travel in place for a fixed camera. Presentation still receives
            // continuous world distance, velocity, support, and one-shot events.
            let axis = Vec3::new(CAMERA_YAW.cos(), 0.0, -CAMERA_YAW.sin());
            RenderEntity {
                position: (axis * (slot as f32 - 1.0) * 2.6 + Vec3::Y * motion.position[1])
                    .to_array(),
                body,
                outfit: match body {
                    BodyId::Person => OutfitId::EverydayHoodie,
                    BodyId::Cat => OutfitId::PufferExplorer,
                    BodyId::Dragon => OutfitId::ToyKnight,
                },
                moving: motion.moving,
                sprinting: motion.sprinting,
                walk_cycle: motion.stride_phase,
                pose: output.pose,
                face: output.face,
                secondary: output.secondary,
                ..Default::default()
            }
        })
        .collect()
}

/// Save a ten-second, three-species motion review using live presentation.
pub fn capture_phase4_motion(
    output_dir: impl AsRef<Path>,
    mut config: CaptureConfig,
) -> Result<CaptureReport, String> {
    let output_dir = output_dir.as_ref();
    config.avatar = CaptureAvatar::Magic;
    config.pose_time = 0.0;
    fs::create_dir_all(output_dir).map_err(|error| format!("create output directory: {error}"))?;
    let mut context = HeadlessContext::new()?;
    let adapter = AdapterRecord {
        name: context.adapter_info.name.clone(),
        backend: format!("{:?}", context.adapter_info.backend),
        device_type: format!("{:?}", context.adapter_info.device_type),
        driver: context.adapter_info.driver.clone(),
        driver_info: context.adapter_info.driver_info.clone(),
        gpu_timestamps: context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
    };
    let mut captures = Vec::with_capacity(301);
    for frame in 0..=300 {
        let mut frame_config = config;
        frame_config.pose_time = frame as f32 / 30.0;
        let mut capture = context.capture(output_dir, frame_config, Scenario::MotionLineup)?;
        let name = format!("motion-{frame:04}");
        let image = format!("{name}.png");
        fs::rename(output_dir.join(&capture.image), output_dir.join(&image))
            .map_err(|error| format!("save motion frame: {error}"))?;
        capture.name = name;
        capture.image = image;
        captures.push(capture);
    }
    let report = CaptureReport {
        format_version: FORMAT_VERSION,
        fixture: "phase-4-gameplay-presentation-motion".to_owned(),
        config,
        adapter,
        captures,
        engine_capacity_characters: 18,
        render_only_stress_characters: 50,
        notes: vec![
            "301 frames at 30 Hz; presentation evaluates deterministic motion samples at 60 Hz.",
            "0–1s idle; 1–3s walk; 3–5s run; 5–6s jump; 6s landing; 7s wave; 8.5–10s look around.",
            "Uses CharacterPresentationState and the production instanced renderer for all three species.",
            "Travel is staged in place; this reviews presentation, not physics, collision, or input latency.",
        ],
    };
    fs::write(
        output_dir.join("phase4_report.json"),
        serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("serialize motion report: {error}"))?,
    )
    .map_err(|error| format!("write motion report: {error}"))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_review_covers_events_and_settles() {
        assert_eq!(sample(300, 0).event, CharacterMotionEvent::Takeoff);
        assert_eq!(sample(360, 0).event, CharacterMotionEvent::Landing);
        assert_eq!(sample(420, 0).emote, CharacterEmote::Wave);
        assert!(
            actors(5.2)
                .iter()
                .all(|a| a.position[1] > 1.0 && a.secondary.spark_life > 0.0)
        );
        assert!(
            actors(6.1)
                .iter()
                .all(|a| a.position[1] == 0.0 && a.secondary.spark_life > 0.0)
        );
        assert!(actors(7.1).iter().all(|a| a.secondary.spark_life > 0.0));
        assert!(actors(8.5).iter().all(|a| a.secondary.spark_life == 0.0));
        for (a, b) in actors(5.2).iter().zip(actors(5.2)) {
            assert_eq!(a.pose, b.pose);
            assert_eq!(a.face, b.face);
            assert_eq!(a.secondary, b.secondary);
        }
    }
}
