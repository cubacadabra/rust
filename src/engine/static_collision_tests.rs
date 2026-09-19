use super::{DEFAULT_ORBIT_DISTANCE, Engine};
use crate::types::Input;
use serde_json::{Value, json};

fn quad(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
    vec![[a, b, c], [a, c, d]]
}

fn collision_engine(spawn: [f32; 3], physics: Value, triangles: Vec<[[f32; 3]; 3]>) -> Engine {
    let manifest = json!({
        "sdkVersion": "0.5.0",
        "lobby": false,
        "startWorld": "triangle-world",
        "worlds": {
            "triangle-world": {
                "world": {"spawn": spawn, "physics": physics},
                "collision": {"formatVersion": 1, "triangles": triangles}
            }
        }
    });
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&manifest.to_string()));
    engine
}

#[test]
fn triangle_floor_lands_without_filling_the_surrounding_void() {
    let floor = quad(
        [-3.0, 0.0, -3.0],
        [3.0, 0.0, -3.0],
        [3.0, 0.0, 3.0],
        [-3.0, 0.0, 3.0],
    );
    let mut engine = collision_engine(
        [0.0, 2.0, 0.0],
        json!({"groundCollision": false, "deathY": -50}),
        floor,
    );
    engine.player.grounded = false;
    for _ in 0..90 {
        engine.step(1.0 / 60.0);
    }
    assert!(engine.player.grounded);
    assert!(engine.player.position[1].abs() < 0.001);

    engine.player.position = [6.0, 0.0, 0.0];
    engine.player.velocity = [0.0; 3];
    engine.player.grounded = false;
    for _ in 0..30 {
        engine.step(1.0 / 60.0);
    }
    assert!(engine.player.position[1] < -0.5);
    assert!(!engine.player.grounded);
}

#[test]
fn player_walks_up_a_triangle_ramp_but_stops_at_a_triangle_wall() {
    let ramp = quad(
        [-3.0, 0.0, -3.0],
        [3.0, 1.2, -3.0],
        [3.0, 1.2, 3.0],
        [-3.0, 0.0, 3.0],
    );
    let mut ramp_engine = collision_engine(
        [-2.5, 0.1, 0.0],
        json!({"groundCollision": false, "deathY": -50}),
        ramp,
    );
    for _ in 0..45 {
        ramp_engine.set_input(Input {
            strafe: 1.0,
            ..Input::default()
        });
        ramp_engine.step(1.0 / 60.0);
        if ramp_engine.player.position[0] > 1.0 {
            break;
        }
    }
    assert!(ramp_engine.player.position[0] > 0.5);
    assert!(ramp_engine.player.position[1] > 0.65);
    assert!(ramp_engine.player.grounded);

    let mut floor_and_wall = quad(
        [-4.0, 0.0, -4.0],
        [4.0, 0.0, -4.0],
        [4.0, 0.0, 4.0],
        [-4.0, 0.0, 4.0],
    );
    floor_and_wall.extend(quad(
        [1.0, 0.0, -3.0],
        [1.0, 5.0, -3.0],
        [1.0, 5.0, 3.0],
        [1.0, 0.0, 3.0],
    ));
    let mut wall_engine = collision_engine(
        [0.0, 0.0, 0.0],
        json!({"groundCollision": false}),
        floor_and_wall,
    );
    for _ in 0..90 {
        wall_engine.set_input(Input {
            strafe: 1.0,
            ..Input::default()
        });
        wall_engine.step(1.0 / 60.0);
    }
    assert!(wall_engine.player.position[0] < 0.55);
    assert!(wall_engine.player.position[1].abs() < 0.001);
}

#[test]
fn triangle_ceiling_stops_a_jump_and_floor_reports_landing() {
    let mut triangles = quad(
        [-4.0, 0.0, -4.0],
        [4.0, 0.0, -4.0],
        [4.0, 0.0, 4.0],
        [-4.0, 0.0, 4.0],
    );
    triangles.extend(quad(
        [-4.0, 4.2, -4.0],
        [4.0, 4.2, -4.0],
        [4.0, 4.2, 4.0],
        [-4.0, 4.2, 4.0],
    ));
    let mut engine = collision_engine(
        [0.0, 0.0, 0.0],
        json!({"groundCollision": false}),
        triangles,
    );
    engine.set_input(Input {
        jump: true,
        ..Input::default()
    });
    let mut maximum_feet = 0.0_f32;
    for _ in 0..150 {
        engine.step(1.0 / 60.0);
        maximum_feet = maximum_feet.max(engine.player.position[1]);
    }
    assert!(maximum_feet <= 4.2 - super::BODY_HEIGHT + 0.01);
    assert!(engine.player.grounded);
    assert!(engine.player.position[1].abs() < 0.001);
}

