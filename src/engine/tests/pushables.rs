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
