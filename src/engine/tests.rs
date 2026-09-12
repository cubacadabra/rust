use super::{DEFAULT_ORBIT_DISTANCE, Engine, MAX_AGENTS, SNAPSHOT_STRIDE};
use crate::character::definition::EquipmentSlot;
use crate::types::{
    Agent, AgentPhase, CharacterEntityKind, CharacterSupport, Input, LaunchPadPhase,
};
use crate::ui::{UiInsets, UiViewport};
use crate::world::{
    HazardVolume, HealthSettings, LadderAxis, LadderVolume, PhysicsSettings, RespawnMode,
    RespawnSettings, SafeZone, block_bounds,
};

#[test]
fn starts_at_the_spawn_pad() {
    let engine = Engine::new();
    assert_eq!(engine.player.position, [0.0, 0.0, 11.5]);
    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
}

#[test]
fn movement_accelerates_in_view_direction() {
    let mut engine = Engine::new();
    engine.set_input(Input {
        forward: 1.0,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    assert!(engine.player.position[2] < 11.5);
    assert!(engine.player.moving);
}

#[test]
fn idle_orbit_keeps_body_heading_and_holds_view_after_release() {
    let mut engine = Engine::new();
    for _ in 0..120 {
        engine.set_input(Input {
            look_x: 12.0,
            ..Input::default()
        });
        engine.step(1.0 / 60.0);
        let local = engine.character_motion_samples().next().unwrap();
        assert_eq!(local.facing_yaw, 0.0);
        assert_eq!(local.look_yaw, 0.0);
    }
    let requested = engine.target_yaw;
    for _ in 0..240 {
        engine.step(1.0 / 60.0);
    }
    assert!((engine.view_yaw - requested).sin().abs() < 0.001);
    assert_eq!(engine.player.facing_yaw, 0.0);
}

#[test]
fn movement_turns_body_and_stop_does_not_snap_to_camera() {
    let mut engine = Engine::new();
    for _ in 0..60 {
        engine.set_input(Input {
            strafe: 1.0,
            ..Input::default()
        });
        engine.step(1.0 / 60.0);
    }
    let facing = engine.player.facing_yaw;
    assert!((facing + std::f32::consts::FRAC_PI_2).abs() < 0.001);
    engine.set_input(Input::default());
    for _ in 0..180 {
        engine.step(1.0 / 60.0);
    }
    assert_eq!(engine.player.facing_yaw, facing);
}

#[test]
fn reconciliation_preserves_orbit_and_zoom() {
    let mut engine = Engine::new();
    engine.view_yaw = 2.0;
    engine.target_yaw = 2.2;
    let camera = engine.camera();
    engine.reconcile_player([1.0, 0.0, 2.0], 0.5);
    assert_eq!(engine.camera(), camera);
    assert_eq!(engine.target_yaw, 2.2);
    assert_eq!(engine.player.facing_yaw, 0.5);
}

#[test]
fn reconciliation_blends_position_instead_of_snapping() {
    let mut engine = Engine::new();
    let original = engine.player.position;
    engine.reconcile_player([6.0, 0.0, original[2]], 0.5);

    assert_eq!(engine.player.position, original);
    engine.step(1.0 / 60.0);

    assert!(engine.player.position[0] > original[0]);
    assert!(engine.player.position[0] < 6.0);
}

#[test]
fn zoom_is_reversible_distance_scaled_and_supports_first_person_and_wide_view() {
    let mut engine = Engine::new();
    for delta in [12.0, -12.0] {
        engine.input.zoom_delta = delta;
        engine.apply_camera_input();
    }
    assert!((engine.target_camera_distance - DEFAULT_ORBIT_DISTANCE).abs() < 0.0001);
    engine.input.zoom_delta = 100.0;
    engine.apply_camera_input();
    assert_eq!(engine.target_camera_distance, 120.0);
    engine.input.zoom_delta = -100.0;
    engine.apply_camera_input();
    assert_eq!(engine.target_camera_distance, 0.0);
    engine.input.zoom_delta = 10.0;
    engine.apply_camera_input();
    assert!(engine.target_camera_distance > 0.75);
    engine.input.zoom_delta = f32::NAN;
    engine.apply_camera_input();
    assert!(engine.target_camera_distance.is_finite());
}

#[test]
fn jump_returns_to_ground() {
    let mut engine = Engine::new();
    engine.set_input(Input {
        jump: true,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    assert!(!engine.player.grounded);
    for _ in 0..120 {
        engine.set_input(Input::default());
        engine.step(1.0 / 60.0);
    }
    assert!(engine.player.grounded);
    assert_eq!(engine.player.position[1], 0.0);
}

#[test]
fn void_death_respawns_at_the_active_checkpoint() {
    let mut engine = Engine::new();
    engine.physics = PhysicsSettings {
        ground_collision: false,
        death_y: -2.0,
        respawn_delay: 0.2,
        ..PhysicsSettings::default()
    };
    engine.obstacles.clear();
    engine.base_obstacles.clear();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.player.grounded = false;
    engine.respawn_position = [2.0, 1.0, 3.0];
    engine.checkpoint_id = "water-tower".to_owned();
    assert_eq!(engine.player_respawn_event_id(), 0);

    for _ in 0..30 {
        engine.step(1.0 / 60.0);
    }
    assert!(engine.player_dead);
    assert_eq!(engine.player_deaths, 1);
    let mut respawned = false;
    for _ in 0..60 {
        engine.step(1.0 / 60.0);
        if !engine.player_dead && engine.player.position == [2.0, 1.0, 3.0] {
            respawned = true;
            break;
        }
    }
    assert!(respawned);
    assert_eq!(engine.player_respawn_event_id(), 1);
}

#[test]
fn damage_hazard_can_kill_and_spawn_respawn_restores_health() {
    let mut engine = Engine::new();
    engine.obstacles.clear();
    engine.base_obstacles.clear();
    engine.physics.ground_collision = true;
    engine.physics.ground_y = 0.0;
    engine.health = HealthSettings {
        max: 100.0,
        start: 100.0,
    };
    engine.respawn = RespawnSettings {
        mode: RespawnMode::Spawn,
        delay: 0.1,
    };
    engine.respawn_position = [4.0, 0.0, 4.0];
    engine.checkpoint_id = "route-marker".to_owned();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.player.grounded = true;
    engine.player_health = 100.0;
    engine.player_max_health = 100.0;
    engine.hazards = vec![HazardVolume {
        id: "poison-water".to_owned(),
        kind: "damage".to_owned(),
        bounds: block_bounds([0.0, 0.25, 0.0], [6.0, 0.5, 6.0]),
        damage_per_second: 100.0,
    }];

    for _ in 0..25 {
        engine.step(0.05);
        if engine.player_dead {
            break;
        }
    }
    assert!(engine.player_dead);
    assert_eq!(engine.player_health, 0.0);

    for _ in 0..10 {
        engine.step(0.05);
        if !engine.player_dead {
            break;
        }
    }
    assert!(!engine.player_dead);
    assert_eq!(engine.player.position, [4.0, 0.0, 4.0]);
    assert_eq!(engine.player_health, 100.0);
}

#[test]
fn throttled_damage_events_report_accumulated_damage() {
    let mut engine = Engine::new();
    engine.hazards = vec![HazardVolume {
        id: "heat".to_owned(),
        kind: "damage".to_owned(),
        bounds: block_bounds([0.0, 0.25, 0.0], [6.0, 0.5, 6.0]),
        damage_per_second: 100.0,
    }];
    engine.player.position = [0.0, 0.0, 0.0];
    engine.player_health = 100.0;
    engine.player_max_health = 100.0;
    engine.player_next_damage_event_at = 1.0;

    engine.update_hazards(0.1);
    engine.update_hazards(0.1);
    assert!(engine.player_events.is_empty());

    engine.elapsed = 1.0;
    engine.update_hazards(0.1);
    assert!(matches!(
        engine.player_events.pop_front(),
        Some(crate::types::PlayerEvent::Damage { amount, .. }) if (amount - 30.0).abs() < 0.001
    ));
}

#[test]
fn safe_zone_suppresses_damage_and_heals() {
    let mut engine = Engine::new();
    engine.hazards = vec![HazardVolume {
        id: "storm".to_owned(),
        kind: "damage".to_owned(),
        bounds: block_bounds([0.0, 0.25, 0.0], [6.0, 0.5, 6.0]),
        damage_per_second: 100.0,
    }];
    engine.safe_zones = vec![SafeZone {
        id: "beacon".to_owned(),
        position: [0.0, 0.0, 0.0],
        radius: 4.0,
        heal_per_second: 20.0,
    }];
    engine.player.position = [0.0, 0.0, 0.0];
    engine.player_health = 40.0;
    engine.player_max_health = 100.0;

    engine.update_hazards(0.5);

    assert_eq!(engine.player_health, 50.0);
    assert!(
        engine
            .player_events
            .iter()
            .any(|event| matches!(event, crate::types::PlayerEvent::Heal { .. }))
    );
}

#[test]
fn ladder_converts_forward_motion_into_vertical_climbing() {
    let mut engine = Engine::new();
    engine.physics.ground_collision = false;
    engine.physics.death_y = -20.0;
    engine.obstacles = vec![block_bounds([0.0, 0.5, 0.0], [4.0, 1.0, 4.0])];
    engine.base_obstacles = engine.obstacles.clone();
    engine.player.position = [0.0, 1.0, 0.0];
    engine.player.grounded = true;
    engine.player.velocity[2] = 5.0;
    engine.view_yaw = std::f32::consts::FRAC_PI_2;
    engine.ladders = vec![LadderVolume {
        id: "test-ladder".to_owned(),
        bounds: block_bounds([0.0, 3.5, 0.0], [2.0, 5.0, 0.8]),
        axis: LadderAxis::Z,
        climb_speed: 4.0,
    }];
    engine.set_input(Input {
        climb: true,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    assert!(engine.player.climbing);
    assert!(engine.player.velocity[1] > 0.0);
    assert_eq!(engine.player.velocity[2], 0.0);
}

#[test]
fn checkpoints_must_be_reached_in_manifest_order() {
    let mut engine = Engine::new();
    engine.checkpoints = vec![
        crate::world::Checkpoint {
            id: "first".to_owned(),
            position: [0.0, 0.0, 0.0],
            radius: 1.0,
        },
        crate::world::Checkpoint {
            id: "second".to_owned(),
            position: [10.0, 0.0, 0.0],
            radius: 1.0,
        },
    ];

    engine.player.position = [10.0, 0.0, 0.0];
    engine.update_player_checkpoints();
    assert!(engine.checkpoint_id.is_empty());

    engine.player.position = [0.0, 0.0, 0.0];
    engine.update_player_checkpoints();
    assert_eq!(engine.checkpoint_id, "first");

    engine.player.position = [10.0, 0.0, 0.0];
    engine.update_player_checkpoints();
    assert_eq!(engine.checkpoint_id, "second");
}

#[test]
fn typed_motion_reports_takeoff_and_landing_events() {
    let mut engine = Engine::new();
    engine.set_input(Input {
        jump: true,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    assert_eq!(
        engine
            .character_motion_samples()
            .next()
            .expect("local sample")
            .event,
        crate::types::CharacterMotionEvent::Takeoff
    );

    let mut landing_seen = false;
    for _ in 0..120 {
        engine.set_input(Input::default());
        engine.step(1.0 / 60.0);
        if engine
            .character_motion_samples()
            .next()
            .is_some_and(|sample| sample.event == crate::types::CharacterMotionEvent::Landing)
        {
            landing_seen = true;
            break;
        }
    }
    assert!(landing_seen);
}

#[test]
fn legacy_remote_slot_generation_changes_at_replacement_boundary() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(1);
    let first = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote sample")
        .key
        .generation;
    engine.set_remote_player_count(0);
    engine.set_remote_player_count(1);
    let replacement = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("replacement sample")
        .key
        .generation;
    assert_ne!(first, replacement);
}

#[test]
fn local_npcs_are_disabled_until_authoritative() {
    let mut engine = Engine::new();
    for _ in 0..181 {
        engine.step(1.0 / 60.0);
    }
    assert!(engine.agents.is_empty());
    assert_eq!(engine.agent_count(), 0);
}

#[test]
fn remote_players_are_written_to_the_snapshot() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(1);
    engine.set_remote_player(0, [4.0, 0.0, -6.0], 0.75, true, false);
    engine.step(1.0 / 60.0);

    assert_eq!(engine.remote_player_count(), 1);
    assert_eq!(engine.agent_count(), 1);
    assert_eq!(
        &engine.snapshot[SNAPSHOT_STRIDE..SNAPSHOT_STRIDE + 3],
        &[4.0, 0.0, -6.0]
    );
    assert_eq!(engine.snapshot[SNAPSHOT_STRIDE + 3], 0.75);
    assert!(engine.snapshot[SNAPSHOT_STRIDE + 4] > 0.0);
    assert_eq!(engine.snapshot[SNAPSHOT_STRIDE + 6], -1.0);
}

#[test]
fn spawned_remote_players_get_distinct_vibrant_hoodies() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(MAX_AGENTS);

    let colors: Vec<_> = engine
        .remote_players
        .iter()
        .map(|player| player.appearance.colors.primary)
        .collect();
    assert!(colors.iter().all(|color| *color != [0.18, 0.40, 0.39, 1.0]));
    for (index, color) in colors.iter().enumerate() {
        assert!(colors[index + 1..].iter().all(|other| other != color));
    }
}

#[test]
fn snapshot_abi_fixture_preserves_entity_suffix_meanings() {
    let mut engine = Engine::new();
    engine.view_yaw = 1.25;
    engine.player.position = [2.0, 0.0, -3.0];
    engine.player.walk_cycle = 0.75;
    engine.player.grounded = true;
    engine.player.moving = true;
    engine.player.sprinting = true;
    engine.write_snapshot();

    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
    assert_eq!(
        &engine.snapshot[..SNAPSHOT_STRIDE],
        &[2.0, 0.0, -3.0, 1.25, 0.75, 1.0, 1.0, 1.0]
    );

    engine.set_remote_player_count(1);
    engine.set_remote_player(0, [4.0, 0.0, -6.0], -0.5, true, true);
    engine.write_snapshot();
    assert_eq!(
        &engine.snapshot[SNAPSHOT_STRIDE..2 * SNAPSHOT_STRIDE],
        &[4.0, 0.0, -6.0, -0.5, 0.0, 1.0, -1.0, 0.0]
    );

    let mut npc_engine = Engine::new();
    npc_engine.agents.push(Agent {
        position: [1.0, 0.0, 2.0],
        target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_index: 3,
        phase: AgentPhase::Assembling,
        spawned_at: 0.0,
        next_decision_at: 0.0,
        gather_at: 0.0,
        next_jump_at: 0.0,
        speed: 1.0,
        walk_cycle: 0.0,
        vertical_velocity: 0.0,
        grounded: true,
    });
    npc_engine.write_snapshot();
    assert_eq!(
        &npc_engine.snapshot[SNAPSHOT_STRIDE..2 * SNAPSHOT_STRIDE],
        &[1.0, 0.0, 2.0, 0.0, 0.0, 2.0, 3.0, 0.0]
    );

    engine.set_remote_player_count(50);
    assert_eq!(engine.remote_player_count(), 17);
    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
}

#[test]
fn typed_motion_keeps_local_sprint_separate_from_npc_gathering() {
    let mut engine = Engine::new();
    engine.set_input(Input {
        forward: 1.0,
        sprint: true,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    let samples: Vec<_> = engine.character_motion_samples().collect();
    let local = samples
        .iter()
        .find(|sample| sample.key.kind == CharacterEntityKind::LocalPlayer)
        .expect("local sample");
    assert!(local.moving);
    assert!(local.sprinting);
    assert_eq!(local.support, CharacterSupport::Grounded { height: 0.0 });
    assert_eq!(engine.snapshot[7], 1.0);

    engine.set_remote_player_count(1);
    let remote = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote sample");
    assert_eq!(remote.support, CharacterSupport::Unknown);
    assert!(remote.planar_velocity.is_none());
    assert!(remote.vertical_velocity.is_none());

    engine.agents.push(Agent {
        position: [1.0, 0.0, 2.0],
        target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_index: 0,
        phase: AgentPhase::Assembling,
        spawned_at: 0.0,
        next_decision_at: 0.0,
        gather_at: 0.0,
        next_jump_at: 0.0,
        speed: 1.0,
        walk_cycle: 0.0,
        vertical_velocity: 0.0,
        grounded: true,
    });
    let npc = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::LocalNpc)
        .expect("NPC sample");
    assert!(npc.moving);
    assert!(!npc.sprinting);
    assert_eq!(npc.support, CharacterSupport::Grounded { height: 0.0 });
}

#[test]
fn render_only_capacity_does_not_change_engine_capacity() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(17);
    assert_eq!(engine.remote_player_count(), 17);
    assert_eq!(engine.agent_count(), 17);
    assert_eq!(engine.snapshot().len(), 18 * SNAPSHOT_STRIDE);
}

#[test]
fn local_appearance_is_atomic_and_revisioned() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.set_local_appearance_json(
            r##"{
                "version":1,
                "body":"cuba:cat.v1",
                "face":"curious",
                "outfit":"cuba:everyday-hoodie.v1",
                "colors":{"primary":"#176b87"},
                "revision":2
            }"##,
        ),
        1
    );
    assert_eq!(engine.player_appearance.body, crate::character::BodyId::Cat);
    assert_eq!(engine.player_appearance.revision, 2);
    assert!((engine.player_appearance.colors.primary[0] - 23.0 / 255.0).abs() < 1e-6);

    assert_eq!(
        engine
            .set_local_appearance_json(r##"{"version":1,"body":"cuba:dragon.v1","revision":1}"##,),
        2
    );
    assert_eq!(engine.player_appearance.body, crate::character::BodyId::Cat);
}

#[test]
fn local_appearance_can_switch_between_authored_person_bases_repeatedly() {
    let mut engine = Engine::new();
    for (revision, base) in [
        (1, "cuba:base/person-02.v1"),
        (2, "cuba:base/person.v1"),
        (3, "cuba:base/person-02.v1"),
    ] {
        let appearance = format!(
            r#"{{"version":1,"body":"cuba:person.v1","equipment":{{"base":"{base}"}},"revision":{revision}}}"#
        );
        assert_eq!(engine.set_local_appearance_json(&appearance), 1);
        let selected = engine
            .player_appearance
            .equipment
            .iter()
            .find(|item| item.slot == EquipmentSlot::Base)
            .unwrap();
        assert_eq!(selected.asset_id, base);
    }
}

#[test]
fn v2_morph_loadouts_project_into_the_preview_renderer_contract() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.set_local_appearance_json(
            r##"{"version":2,"base":"cuba:base/person-02.v1","parts":["cuba:hair/shag.v1","cuba:everyday-hoodie.v1"],"face":"cuba:face/surprised.v1","parameters":{},"revision":1}"##,
        ),
        1
    );
    assert_eq!(
        engine.player_appearance.body,
        crate::character::BodyId::PersonNonbinary
    );
    assert_eq!(
        engine.player_appearance.face,
        crate::character::FacePreset::Surprised
    );
    assert_eq!(
        engine.player_appearance.outfit,
        crate::character::OutfitId::EverydayHoodie
    );
    assert!(engine.player_appearance.equipment.iter().any(|item| {
        item.slot == EquipmentSlot::Base && item.asset_id == "cuba:base/person-02.v1"
    }));
}

#[test]
fn versioned_remote_roster_preserves_identity_through_reorder() {
    let mut engine = Engine::new();
    let first = r##"{
        "version":1,"sequence":1,"worldId":"lobby","players":[
        {"id":"account:alice","username":"Alice","generation":7,"position":[1,0,2],"yaw":0.2,
         "moving":true,"appearance":{"version":1,"body":"cuba:cat.v1","revision":4}},
        {"id":"account:bob","generation":9,"position":[-1,0,2],"yaw":-0.2,"moving":false}
    ]}"##;
    assert!(engine.apply_remote_update_json(first));
    let first_samples: Vec<_> = engine.character_motion_samples().collect();
    let alice = first_samples
        .iter()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 1.0
        })
        .expect("alice sample");
    let alice_identity = alice.key.identity;
    assert_eq!(alice.key.generation, 7);
    assert_eq!(engine.remote_players[0].display_name, "Alice");
    assert_eq!(
        engine
            .remote_appearance(alice.key)
            .expect("alice appearance")
            .body,
        crate::character::BodyId::Cat
    );

    let second = r##"{
        "version":1,"sequence":2,"worldId":"lobby","players":[
        {"id":"account:bob","generation":9,"position":[-2,0,2],"yaw":-0.3,"moving":true},
        {"id":"account:alice","generation":7,"position":[3,0,2],"yaw":0.4,"moving":false}
    ]}"##;
    assert!(engine.apply_remote_update_json(second));
    let alice_after = engine
        .character_motion_samples()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 3.0
        })
        .expect("reordered alice sample");
    assert_eq!(alice_after.key.identity, alice_identity);
    assert_eq!(alice_after.key.generation, 7);
    assert_eq!(alice_after.appearance_revision, 4);
    assert_eq!(engine.remote_update_sequence(), 2);
}

