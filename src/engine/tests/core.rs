#[test]
fn starts_at_the_spawn_pad() {
    let engine = Engine::new();
    assert_eq!(engine.player.position, [0.0, 0.0, 11.5]);
    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
}

#[cfg(debug_assertions)]
#[test]
fn debug_script_teleport_moves_the_player_to_the_requested_world() {
    let manifest = r#"{
        "startWorld":"lobby",
        "worlds":{
            "island-3":{"world":{"spawn":[0,0,0]}}
        }
    }"#;
    let script = r#"
        local game = {}
        function game.on_start(api)
            api.ui:set_document({
                nodes = {
                    {
                        id = "skip",
                        kind = "button",
                        action = "debug.skip",
                        layout = { width = 120, height = 48, offset = { 0, 140 } },
                    },
                },
            })
        end
        function game.on_ui_event(api, event)
            if event.action == "debug.skip" then
                api.debug:teleport_to("island-3", { 4, 0, 6 }, 1.25)
            end
        end
        return game
    "#;

    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert!(engine.load_script_source(script));
    engine.set_ui_viewport(UiViewport {
        width: 390.0,
        height: 844.0,
        scale: 1.0,
        safe_area: UiInsets::default(),
    });
    assert!(
        engine
            .ui
            .borrow_mut()
            .frame()
            .nodes
            .iter()
            .any(|node| node.id == "skip")
    );
    let skip = engine
        .ui
        .borrow_mut()
        .frame()
        .nodes
        .iter()
        .find(|node| node.id == "skip")
        .cloned()
        .expect("skip node should have a frame");
    let x = skip.rect.x + skip.rect.width / 2.0;
    let y = skip.rect.y + skip.rect.height / 2.0;
    assert!(engine.ui_pointer_event(1, 0, x, y));
    assert!(engine.ui_pointer_event(1, 2, x, y));
    engine.step(1.0 / 60.0);

    assert_eq!(engine.active_world_id(), Some("island-3"));
    assert_eq!(engine.player.position, [4.0, 0.0, 6.0]);
    assert_eq!(engine.player.facing_yaw, 1.25);
}

#[cfg(debug_assertions)]
#[test]
fn debug_script_teleport_preserves_portal_progression() {
    let manifest = r#"{
        "startWorld":"island-1",
        "worlds":{
            "island-1":{
                "world":{"spawn":[0,0,0]},
                "portals":[{
                    "position":[4,0,6],
                    "radius":2,
                    "destinationWorld":"island-2",
                    "destinationSpawn":[0,0,0]
                }]
            },
            "island-2":{"world":{"spawn":[0,0,0]}}
        }
    }"#;
    let script = r#"
        local pending_island_advance = false
        local current_island = 1
        local game = {}

        function game.on_start(api)
            api.ui:set_document({
                nodes = {
                    {
                        id = "skip",
                        kind = "button",
                        action = "debug.skip",
                        layout = { width = 120, height = 48, offset = { 0, 140 } },
                    },
                },
            })
        end

        function game.on_ui_event(api, event)
            if event.action == "debug.skip" then
                pending_island_advance = true
                api.debug:teleport_to("island-1", { 4, 0, 6 }, 0)
            end
        end

        function game.on_player_event(api, event)
            if event.kind == "spawn" and pending_island_advance then
                current_island += 1
                pending_island_advance = false
                api.lobby:set_status("ISLAND " .. tostring(current_island))
            end
        end

        return game
    "#;

    let mut engine = Engine::new();
    assert!(engine.load_package_source(manifest));
    assert!(engine.load_script_source(script));
    engine.set_ui_viewport(UiViewport {
        width: 390.0,
        height: 844.0,
        scale: 1.0,
        safe_area: UiInsets::default(),
    });
    let skip = engine
        .ui
        .borrow_mut()
        .frame()
        .nodes
        .iter()
        .find(|node| node.id == "skip")
        .cloned()
        .expect("skip node should have a frame");
    let x = skip.rect.x + skip.rect.width / 2.0;
    let y = skip.rect.y + skip.rect.height / 2.0;
    assert!(engine.ui_pointer_event(1, 0, x, y));
    assert!(engine.ui_pointer_event(1, 2, x, y));
    engine.step(1.0 / 60.0);
    assert_eq!(engine.active_world_id(), Some("island-2"));

    engine.step(1.0 / 60.0);
    assert_eq!(engine.active_world_id(), Some("island-2"));
    assert_eq!(
        engine
            .script
            .as_ref()
            .expect("progression script should be loaded")
            .state()
            .borrow()
            .lobby_status,
        "ISLAND 2"
    );
}

