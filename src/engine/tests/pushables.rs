use serde_json::{Value, json};

fn pushable_manifest(collision: Option<Value>, terrain: Option<Value>) -> String {
    let mut world = json!({
        "world": { "spawn": [0, 0, 1.55] },
        "blocks": [
            { "id": "cube", "position": [0, 1, 0], "size": [2, 2, 2], "pushable": true },
            { "id": "child", "position": [0, 1, 0], "size": [1, 1, 1], "attachedTo": "cube" }
        ]
    });
    if let Some(collision) = collision { world["collision"] = collision; }
    if let Some(terrain) = terrain { world["terrain"] = terrain; }
    json!({
        "sdkVersion": "0.5.0",
        "startWorld": "world",
        "worlds": { "world": world }
    }).to_string()
}

fn push_for(engine: &mut Engine, ticks: usize) {
    engine.set_input(Input { forward: 1.0, sprint: true, ..Input::default() });
    for _ in 0..ticks { engine.step(1.0 / 60.0); }
}

#[test]
fn attached_collidable_child_moves_with_cube_and_snapshot_restores_both() {
    let manifest = pushable_manifest(None, None);
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&manifest));
    let parent = engine.obstacles[0];
    let child = engine.obstacles[1];
    push_for(&mut engine, 180);
    let displacement = engine.pushable_blocks[0].offset[1];
    assert!(displacement < -0.1, "cube did not move: {displacement}");
    assert!((engine.obstacles[0].min_z - parent.min_z - displacement).abs() < 0.001);
    assert!((engine.obstacles[1].min_z - child.min_z - displacement).abs() < 0.001);

    let snapshot = engine.capture_snapshot_json().expect("snapshot");
    let mut restored = Engine::new();
    assert!(restored.load_package_source(&manifest));
    restored.restore_snapshot_json(&snapshot).expect("restore");
    assert_eq!(restored.pushable_blocks[0].offset, engine.pushable_blocks[0].offset);
    assert!((restored.obstacles[1].min_z - engine.obstacles[1].min_z).abs() < 0.001);
    assert_eq!(restored.state_hash(), engine.state_hash());
}

#[test]
fn backend_state_moves_the_other_clients_collision_boxes() {
    let manifest = pushable_manifest(None, None);
    let mut viewer = Engine::new();
    assert!(viewer.load_package_source(&manifest));
    let state = json!({
        "type": "world_block_state", "contentHash": viewer.pushable_content_hash,
        "blockIndex": 0, "x": 0.0, "z": -1.0, "sequence": 1,
        "senderId": "web-player", "requestId": 1
    });
    assert!(viewer.receive_world_block_state_json(&state.to_string()));
    assert_eq!(viewer.pushable_blocks[0].offset, [0.0, -1.0]);
    assert!((viewer.obstacles[0].min_z + 2.0).abs() < 0.001);
    assert!((viewer.obstacles[1].min_z + 1.5).abs() < 0.001);
    assert!(viewer.receive_world_block_state_json(&state.to_string()));
    assert_eq!(viewer.pushable_blocks[0].offset, [0.0, -1.0]);
}

#[test]
fn rejected_push_restores_local_collision_and_keeps_newer_pending_motion() {
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&pushable_manifest(None, None)));
    engine.reset_pushable_network(true);
    let original = engine.obstacles[0];
    engine.set_pushable_offset(0, [0.0, -0.1]);
    engine.record_pushable_motion(0, 2, -0.1);
    engine.set_pushable_offset(0, [0.0, -0.2]);
    engine.record_pushable_motion(0, 2, -0.1);
    assert!(engine.receive_world_block_rejection_json(&json!({
        "type": "world_block_rejected", "contentHash": engine.pushable_content_hash,
        "blockIndex": 0, "requestId": 1, "x": 0, "z": 0, "sequence": 0
    }).to_string()));
    assert_eq!(engine.pushable_blocks[0].offset, [0.0, -0.1]);
    assert!((engine.obstacles[0].min_z - original.min_z + 0.1).abs() < 0.001);
    assert!(engine.receive_world_block_rejection_json(&json!({
        "type": "world_block_rejected", "contentHash": engine.pushable_content_hash,
        "blockIndex": 0, "requestId": 2, "x": 0, "z": 0, "sequence": 0
    }).to_string()));
    assert_eq!(engine.pushable_blocks[0].offset, [0.0, 0.0]);
    assert!((engine.obstacles[0].min_z - original.min_z).abs() < 0.001);
}