#[test]
fn remote_updates_reject_stale_sequences_and_deduplicate_emotes() {
    let mut engine = Engine::new();
    let wave = r##"{
        "version":1,"sequence":5,"players":[
        {"id":"account:wave","generation":3,"position":[0,0,0],"yaw":0,
         "emote":"wave","emoteSequence":11}
    ]}"##;
    assert!(engine.apply_remote_update_json(wave));
    assert_eq!(
        engine
            .character_motion_samples()
            .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
            .expect("wave sample")
            .emote,
        crate::types::CharacterEmote::Wave
    );

    let duplicate_emote = r##"{
        "version":1,"sequence":6,"players":[
        {"id":"account:wave","generation":3,"position":[0,0,0],"yaw":0,
         "emote":"wave","emoteSequence":11}
    ]}"##;
    assert!(engine.apply_remote_update_json(duplicate_emote));
    assert_eq!(
        engine
            .character_motion_samples()
            .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
            .expect("deduplicated sample")
            .emote,
        crate::types::CharacterEmote::None
    );

    let stale = r##"{
        "version":1,"sequence":4,"players":[]
    }"##;
    assert!(!engine.apply_remote_update_json(stale));
    assert_eq!(engine.remote_update_status(), 2);
    assert_eq!(engine.remote_player_count(), 1);
}

