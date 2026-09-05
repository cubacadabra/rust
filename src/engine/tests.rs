use super::{Engine, SNAPSHOT_STRIDE};
use crate::types::{Agent, AgentPhase, Input, LaunchPadPhase};
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
fn render_only_capacity_does_not_change_engine_capacity() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(17);
    assert_eq!(engine.remote_player_count(), 17);
    assert_eq!(engine.agent_count(), 17);
    assert_eq!(engine.snapshot().len(), 18 * SNAPSHOT_STRIDE);
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