#[test]
fn cubes_stop_at_triangle_and_terrain_walls() {
    let triangle_wall = json!({
        "formatVersion": 1,
        "triangles": [
            [[-4, 0, -3], [4, 0, -3], [4, 4, -3]],
            [[-4, 0, -3], [4, 4, -3], [-4, 4, -3]]
        ]
    });
    let terrain_wall = json!({
        "cellSize": 0.5,
        "operations": [{ "operation": "fill", "shape": "block", "position": [0, 2, -3.5], "size": [8, 4, 1], "material": "ground" }]
    });
    for (collision, terrain) in [(Some(triangle_wall), None), (None, Some(terrain_wall))] {
        let mut engine = Engine::new();
        assert!(engine.load_package_source(&pushable_manifest(collision, terrain)));
        push_for(&mut engine, 360);
        assert!(engine.pushable_blocks[0].offset[1] < -0.1);
        assert!(engine.obstacles[0].min_z > -3.05,
            "cube crossed wall: {}", engine.obstacles[0].min_z);
    }
}

#[test]
fn watched_positions_follow_local_and_received_pushes_without_mutating_physics() {
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&pushable_manifest(None, None)));
    assert!(engine.load_script_source(r#"
        local game = {}
        function game.on_start(api)
            assert(api.world:get_positions() == nil)
            api.world:watch_positions({"cube", "missing"})
            assert(api.world:get_positions() == nil)
        end
        function game.on_tick(api)
            local positions = api.world:get_positions()
            assert(positions.worldId == "world")
            assert(positions.blocks.missing == nil)
            assert(positions.blocks.cube[2] == 1)
            positions.blocks.cube[1] = 999
            assert(api.world:get_positions().blocks.cube[1] ~= 999)
        end
        function game.on_save(api) return api.world:get_positions() end
        function game.on_restore(api) end
        return game
    "#));
    push_for(&mut engine, 120);
    assert!(engine.last_script_error().is_none(), "{:?}", engine.last_script_error());
    let saved = engine.capture_snapshot().unwrap();
    let expected = engine.pushable_blocks[0].offset[1];
    assert!(expected < -0.1);
    assert!((saved.game_state["blocks"]["cube"][2].as_f64().unwrap() - f64::from(expected)).abs() < 0.001);
    engine.set_input(Input::default());
    assert!(engine.receive_world_block_state_json(&json!({
        "type": "world_block_state", "contentHash": engine.pushable_content_hash,
        "blockIndex": 0, "x": 2.0, "z": -3.0, "sequence": 1
    }).to_string()));
    engine.step(1.0 / 60.0);
    let positions = engine.capture_snapshot().unwrap().game_state;
    assert_eq!(positions["blocks"]["cube"], json!([2.0, 1.0, -3.0]));
    assert_eq!(positions["player"], json!(engine.player.position));
}

#[test]
fn watched_positions_are_opt_in_bounded_and_clear_on_world_entry() {
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&pushable_manifest(None, None)));
    assert!(engine.load_script_source(r#"
        local game = {}
        function game.on_start(api)
            assert(not pcall(function() api.world:watch_positions(table.create(65, "cube")) end))
            assert(not pcall(function() api.world:watch_positions({""}) end))
            assert(not pcall(function() api.world:watch_positions({string.rep("a", 129)}) end))
        end
        return game
    "#));
    engine.step(1.0 / 60.0);
    assert!(engine.script.as_ref().unwrap().state().borrow().watched_player.is_none());
    assert!(engine.load_script_source(r#"
        return { on_start = function(api) api.world:watch_positions({"cube"}) end }
    "#));
    engine.step(1.0 / 60.0);
    let state = engine.script.as_ref().unwrap().state();
    assert!(state.borrow().watched_player.is_some());
    engine.start_world_by_id("world");
    assert!(state.borrow().watched_player.is_none());
    assert!(state.borrow().watched_blocks["cube"].is_none());
    engine.step(1.0 / 60.0);
    assert_eq!(state.borrow().watched_blocks["cube"], Some([0.0, 1.0, 0.0]));
}