#[test]
fn remote_duplicate_motion_sequence_does_not_replace_newer_motion() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":10,"players":[
            {"id":"account:motion","generation":1,"position":[5,0,0],"yaw":0,
             "motionSequence":100}
        ]}"##,
    ));
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":11,"players":[
            {"id":"account:motion","generation":1,"position":[50,0,0],"yaw":1,
             "motionSequence":100}
        ]}"##,
    ));

    let sample = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote motion sample");
    assert_eq!(sample.position, [5.0, 0.0, 0.0]);
    assert_eq!(sample.facing_yaw, 0.0);
}

#[test]
fn compact_remote_motion_batch_updates_only_newer_motion() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:typed-motion","generation":2,"position":[5,0,0],"yaw":0,
             "motionSequence":100}
        ]}"##,
    ));
    let identity = crate::engine::identity::stable_identity("account:typed-motion");
    let mut batch = vec![0_u8; 8 + crate::engine::identity::REMOTE_MOTION_RECORD_BYTES];
    batch[0..4].copy_from_slice(&1_u32.to_le_bytes());
    batch[4..8].copy_from_slice(&1_u32.to_le_bytes());
    batch[8..16].copy_from_slice(&identity.to_le_bytes());
    batch[16..20].copy_from_slice(&2_u32.to_le_bytes());
    batch[20..28].copy_from_slice(&100_u64.to_le_bytes());
    batch[28..32].copy_from_slice(&50.0_f32.to_le_bytes());
    batch[32..36].copy_from_slice(&0.0_f32.to_le_bytes());
    batch[36..40].copy_from_slice(&0.0_f32.to_le_bytes());
    batch[40..44].copy_from_slice(&1.0_f32.to_le_bytes());
    batch[44..48].copy_from_slice(&1_u32.to_le_bytes());
    assert!(engine.apply_remote_motion_batch(&batch));
    let duplicate = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("typed motion sample");
    assert_eq!(duplicate.position, [5.0, 0.0, 0.0]);

    batch[20..28].copy_from_slice(&101_u64.to_le_bytes());
    assert!(engine.apply_remote_motion_batch(&batch));
    let newer = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("newer typed motion sample");
    assert_eq!(newer.position, [50.0, 0.0, 0.0]);
}