#[test]
fn engine_snapshot_round_trip_continues_deterministically() {
    let mut uninterrupted = Engine::new();
    for _ in 0..45 {
        uninterrupted.set_input(Input {
            forward: 0.8,
            strafe: -0.2,
            sprint: true,
            jump: false,
            climb: false,
            look_x: 0.0,
            look_y: 0.0,
            zoom_delta: 0.0,
        });
        uninterrupted.step(1.0 / 60.0);
    }
    let encoded = uninterrupted.capture_snapshot_json().unwrap();

    let mut restored = Engine::new();
    restored.restore_snapshot_json(&encoded).unwrap();
    for _ in 0..45 {
        restored.set_input(Input {
            forward: 0.8,
            strafe: -0.2,
            sprint: true,
            jump: false,
            climb: false,
            look_x: 0.0,
            look_y: 0.0,
            zoom_delta: 0.0,
        });
        uninterrupted.step(1.0 / 60.0);
        restored.step(1.0 / 60.0);
    }

    assert_eq!(restored.state_hash(), uninterrupted.state_hash());
    assert_eq!(
        restored.capture_snapshot().unwrap(),
        uninterrupted.capture_snapshot().unwrap()
    );
}

#[test]
fn engine_snapshot_rejects_incompatible_content() {
    let mut source = Engine::new();
    source.package_buffer = b"package-a".to_vec();
    let snapshot = source.capture_snapshot().unwrap();
    let mut target = Engine::new();
    target.package_buffer = b"package-b".to_vec();

    assert!(matches!(
        target.restore_snapshot(&snapshot),
        Err(crate::engine::snapshot::SnapshotError::ContentMismatch { .. })
    ));
}

#[test]
fn engine_snapshot_restores_explicit_luau_state() {
    let manifest = r#"{
        "id": "snapshot-test",
        "lobby": false,
        "startWorld": "arena",
        "worlds": { "arena": { "world": { "spawn": [0, 0, 8] } } }
    }"#;
    let script = r#"
        local score = 0
        return {
            on_tick = function(api)
                score = score + 1
                api.lobby:set_status("score:" .. score)
            end,
            on_save = function(_api)
                return { score = score }
            end,
            on_restore = function(_api, state)
                score = state.score
            end,
        }
    "#;

    let mut uninterrupted = Engine::new();
    assert!(uninterrupted.load_package_source(manifest));
    assert!(uninterrupted.load_script_source(script));
    for _ in 0..12 {
        uninterrupted.step(1.0 / 60.0);
    }
    let snapshot = uninterrupted.capture_snapshot_json().unwrap();

    let mut restored = Engine::new();
    assert!(restored.load_package_source(manifest));
    assert!(restored.load_script_source(script));
    restored.restore_snapshot_json(&snapshot).unwrap();
    for _ in 0..12 {
        uninterrupted.step(1.0 / 60.0);
        restored.step(1.0 / 60.0);
    }

    assert_eq!(restored.state_hash(), uninterrupted.state_hash());
    assert_eq!(restored.last_script_error(), None);
}

#[test]
fn engine_owns_a_generic_data_model_with_an_observable_mutation_path() {
    let mut engine = Engine::new();
    let root = engine.data_model().root();
    assert_eq!(engine.data_model().entity(root).unwrap().name, "game");
    let mut cursor = engine.data_model().subscribe();

    let beacon = engine
        .data_model_mut()
        .create_entity(
            "Beacon",
            "Node 1",
            Some(root),
            crate::data_model::MutationSource::Script,
        )
        .unwrap();
    engine
        .data_model_mut()
        .set_property(
            beacon,
            "team",
            serde_json::json!("red"),
            crate::data_model::MutationSource::Script,
        )
        .unwrap();

    let changes = engine.data_model().changes_since(&mut cursor).unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].source, crate::data_model::MutationSource::Script);
    assert_eq!(
        engine.data_model().get_property(beacon, "team").unwrap(),
        Some(&serde_json::json!("red"))
    );
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
fn reset_view_restores_the_default_third_person_orbit() {
    let mut engine = Engine::new();
    engine.camera_distance = 0.0;
    engine.target_camera_distance = 0.0;
    engine.view_pitch = -0.5;
    engine.reset_view();
    assert_eq!(engine.camera_distance, DEFAULT_ORBIT_DISTANCE);
    assert!((engine.target_camera_distance - DEFAULT_ORBIT_DISTANCE).abs() < 0.0001);
    assert_eq!(engine.view_pitch, super::super::DEFAULT_ORBIT_PITCH);
}

#[cfg(feature = "studio-ui")]
#[test]
fn studio_selection_moves_preview_player_near_the_target() {
    let mut engine = Engine::new();
    engine.player.position = [-24.6, 2.2, 24.0];
    engine.write_snapshot();

    engine.studio_move_player_near([-78.13532, 4.1533, -19.07567], 2.1);

    let position = engine.player.position;
    let distance = (position[0] + 78.13532).hypot(position[2] + 19.07567);
    assert!((distance - 4.0).abs() < 0.001);
    assert!((position[1] - 2.2).abs() < 0.001);
    assert_eq!(&engine.snapshot[..3], &position);
    assert!(!engine.player.moving);
    assert_eq!(engine.player.velocity, [0.0; 3]);
}

