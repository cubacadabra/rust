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
fn a_decorated_tall_stack_follows_pushes_without_script_rebuilding() {
    let manifest = pushable_manifest(None, None);
    let mut single = Engine::new();
    assert!(single.load_package_source(&manifest));
    push_for(&mut single, 180);
    let single_travel = single.pushable_blocks[0].offset[1];

    let mut stacked = Engine::new();
    assert!(stacked.load_package_source(&manifest));
    assert!(stacked.load_script_source(r#"
        local game = {}
        local built = false
        function game.on_tick(api)
            if built then return end
            built = true
            local blocks = {
                { position = { 0, 1, 1.03 }, size = { 1, 0.08, 0.06 }, color = 0x0B102B, collidable = false, attachedTo = "cube" },
            }
            for row = 2, 12 do
                local y = 1 + 2 * (row - 1)
                blocks[#blocks + 1] = { position = { 0, y, 0 }, size = { 2, 2, 2 }, color = 0xFFFFFF, attachedTo = "cube" }
                blocks[#blocks + 1] = { position = { 0, y, 1.03 }, size = { 1, 0.08, 0.06 }, color = 0x0B102B, collidable = false, attachedTo = "cube" }
            end
            api.world:set_build_blocks(blocks)
        end
        return game
    "#));
    push_for(&mut stacked, 180);

    let stack_travel = stacked.pushable_blocks[0].offset[1];
    assert!(stack_travel < -0.1, "stack did not move: {stack_travel}");
    assert!((stack_travel - single_travel).abs() < 0.02,
        "stack moved {stack_travel}, single cube moved {single_travel}");
    assert!((stacked.build_blocks()[1].position[2] - stack_travel).abs() < 0.001);
    assert!((stacked.build_blocks()[2].position[2] - 1.03 - stack_travel).abs() < 0.001);
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
fn remote_push_moves_attached_runtime_blocks_and_snapshot_restores_them() {
    let manifest = pushable_manifest(None, None);
    let script = r#"
        local game = {}
        function game.on_tick(api)
            api.world:set_build_blocks({
                { position = { 0, 3, 0 }, size = { 2, 2, 2 }, color = 0xFFFFFF, attachedTo = "cube" },
                { position = { 0, 1, 1.03 }, size = { 1, 0.08, 0.06 }, color = 0x0B102B, collidable = false, attachedTo = "cube" },
            })
        end
        return game
    "#;
    let mut viewer = Engine::new();
    assert!(viewer.load_package_source(&manifest));
    assert!(viewer.load_script_source(script));
    viewer.step(1.0 / 60.0);
    assert_eq!(viewer.obstacles.len(), viewer.base_obstacles.len() + 1);
    let state = json!({
        "type": "world_block_state", "contentHash": viewer.pushable_content_hash,
        "blockIndex": 0, "x": 0.0, "z": -1.0, "sequence": 1,
    });
    assert!(viewer.receive_world_block_state_json(&state.to_string()));
    assert_eq!(viewer.build_blocks()[0].position[2], -1.0);
    assert!((viewer.obstacles[viewer.base_obstacles.len()].min_z + 2.0).abs() < 0.001);

    let saved = viewer.capture_snapshot_json().expect("snapshot");
    let mut restored = Engine::new();
    assert!(restored.load_package_source(&manifest));
    assert!(restored.load_script_source(script));
    restored.restore_snapshot_json(&saved).expect("restore");
    assert_eq!(restored.build_blocks()[0].position[2], -1.0);
    assert_eq!(restored.state_hash(), viewer.state_hash());
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