#[test]
fn reconnect_can_hydrate_cached_appearance_without_reusing_motion_state() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":20,"players":[
            {"id":"account:reconnect","generation":4,"position":[8,0,1],"yaw":0.5,
             "appearance":{"version":1,"body":"cuba:dragon.v1","revision":6}}
        ]}"##,
    ));
    let old = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("initial remote");
    assert_eq!(old.key.generation, 4);
    engine.reset_remote_session();

    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:reconnect","generation":5,"position":[-2,0,1],"yaw":-0.5}
        ]}"##,
    ));
    let restored = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("reconnected remote");
    assert_eq!(restored.key.identity, old.key.identity);
    assert_eq!(restored.key.generation, 5);
    assert_eq!(restored.appearance_revision, 6);
    assert_eq!(
        engine
            .remote_appearance(restored.key)
            .expect("cached appearance")
            .body,
        crate::character::BodyId::Dragon
    );
}

#[test]
fn reconnect_rejects_conflicting_same_revision_appearance_from_known_identity() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:appearance-reconnect","generation":4,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:dragon.v1","revision":6}}
        ]}"##,
    ));
    engine.reset_remote_session();

    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:appearance-reconnect","generation":5,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":6}}
        ]}"##,
    ));
    let restored = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("reconnected remote");
    assert_eq!(engine.remote_update_status(), 3);
    assert_eq!(restored.appearance_revision, 6);
    assert_eq!(
        engine
            .remote_appearance(restored.key)
            .expect("cached appearance")
            .body,
        crate::character::BodyId::Dragon
    );
}