#[cfg(feature = "studio-ui")]
#[test]
fn studio_selection_chooses_a_clear_side_when_the_nearest_side_is_blocked() {
    let mut engine = Engine::new();
    engine.player.position = [4.0, 0.0, 4.0];
    engine.obstacles = vec![block_bounds([2.8, 1.5, 2.8], [2.0, 3.0, 2.0])];

    engine.studio_move_player_near([0.0, 0.0, 0.0], 0.0);

    let position = engine.player.position;
    let direct = [4.0_f32 / 2.0_f32.sqrt(), 0.0, 4.0 / 2.0_f32.sqrt()];
    assert!(!engine.player_can_occupy(direct));
    assert!(engine.player_can_occupy(position));
    assert!((position[0].hypot(position[2]) - 4.0).abs() < 0.001);
}

#[test]
fn camera_occlusion_clamps_effective_distance_without_overwriting_requested_zoom() {
    let mut engine = Engine::new();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.obstacles = vec![block_bounds([0.0, 2.0, 4.0], [4.0, 4.0, 1.0])];
    engine.base_obstacles = engine.obstacles.clone();

    engine.step(1.0 / 60.0);

    assert!(engine.camera_distance > 2.5 && engine.camera_distance < 3.5);
    assert!((engine.target_camera_distance - DEFAULT_ORBIT_DISTANCE).abs() < 0.0001);
}

#[test]
fn camera_occlusion_uses_the_runtime_terrain_field() {
    let definition: crate::terrain::TerrainDefinition = serde_json::from_str(
        r#"{
            "cellSize":0.5,
            "operations":[{
                "shape":"block",
                "operation":"fill",
                "position":[0,2,4],
                "size":[4,4,1],
                "material":"builtin:grass"
            }]
        }"#,
    )
    .unwrap();
    let mut engine = Engine::new();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.obstacles.clear();
    engine.base_obstacles.clear();
    engine.terrain = crate::terrain::TerrainGrid::build(&definition).unwrap();

    engine.step(1.0 / 60.0);

    assert!(engine.camera_distance > 2.5 && engine.camera_distance < 3.5);
    assert_eq!(engine.target_camera_distance, DEFAULT_ORBIT_DISTANCE);
}

#[test]
fn camera_sphere_catches_a_narrow_gap_that_a_center_ray_would_clear() {
    let mut engine = Engine::new();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.obstacles = vec![
        block_bounds([-1.1, 2.0, 4.0], [1.8, 4.0, 1.0]),
        block_bounds([1.1, 2.0, 4.0], [1.8, 4.0, 1.0]),
    ];
    engine.base_obstacles = engine.obstacles.clone();

    engine.step(1.0 / 60.0);

    assert!(engine.camera_distance < 3.5);
}

#[test]
fn camera_occlusion_handles_corners_and_ceilings() {
    let mut corner = Engine::new();
    corner.player.position = [0.0, 0.0, 0.0];
    corner.obstacles = vec![block_bounds([0.65, 2.0, 4.0], [0.6, 4.0, 1.0])];
    corner.base_obstacles = corner.obstacles.clone();
    corner.step(1.0 / 60.0);
    assert!(corner.camera_distance < 3.5);

    let mut ceiling = Engine::new();
    ceiling.player.position = [0.0, 0.0, 0.0];
    ceiling.view_pitch = 0.8;
    ceiling.target_pitch = 0.8;
    ceiling.obstacles = vec![block_bounds([0.0, 5.0, 3.0], [5.0, 1.0, 5.0])];
    ceiling.base_obstacles = ceiling.obstacles.clone();
    ceiling.step(1.0 / 60.0);
    assert!(ceiling.camera_distance < DEFAULT_ORBIT_DISTANCE);
}

#[test]
fn camera_snaps_inward_and_eases_back_after_an_obstruction_clears() {
    let mut engine = Engine::new();
    engine.player.position = [0.0, 0.0, 0.0];
    engine.obstacles = vec![block_bounds([0.0, 2.0, 1.5], [4.0, 4.0, 1.0])];
    engine.base_obstacles = engine.obstacles.clone();
    engine.step(1.0 / 60.0);
    let obstructed = engine.camera_distance;
    assert!(obstructed <= crate::camera::FIRST_PERSON_DISTANCE);

    engine.obstacles.clear();
    engine.base_obstacles.clear();
    engine.step(1.0 / 60.0);
    assert!(engine.camera_distance > obstructed);
    assert!(engine.camera_distance < DEFAULT_ORBIT_DISTANCE);
    for _ in 0..30 {
        engine.step(1.0 / 60.0);
    }
    assert!((engine.camera_distance - DEFAULT_ORBIT_DISTANCE).abs() < 0.02);
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