#[test]
fn triangle_wall_occludes_the_camera_sphere() {
    let wall = quad(
        [-3.0, 0.0, 4.0],
        [3.0, 0.0, 4.0],
        [3.0, 6.0, 4.0],
        [-3.0, 6.0, 4.0],
    );
    let mut engine = collision_engine([0.0, 0.0, 0.0], json!({}), wall);
    engine.step(1.0 / 60.0);
    assert!(engine.camera_distance > 2.5 && engine.camera_distance < 3.8);
    assert_eq!(engine.target_camera_distance, DEFAULT_ORBIT_DISTANCE);
}

#[test]
fn authored_horizontal_bounds_override_but_absence_retains_legacy_limits() {
    let manifest = json!({
        "sdkVersion": "0.5.0",
        "lobby": false,
        "startWorld": "bounded",
        "worlds": {
            "bounded": {
                "world": {
                    "spawn": [0, 0, 0],
                    "physics": {
                        "horizontalBounds": {"minimum": [-2, -3], "maximum": [2, 3]}
                    }
                }
            }
        }
    });
    let mut bounded = Engine::new();
    assert!(bounded.load_package_source(&manifest.to_string()));
    for _ in 0..120 {
        bounded.set_input(Input {
            strafe: 1.0,
            ..Input::default()
        });
        bounded.step(1.0 / 60.0);
    }
    assert!((bounded.player.position[0] - (2.0 - super::PLAYER_RADIUS)).abs() < 0.001);

    let mut legacy = Engine::new();
    legacy.obstacles.clear();
    legacy.base_obstacles.clear();
    legacy.player.position = [56.0, 0.0, 0.0];
    for _ in 0..120 {
        legacy.set_input(Input {
            strafe: 1.0,
            ..Input::default()
        });
        legacy.step(1.0 / 60.0);
    }
    assert!(
        (legacy.player.position[0] - (super::WORLD_LIMIT - super::PLAYER_RADIUS)).abs() < 0.001
    );
}

#[test]
fn package_rejects_unsupported_collision_version_and_invalid_bounds() {
    let unsupported = json!({
        "sdkVersion": "0.5.0",
        "collision": {"formatVersion": 2, "triangles": []}
    });
    let mut engine = Engine::new();
    assert!(!engine.load_package_source(&unsupported.to_string()));

    let invalid_bounds = json!({
        "sdkVersion": "0.5.0",
        "world": {
            "physics": {"horizontalBounds": {"minimum": [2, -1], "maximum": [1, 1]}}
        }
    });
    assert!(!engine.load_package_source(&invalid_bounds.to_string()));
}

#[test]
#[ignore = "requires the maze-101 source-derived hub collision fixture"]
fn source_hub_collision_walks_from_main_island_across_very_easy_bridge() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/maze-101/reference/hub-collision.json");
    let collision: Value = serde_json::from_str(
        &std::fs::read_to_string(&fixture)
            .unwrap_or_else(|error| panic!("read {}: {error}", fixture.display())),
    )
    .expect("hub collision fixture should be valid JSON");
    let manifest = json!({
        "sdkVersion": "0.5.0",
        "lobby": false,
        "startWorld": "hub",
        "worlds": {
            "hub": {
                "world": {
                    "spawn": [13.2, 4.1, 10.8],
                    "physics": {
                        "groundCollision": false,
                        "deathY": -70,
                        "horizontalBounds": {
                            "minimum": [-620, -180],
                            "maximum": [330, 410]
                        }
                    }
                },
                "collision": collision
            }
        }
    });
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&manifest.to_string()));
    engine.player.grounded = false;
    for _ in 0..180 {
        engine.step(1.0 / 60.0);
        assert!(!engine.player_dead, "spawn fell through hub collision");
        if engine.player.grounded {
            break;
        }
    }
    assert!(
        engine.player.grounded,
        "spawn did not settle on the main island"
    );

    let waypoints = [
        [-20.9, -4.8, -5.45],
        [-25.062, -4.2, -8.25],
        [-84.0, -7.8, -48.0],
        [-102.732, -2.784, -52.032],
    ];
    for (waypoint_index, waypoint) in waypoints.into_iter().enumerate() {
        let mut reached = false;
        for tick in 0..1_800 {
            let dx = waypoint[0] - engine.player.position[0];
            let dz = waypoint[2] - engine.player.position[2];
            let tolerance = if waypoint_index == 1 { 0.5 } else { 1.75 };
            if dx.hypot(dz) < tolerance {
                reached = true;
                break;
            }
            let yaw = (-dx).atan2(-dz);
            engine.view_yaw = yaw;
            engine.target_yaw = yaw;
            engine.set_input(Input {
                forward: 1.0,
                sprint: true,
                jump: waypoint_index == 1 && tick % 90 == 0,
                ..Input::default()
            });
            engine.step(1.0 / 60.0);
            assert!(
                !engine.player_dead,
                "fell before waypoint {waypoint:?} from {:?}",
                engine.player.position
            );
        }
        assert!(
            reached,
            "blocked before waypoint {waypoint:?} at {:?}",
            engine.player.position
        );
    }
}