#[test]
fn remote_identity_cache_evicts_least_recently_seen_entry() {
    let mut engine = Engine::new();
    let appearance = crate::character::definition::CharacterAppearance::default();
    for index in 0..MAX_AGENTS {
        engine.cache_remote_appearance(format!("account:{index}"), appearance.clone());
    }
    engine.cache_remote_appearance("account:0".to_owned(), appearance.clone());
    engine.cache_remote_appearance("account:new".to_owned(), appearance);

    assert!(engine.remote_identity_cache.contains_key("account:0"));
    assert!(!engine.remote_identity_cache.contains_key("account:1"));
    assert!(engine.remote_identity_cache.contains_key("account:new"));
}

#[test]
fn first_appearance_after_appearance_omission_is_accepted() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:first-appearance","generation":1,"position":[0,0,0],"yaw":0}
        ]}"##,
    ));
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":2,"players":[
            {"id":"account:first-appearance","generation":1,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":0}}
        ]}"##,
    ));
    let sample = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote with first appearance");
    assert_eq!(
        engine
            .remote_appearance(sample.key)
            .expect("accepted appearance")
            .body,
        crate::character::BodyId::Cat
    );
}

#[test]
fn remote_identity_is_hidden_in_other_world_and_returns_with_same_appearance() {
    let mut engine = Engine::new();
    engine.world_ids = vec!["lobby".to_owned(), "arena".to_owned()];
    engine.worlds = vec![
        crate::world::RuntimeWorld::default(),
        crate::world::RuntimeWorld::default(),
    ];
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"worldId":"lobby","players":[
            {"id":"account:world","generation":8,"position":[1,0,1],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":3}}
        ]}"##,
    ));
    let before = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("lobby remote");
    assert!(engine.start_world(1));
    assert!(
        !engine
            .character_motion_samples()
            .any(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
    );
    assert!(engine.start_world(0));
    let after = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("returning lobby remote");
    assert_eq!(before.key.identity, after.key.identity);
    assert_eq!(before.key.generation, after.key.generation);
    assert_eq!(after.appearance_revision, 3);
}

