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
fn package_terrain_drives_world_collision_and_can_replace_the_flat_floor() {
    let manifest = r#"{
        "sdkVersion":"0.4.0",
        "lobby": false,
        "startWorld": "maze",
        "worlds": {
            "maze": {
                "world": {"spawn":[0,1,0]},
                "terrain": {
                    "cellSize": 1,
                    "hideDefaultGround": true,
                    "operations": [
                        {"operation":"fill","shape":"block","position":[0,-1,0],"size":[20,2,20],"material":"ground"},
                        {"operation":"fill","shape":"block","position":[0,3,5],"size":[8,6,1],"material":"grass"},
                        {"operation":"carve","shape":"ball","position":[0,3,5],"radius":1}
                    ]
                }
            }
        }
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();

    assert!(engine.load_package_buffer());
    let terrain = engine
        .terrain
        .as_ref()
        .expect("the active world has terrain");
    assert!(terrain.signed_distance([0.0, -1.0, 0.0]) < 0.0);
    assert!(terrain.signed_distance([0.0, 3.0, 5.0]) > 0.0);
    assert!(!engine.physics.ground_collision);
}

#[test]
fn non_collidable_world_blocks_render_without_becoming_obstacles() {
    let manifest = r#"{
        "startWorld":"world",
        "worlds":{"world":{
            "world":{"spawn":[0,2,0]},
            "blocks":[
                {"position":[0,1,0],"size":[4,2,4],"collidable":false},
                {"position":[8,1,0],"size":[4,2,4]}
            ]
        }}
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();
    assert!(engine.load_package_buffer());
    assert_eq!(engine.obstacles.len(), 1);
}

#[test]
fn package_loading_rejects_unsupported_terrain_materials() {
    let manifest = r#"{
        "sdkVersion":"0.4.0",
        "lobby": false,
        "startWorld": "maze",
        "worlds": {
            "maze": {
                "terrain": {
                    "operations": [
                        {"shape":"block","position":[0,0,0],"size":[4,4,4],"material":"lava"}
                    ]
                }
            }
        }
    }"#;
    let mut engine = Engine::new();
    engine.package_buffer = manifest.as_bytes().to_vec();
    assert!(!engine.load_package_buffer());
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
fn luau_world_api_queues_package_transition_after_callback() {
    let manifest = r#"
        {
            "sdkVersion":"0.5.0",
            "startWorld":"lobby",
            "world":{"spawn":[0,0,0]},
            "worlds":{"maze":{"world":{"spawn":[4,0,8]}}}
        }
    "#;
    let script = r#"
        local game = {}
        function game.on_start(api)
            api.world:enter("maze")
        end
        function game.on_tick(api, delta)
            api.lobby:set_status(api.world:get_id())
        end
        return game
    "#;
    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert!(engine.load_script_source(script));
    assert_eq!(engine.active_world_id(), Some("lobby"));

    engine.step(1.0 / 60.0);
    assert_eq!(engine.active_world_id(), Some("maze"));
    assert_eq!(engine.player.position, [4.0, 0.0, 8.0]);

    engine.step(1.0 / 60.0);
    assert_eq!(
        engine
            .script
            .as_ref()
            .expect("world script should remain loaded")
            .state()
            .borrow()
            .lobby_status,
        "maze"
    );
}

#[test]
fn authored_world_camera_applies_on_load_entry_and_reset() {
    let manifest = r#"
        {
            "sdkVersion":"0.5.0",
            "startWorld":"lobby",
            "world":{"spawn":[0,0,0],"camera":{"yaw":1.25,"pitch":0.4,"distance":18}},
            "worlds":{"room":{"world":{"spawn":[4,0,8],"camera":{"yaw":-2,"pitch":-0.3,"distance":12}}}}
        }
    "#;
    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert_eq!(engine.camera(), [1.25, 0.4, 18.0]);

    engine.view_yaw = 0.0;
    engine.view_pitch = 0.0;
    engine.camera_distance = 2.0;
    engine.reset_view();
    assert_eq!(engine.camera(), [1.25, 0.4, 18.0]);

    engine.view_yaw = 0.2;
    engine.view_pitch = -0.2;
    engine.target_yaw = 0.3;
    engine.target_pitch = -0.1;
    engine.camera_distance = 7.0;
    engine.target_camera_distance = 8.0;
    let snapshot = engine.capture_snapshot().expect("camera snapshot");
    engine.reset_view();
    engine
        .restore_snapshot(&snapshot)
        .expect("camera snapshot should restore");
    assert_eq!(engine.camera(), [0.2, -0.2, 7.0]);

    engine.script = None;
    assert!(engine.start_world_by_id("room"));
    assert_eq!(engine.camera(), [-2.0, -0.3, 12.0]);
}

#[test]
fn luau_world_travel_notifies_once_and_spawn_observes_destination_id() {
    let manifest = r#"
        {
            "startWorld":"lobby",
            "world":{"spawn":[0,0,0]},
            "worlds":{"room":{"world":{"spawn":[4,0,8]}}}
        }
    "#;
    let script = r#"
        local game = {}
        function game.on_start(api)
            api.world:enter("room")
        end
        function game.on_tick(api, delta)
            api.lobby:set_status("tick:" .. api.world:get_id())
        end
        function game.on_player_event(api, event)
            if event.kind == "spawn" then
                api.session:start("spawn:" .. api.world:get_id(), {})
            end
        end
        return game
    "#;
    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert!(engine.load_script_source(script));
    engine.input.forward = 1.0;
    engine.input.strafe = -1.0;
    engine.input.sprint = true;
    engine.input.jump = true;

    engine.step(1.0 / 60.0);
    assert_eq!(engine.active_world_id(), Some("room"));
    assert_eq!(engine.world_event_id(), 1);
    assert_eq!(engine.last_world_destination(), 1);
    assert_eq!(engine.player.position, [4.0, 0.0, 8.0]);
    assert_eq!(engine.player.velocity, [0.0; 3]);
    assert!(!engine.player.moving);
    assert!(!engine.player.sprinting);
    assert_eq!(engine.input.forward, 0.0);
    assert_eq!(engine.input.strafe, 0.0);
    assert!(!engine.input.sprint);
    assert!(!engine.input.jump);

    engine.step(1.0 / 60.0);
    let script_state = engine
        .script
        .as_ref()
        .expect("world script should remain loaded")
        .state();
    let script_state = script_state.borrow();
    assert_eq!(script_state.lobby_status, "tick:room");
    assert_eq!(script_state.session_name.as_deref(), Some("spawn:room"));
    assert_eq!(engine.world_event_id(), 1);
}

#[test]
fn luau_world_api_rejects_unknown_world_without_changing_world() {
    let manifest = r#"
        {
            "startWorld":"lobby",
            "world":{"spawn":[0,0,0]},
            "worlds":{"maze":{"world":{"spawn":[4,0,8]}}}
        }
    "#;
    let script = r#"
        local game = {}
        function game.on_tick(api, delta)
            api.world:enter("missing")
        end
        return game
    "#;
    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert!(engine.load_script_source(script));
    engine.step(1.0 / 60.0);

    assert_eq!(engine.active_world_id(), Some("lobby"));
    assert_eq!(engine.player.position, [0.0, 0.0, 0.0]);
    assert!(
        engine
            .last_script_error()
            .is_some_and(|error| error.contains("cannot find package world"))
    );
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
