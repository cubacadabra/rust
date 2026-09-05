use super::{Engine, SNAPSHOT_STRIDE};
use crate::types::{
    Agent, AgentPhase, CharacterEntityKind, CharacterSupport, Input, LaunchPadPhase,
};
use crate::ui::{UiInsets, UiViewport};

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
        engine.set_local_appearance_json(
            r##"{"version":1,"body":"cuba:dragon.v1","revision":1}"##,
        ),
        2
    );
    assert_eq!(engine.player_appearance.body, crate::character::BodyId::Cat);
}

#[test]
fn versioned_remote_roster_preserves_identity_through_reorder() {
    let mut engine = Engine::new();
    let first = r##"{
        "version":1,"sequence":1,"worldId":"lobby","players":[
        {"id":"account:alice","generation":7,"position":[1,0,2],"yaw":0.2,
         "moving":true,"appearance":{"version":1,"body":"cuba:cat.v1","revision":4}},
        {"id":"account:bob","generation":9,"position":[-1,0,2],"yaw":-0.2,"moving":false}
    ]}"##;
    assert!(engine.apply_remote_update_json(first));
    let first_samples: Vec<_> = engine.character_motion_samples().collect();
    let alice = first_samples
        .iter()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 1.0)
        .expect("alice sample");
    let alice_identity = alice.key.identity;
    assert_eq!(alice.key.generation, 7);
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
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 3.0)
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
    assert!(!engine
        .character_motion_samples()
        .any(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer));
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