#[test]
fn remote_missing_content_keeps_a_usable_bundled_fallback() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:missing","generation":1,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:unreleased.v9",
             "outfit":"cuba:missing-coat.v1","revision":2}},
            {"id":"account:legacy","generation":1,"position":[1,0,0],"yaw":0}
        ]}"##,
    ));
    assert_eq!(engine.remote_update_status(), 3);
    let samples: Vec<_> = engine.character_motion_samples().collect();
    let missing = samples
        .iter()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 0.0
        })
        .expect("missing-content remote");
    assert_eq!(
        engine
            .remote_appearance(missing.key)
            .expect("fallback appearance")
            .body,
        crate::character::BodyId::Person
    );
}

#[test]
fn occupied_launch_pad_counts_down_and_emits_event() {
    let mut engine = Engine::new();
    engine.player.position = [-10.0, 0.0, -3.0];

    engine.step(1.0 / 60.0);
    assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Countdown.code());
    assert_eq!(engine.launch_pad_occupants(0), 1);
    assert!(engine.launch_pad_seconds(0) > 7.9);

    for _ in 0..480 {
        engine.step(1.0 / 60.0);
    }

    assert_eq!(engine.launch_event_id, 1);
    assert_eq!(engine.last_launch_pad, 0);
    assert_eq!(engine.last_launch_occupants, 1);
    assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Launched.code());
}

