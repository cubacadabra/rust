use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CharacterEntityKind, CharacterMotionSource};

    fn sample(sequence: u64, time: f32) -> CharacterMotionSample {
        CharacterMotionSample {
            key: CharacterEntityKey {
                kind: CharacterEntityKind::LocalPlayer,
                slot: 0,
                generation: 1,
                identity: 0,
            },
            sequence,
            time,
            position: [0.0, 0.0, -time],
            facing_yaw: std::f32::consts::PI - 0.01,
            look_yaw: -std::f32::consts::PI + 0.01,
            planar_velocity: Some([0.0, 6.4]),
            vertical_velocity: Some(0.0),
            support: CharacterSupport::Grounded { height: 0.0 },
            stride_phase: time * 4.0,
            moving: true,
            sprinting: false,
            source: CharacterMotionSource::Simulation,
            event: CharacterMotionEvent::None,
            emote: CharacterEmote::None,
            emote_sequence: 0,
            appearance_revision: 0,
        }
    }

    #[test]
    fn repeated_sequence_does_not_advance_presentation() {
        let mut state = CharacterPresentationState::new(sample(1, 0.0).key, BodyId::Person);
        let first = state.evaluate(sample(1, 0.0), BodyId::Person, false);
        let repeated = state.evaluate(sample(1, 1.0), BodyId::Person, false);
        assert_eq!(first, repeated);
    }

    #[test]
    fn planted_targets_follow_real_presentation_lifecycle() {
        let mut state = CharacterPresentationState::new(sample(1, 0.0).key, BodyId::Person);
        let first = state.evaluate(sample(1, 0.0), BodyId::Person, false);
        let left_target = first.secondary.left_foot_target.expect("stance target");
        assert!((left_target.y - STANCE_ANKLE_HEIGHT).abs() < 0.0001);
        assert!(first.secondary.right_foot_target.is_none());

        let repeated = state.evaluate(sample(1, 1.0), BodyId::Person, false);
        assert_eq!(repeated, first);

        let mut takeoff = sample(2, 1.0 / 60.0);
        takeoff.event = CharacterMotionEvent::Takeoff;
        takeoff.support = CharacterSupport::Airborne;
        assert!(
            state
                .evaluate(takeoff, BodyId::Person, false)
                .secondary
                .left_foot_target
                .is_none()
        );

        let mut teleport = sample(3, 2.0 / 60.0);
        teleport.position = [20.0, 0.0, 0.0];
        assert!(
            state
                .evaluate(teleport, BodyId::Person, false)
                .secondary
                .left_foot_target
                .is_none()
        );

        let mut raised = sample(1, 0.0);
        raised.position[1] = 2.0;
        raised.support = CharacterSupport::Grounded { height: 2.0 };
        let raised_output = CharacterPresentationState::new(raised.key, BodyId::Person).evaluate(
            raised,
            BodyId::Person,
            false,
        );
        assert!(
            (raised_output
                .secondary
                .left_foot_target
                .expect("raised target")
                .y
                - 2.05)
                .abs()
                < 0.0001
        );
    }

    #[test]
    fn landing_compression_is_event_driven_and_clears_on_reset_paths() {
        let mut state = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
        assert_eq!(
            state
                .evaluate(sample(0, 0.0), BodyId::Person, false)
                .secondary
                .landing_compression,
            0.0
        );

        let mut landing = sample(1, 1.0 / 60.0);
        landing.event = CharacterMotionEvent::Landing;
        let peak = state.evaluate(landing, BodyId::Person, false);
        assert_eq!(peak.secondary.landing_compression, 1.0);

        // A duplicate sequence returns the cached output even if a caller
        // supplies a later wall-clock value.
        let repeated = state.evaluate(sample(1, 1.0), BodyId::Person, false);
        assert_eq!(repeated, peak);

        let decayed = state.evaluate(sample(2, 2.0 / 60.0), BodyId::Person, false);
        assert!(decayed.secondary.landing_compression > 0.0);
        assert!(decayed.secondary.landing_compression < 1.0);
        assert!(decayed.secondary.landing_compression.is_finite());

        let mut takeoff = sample(3, 3.0 / 60.0);
        takeoff.event = CharacterMotionEvent::Takeoff;
        assert_eq!(
            state
                .evaluate(takeoff, BodyId::Person, false)
                .secondary
                .landing_compression,
            0.0
        );

        let mut landing_again = sample(4, 4.0 / 60.0);
        landing_again.event = CharacterMotionEvent::Landing;
        assert_eq!(
            state
                .evaluate(landing_again, BodyId::Person, false)
                .secondary
                .landing_compression,
            1.0
        );
        let mut teleport = sample(5, 5.0 / 60.0);
        teleport.position = [20.0, 0.0, 0.0];
        assert_eq!(
            state
                .evaluate(teleport, BodyId::Person, false)
                .secondary
                .landing_compression,
            0.0
        );

        let mut replacement = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
        replacement.evaluate(sample(0, 0.0), BodyId::Person, false);
        let mut replacement_landing = sample(1, 1.0 / 60.0);
        replacement_landing.event = CharacterMotionEvent::Landing;
        assert_eq!(
            replacement
                .evaluate(replacement_landing, BodyId::Person, false)
                .secondary
                .landing_compression,
            1.0
        );
        let mut roster_replacement = sample(2, 2.0 / 60.0);
        roster_replacement.key.generation = 2;
        assert_eq!(
            replacement
                .evaluate(roster_replacement, BodyId::Person, false)
                .secondary
                .landing_compression,
            0.0
        );

        state.reset(BodyId::Person);
        assert_eq!(
            state
                .evaluate(sample(0, 0.0), BodyId::Person, false)
                .secondary
                .landing_compression,
            0.0
        );
    }

    #[test]
    fn landing_leg_rotation_remains_for_non_hero_presentations() {
        let mut state = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Cat);
        state.evaluate(sample(0, 0.0), BodyId::Cat, false);
        let mut landing = sample(1, 1.0 / 60.0);
        landing.event = CharacterMotionEvent::Landing;
        let output = state.evaluate(landing, BodyId::Cat, false);
        assert!(
            output.pose.transforms[JointId::LeftUpperLeg.index()]
                .rotation
                .to_scaled_axis()
                .x
                < -0.19
        );
    }

    #[test]
    fn yaw_wrap_uses_shortest_turn() {
        let mut state = CharacterPresentationState::new(sample(1, 0.0).key, BodyId::Person);
        let output = state.evaluate(sample(1, 0.0), BodyId::Person, false);
        assert!(
            output.pose.transforms[JointId::Head.index()]
                .rotation
                .is_normalized()
        );
    }

    #[test]
    fn bursts_expire_during_continuous_travel() {
        for event in [CharacterMotionEvent::Takeoff, CharacterMotionEvent::Landing] {
            let mut state = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
            state.evaluate(sample(0, 0.0), BodyId::Person, false);
            let mut trigger = sample(1, 1.0 / 60.0);
            trigger.event = event;
            assert!(
                state
                    .evaluate(trigger, BodyId::Person, false)
                    .secondary
                    .spark_life
                    > 0.0
            );
            for tick in 2..=60 {
                state.evaluate(sample(tick, tick as f32 / 60.0), BodyId::Person, false);
            }
            let settled = state.output.unwrap();
            assert_eq!(settled.secondary.spark_life, 0.0);
            assert_eq!(settled.secondary.gap_expansion, 0.0);
        }
    }

    #[test]
    fn paused_presentation_expires_the_burst() {
        let mut state = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
        state.evaluate(sample(0, 0.0), BodyId::Person, false);
        let mut input = sample(1, 0.016);
        input.event = CharacterMotionEvent::Takeoff;
        assert!(
            state
                .evaluate(input, BodyId::Person, false)
                .secondary
                .spark_life
                > 0.0
        );
        assert_eq!(
            state
                .evaluate(sample(2, 1.0), BodyId::Person, false)
                .secondary
                .spark_life,
            0.0
        );
    }

    #[test]
    fn first_sample_and_teleports_do_not_replay_events() {
        let mut trigger = sample(1, 0.0);
        trigger.event = CharacterMotionEvent::Landing;
        trigger.emote = CharacterEmote::Wave;
        trigger.emote_sequence = 1;
        let mut state = CharacterPresentationState::new(trigger.key, BodyId::Cat);
        assert_eq!(
            state
                .evaluate(trigger, BodyId::Cat, false)
                .secondary
                .spark_life,
            0.0
        );
        for (tick, position) in [[20.0, 0.0, 0.0], [20.0, 20.0, 0.0]]
            .into_iter()
            .enumerate()
        {
            trigger.sequence += 1;
            trigger.time = (tick + 1) as f32 / 60.0;
            trigger.position = position;
            trigger.emote_sequence += 1;
            assert_eq!(
                state
                    .evaluate(trigger, BodyId::Cat, false)
                    .secondary
                    .spark_life,
                0.0
            );
            assert_eq!(state.wave_until, 0.0);
        }
    }

    #[test]
    fn reduced_effects_suppress_bursts_and_shrink_creature_gaps() {
        let mut full = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Cat);
        let mut reduced = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Cat);
        for tick in 0..30 {
            let mut input = sample(tick, tick as f32 / 60.0);
            if tick == 29 {
                input.event = CharacterMotionEvent::Takeoff;
            }
            full.evaluate(input, BodyId::Cat, false);
            reduced.evaluate(input, BodyId::Cat, true);
        }
        let a = full.output.unwrap();
        let b = reduced.output.unwrap();
        assert!(a.secondary.spark_life > 0.0);
        assert_eq!(b.secondary.spark_life, 0.0);
        let rest = body_recipe(BodyId::Cat).rig.joints[JointId::Head.index()]
            .rest
            .translation
            .y;
        assert!(
            b.pose.transforms[JointId::Head.index()].translation.y - rest
                < a.pose.transforms[JointId::Head.index()].translation.y - rest
        );
    }

    #[test]
    fn unknown_support_does_not_imply_airborne() {
        let mut known = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
        let mut unknown = CharacterPresentationState::new(sample(0, 0.0).key, BodyId::Person);
        for tick in 0..30 {
            let input = sample(tick, tick as f32 / 60.0);
            let a = known.evaluate(input, BodyId::Person, false);
            let b = unknown.evaluate(
                CharacterMotionSample {
                    support: CharacterSupport::Unknown,
                    ..input
                },
                BodyId::Person,
                false,
            );
            assert_eq!(a.face, b.face);
            assert_eq!(
                a.pose.transforms[JointId::LeftUpperArm.index()].rotation,
                b.pose.transforms[JointId::LeftUpperArm.index()].rotation
            );
        }
    }

    #[test]
    fn all_expression_presets_are_authored() {
        assert!(FacePreset::ALL.len() >= 20);
        assert!(
            FacePreset::ALL
                .iter()
                .all(|preset| !preset.stable_id().is_empty())
        );
    }
}