#[test]
fn empty_launch_pad_cancels_countdown() {
    let mut engine = Engine::new();
    engine.player.position = [-10.0, 0.0, -3.0];
    engine.step(1.0 / 60.0);
    engine.player.position = [0.0, 0.0, 11.5];
    engine.step(1.0 / 60.0);

    assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Idle.code());
    assert_eq!(engine.launch_event_id, 0);
}

#[test]
fn launch_pad_registry_accepts_world_defined_counts() {
    let mut engine = Engine::new();
    engine.set_launch_pad_count(1);
    engine.set_launch_pad(0, 4.0, -2.0, 2.0, 4.0);

    assert_eq!(engine.launch_pad_count(), 1);
    engine.player.position = [4.0, 0.0, -2.0];
    engine.step(1.0 / 60.0);

    assert_eq!(engine.launch_pad_occupants(0), 1);
    assert_eq!(engine.launch_pad_phase(0), LaunchPadPhase::Countdown.code());
}

#[test]
fn entering_session_keeps_only_players_from_launched_pad() {
    let mut engine = Engine::new();
    engine.agents.push(Agent {
        position: [-10.0, 0.0, -3.0],
        target: crate::math::Vec2 { x: -10.0, z: -3.0 },
        meeting_target: crate::math::Vec2 { x: -10.0, z: -3.0 },
        meeting_index: 0,
        phase: AgentPhase::Assembled,
        spawned_at: 0.0,
        next_decision_at: 0.0,
        gather_at: 0.0,
        next_jump_at: 0.0,
        speed: 1.0,
        walk_cycle: 0.0,
        vertical_velocity: 0.0,
        grounded: true,
    });
    engine.agents.push(Agent {
        meeting_index: 1,
        ..engine.agents[0]
    });
    engine.player.position = [-10.0, 0.0, -3.0];

    let player_count = engine.enter_session(0, [0.0, 0.0, 8.0]);

    assert_eq!(player_count, 2);
    assert_eq!(engine.agents.len(), 1);
    assert_eq!(engine.launch_pad_count(), 0);
    assert_eq!(engine.player.position, [0.0, 0.0, 8.0]);
}

#[test]
fn registered_world_route_transitions_selected_player_in_engine() {
    let mut engine = Engine::new();
    engine.set_world_count(2);
    engine.set_world_spawn(0, [0.0, 0.0, 6.0]);
    engine.set_world_launch_pad_count(0, 1);
    engine.set_world_launch_pad(0, 0, 4.0, -2.0, 2.0, 0.1);
    engine.set_world_launch_destination(0, 0, 1);
    engine.set_world_spawn(1, [0.0, 0.0, 8.0]);
    engine.set_world_obstacle_count(1, 1);
    engine.set_world_obstacle(1, 0, [0.0, 1.0, -7.0], [4.0, 2.0, 4.0]);
    assert!(engine.start_world(0));

    engine.player.position = [4.0, 0.0, -2.0];
    for _ in 0..8 {
        engine.step(1.0 / 60.0);
    }

    assert_eq!(engine.active_world(), 1);
    assert_eq!(engine.world_event_id(), 1);
    assert_eq!(engine.last_world_source_pad(), 0);
    assert_eq!(engine.last_world_destination(), 1);
    assert_eq!(engine.player.position, [0.0, 0.0, 8.0]);
    assert_eq!(engine.launch_pad_count(), 0);
    assert_eq!(engine.obstacles.len(), 1);
}

#[test]
fn world_transitions_update_ui_visibility_context() {
    let manifest = r#"{
        "startWorld":"lobby",
        "world":{"spawn":[0,0,0]},
        "worlds":{"real-game":{"world":{"spawn":[0,0,0]}}}
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();
    assert!(engine.load_package_buffer());
    engine.set_ui_viewport(UiViewport {
        width: 390.0,
        height: 844.0,
        scale: 1.0,
        safe_area: UiInsets::default(),
    });
    assert!(engine.set_ui_document(
        r#"{"nodes":[{"id":"build","kind":"button","visibleIn":["real-game"]}]}"#
    ));
    assert!(
        !engine
            .ui
            .borrow_mut()
            .frame()
            .nodes
            .iter()
            .any(|node| node.id == "build")
    );

    assert!(engine.start_world(1));
    assert!(
        engine
            .ui
            .borrow_mut()
            .frame()
            .nodes
            .iter()
            .any(|node| node.id == "build")
    );
}

#[test]
fn disabled_lobby_starts_directly_in_the_shared_world() {
    let manifest = r#"{
        "lobby": false,
        "startWorld": "lobby",
        "launch": {"destinationWorld":"real-game"},
        "worlds":{"real-game":{"world":{"spawn":[0,0,8]}}}
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();

    assert!(engine.load_package_buffer());
    assert_eq!(engine.world_ids[engine.active_world()], "real-game");
    assert_eq!(engine.player.position, [0.0, 0.0, 8.0]);
}

#[test]
fn luau_can_switch_a_lobby_package_to_direct_startup() {
    let manifest = r#"{
        "startWorld": "lobby",
        "launch": {"destinationWorld":"real-game"},
        "worlds":{"real-game":{"world":{"spawn":[0,0,8]}}}
    }"#;
    let script = r#"
        local game = {}
        function game.on_start(api)
            api.lobby:set_enabled(false)
        end
        return game
    "#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();
    assert!(engine.load_package_buffer());
    engine.script_buffer = script.as_bytes().to_vec();

    assert!(engine.load_script_buffer());
    assert_eq!(engine.world_ids[engine.active_world()], "real-game");
}

#[test]
fn luau_effect_commands_enter_the_bounded_engine_runtime() {
    let script = r#"
        local game = {}
        function game.on_start(api)
            api.effects:set_state("checkpoint-a", "open")
            api.effects:play("finish-flash", { position = { 2, 1, -4 } })
        end
        return game
    "#;
    let mut engine = Engine::new();
    engine.script_buffer = script.as_bytes().to_vec();

    assert!(engine.load_script_buffer());
    engine.step(0.25);

    assert_eq!(
        engine
            .effects
            .states
            .get(&(engine.active_world, "checkpoint-a".to_owned()))
            .map(String::as_str),
        Some("open")
    );
    let instance = engine.effects.instances.back().expect("one-shot effect");
    assert_eq!(instance.template, "finish-flash");
    assert_eq!(instance.position, [2.0, 1.0, -4.0]);
    assert_eq!(instance.world, engine.active_world);
    assert_eq!(instance.started_at, engine.elapsed);
}

#[test]
fn portals_enter_and_exit_the_immersive_settings_world() {
    let manifest = r#"{
        "startWorld":"lobby",
        "settingsRoom":{
            "worldId":"settings",
            "usernameStationPosition":[0,0,-5],
            "interactionRadius":3
        },
        "world":{"spawn":[0,0,0]},
        "portals":[{
            "position":[4,0,0],
            "radius":1,
            "destinationWorld":"settings",
            "destinationSpawn":[0,0,6]
        }],
        "worlds":{
            "settings":{
                "world":{"spawn":[0,0,6]},
                "portals":[{
                    "position":[0,0,9],
                    "radius":1,
                    "destinationWorld":"lobby",
                    "destinationSpawn":[3,0,0]
                }]
            }
        }
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();
    assert!(engine.load_package_buffer());

    engine.player.position = [4.0, 0.0, 0.0];
    engine.step(1.0 / 60.0);
    assert_eq!(engine.world_ids[engine.active_world()], "settings");
    assert_eq!(engine.player.position, [0.0, 0.0, 6.0]);
    assert_eq!(engine.settings_room_state(), 1);

    engine.player.position = [0.0, 0.0, -5.0];
    assert_eq!(engine.settings_room_state(), 2);

    engine.portal_cooldown_until = 0.0;
    engine.player.position = [0.0, 0.0, 9.0];
    engine.step(1.0 / 60.0);
    assert_eq!(engine.world_ids[engine.active_world()], "lobby");
    assert_eq!(engine.player.position, [3.0, 0.0, 0.0]);
    assert_eq!(engine.settings_room_state(), 0);
}
