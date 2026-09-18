//! Opt-in checks against the real CLI-built examples/maze-101 package.
//! Set CUBACADABRA_TEST_MAZE_PACKAGE_DIR and run `maze_tests -- --ignored`.
use super::{BODY_HEIGHT, Engine, PLAYER_RADIUS, RUN_SPEED};
use crate::types::Input;
use crate::ui::{UiInsets, UiViewport};
use std::collections::VecDeque;

const STEP: f32 = 1.0 / 60.0;
// The fixture's authored 8x8 maze. Connectivity is read from runtime collision,
// not copied from the builder's maze-generation algorithm.
const WIDTH: usize = 8;
const CELL_SIZE: f32 = 15.0;

fn load_maze() -> Engine {
    let directory = std::env::var("CUBACADABRA_TEST_MAZE_PACKAGE_DIR")
        .expect("set CUBACADABRA_TEST_MAZE_PACKAGE_DIR to a built maze-101 package");
    let directory = std::path::Path::new(&directory);
    let manifest = std::fs::read_to_string(directory.join("manifest.json")).unwrap();
    let script = std::fs::read_to_string(directory.join("game.luau")).unwrap();
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&manifest));
    assert!(engine.load_script_source(&script));
    engine.set_ui_viewport(UiViewport {
        width: 1280.0,
        height: 800.0,
        scale: 1.0,
        safe_area: UiInsets::default(),
    });
    for _ in 0..120 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("easy-room"));
    assert!(engine.player.grounded, "spawn must settle on the floor");
    engine
}

fn cell_center(cell: usize) -> [f32; 3] {
    [
        (cell % WIDTH) as f32 * CELL_SIZE - 52.5,
        0.1,
        (cell / WIDTH) as f32 * CELL_SIZE - 52.5,
    ]
}

fn cell_at(position: [f32; 3]) -> usize {
    let x = ((position[0] + 52.5) / CELL_SIZE).round() as usize;
    let z = ((position[2] + 52.5) / CELL_SIZE).round() as usize;
    assert!(x < WIDTH && z < WIDTH);
    z * WIDTH + x
}

fn collision_graph(engine: &Engine) -> Vec<Vec<usize>> {
    let terrain = engine.terrain.as_ref().unwrap();
    (0..WIDTH * WIDTH)
        .map(|cell| {
            let from = cell_center(cell);
            (0..WIDTH * WIDTH)
                .filter(|next| {
                    (cell % WIDTH).abs_diff(next % WIDTH) + (cell / WIDTH).abs_diff(next / WIDTH)
                        == 1
                })
                .filter(|next| {
                    let to = cell_center(*next);
                    let midpoint = [(from[0] + to[0]) / 2.0, 0.1, (from[2] + to[2]) / 2.0];
                    terrain.capsule_clear(midpoint, PLAYER_RADIUS, BODY_HEIGHT)
                })
                .collect()
        })
        .collect()
}

fn route(graph: &[Vec<usize>], start: usize, goal: usize, finish: usize) -> Vec<usize> {
    let mut previous = vec![None; graph.len()];
    previous[start] = Some(start);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        if cell == goal {
            break;
        }
        for &next in &graph[cell] {
            // Do not complete the maze until all collectibles were visited.
            if previous[next].is_none() && (next != finish || goal == finish) {
                previous[next] = Some(cell);
                queue.push_back(next);
            }
        }
    }
    assert!(
        previous[goal].is_some(),
        "no walkable route from {start} to {goal}"
    );
    let mut path = vec![goal];
    while *path.last().unwrap() != start {
        path.push(previous[*path.last().unwrap()].unwrap());
    }
    path.reverse();
    path
}

fn walk_to(engine: &mut Engine, target: [f32; 3]) {
    for _ in 0..600 {
        let dx = target[0] - engine.player.position[0];
        let dz = target[2] - engine.player.position[2];
        if dx.hypot(dz) < 0.18 {
            return;
        }
        let yaw = engine.view_yaw;
        let scale = (3.0 / RUN_SPEED).min(1.0 / dx.hypot(dz));
        engine.set_input(Input {
            forward: (-dx * yaw.sin() - dz * yaw.cos()) * scale,
            strafe: (dx * yaw.cos() - dz * yaw.sin()) * scale,
            sprint: true,
            ..Input::default()
        });
        engine.step(STEP);
        assert_eq!(engine.player_deaths, 0, "fell through maze terrain");
    }
    panic!(
        "blocked walking to {target:?}, player at {:?}",
        engine.player.position
    );
}

fn hud(engine: &Engine, id: &str) -> String {
    engine
        .ui
        .borrow_mut()
        .frame()
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap()
        .text
        .clone()
}

#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_maze_collects_coins_and_reaches_exit_with_real_movement() {
    let mut engine = load_maze();
    let graph = collision_graph(&engine);
    eprintln!(
        "maze collision graph: {} passages",
        graph.iter().map(Vec::len).sum::<usize>() / 2
    );
    let finish = cell_at(
        engine
            .interactions
            .world
            .iter()
            .find(|zone| zone.id == "maze-finish")
            .unwrap()
            .position,
    );
    let mut goals: Vec<_> = engine
        .interactions
        .world
        .iter()
        .filter(|zone| zone.kind == "collectible")
        .map(|zone| cell_at(zone.position))
        .collect();
    assert_eq!(goals.len(), 8);
    while !goals.is_empty() {
        let start = cell_at(engine.player.position);
        let path = goals
            .iter()
            .map(|goal| route(&graph, start, *goal, finish))
            .min_by_key(Vec::len)
            .unwrap();
        for &cell in &path {
            walk_to(&mut engine, cell_center(cell));
        }
        goals.retain(|cell| !path.contains(cell));
    }
    assert!(hud(&engine, "maze-progress").contains("8/8 COINS"));
    for cell in route(&graph, cell_at(engine.player.position), finish, finish) {
        walk_to(&mut engine, cell_center(cell));
    }
    assert!(hud(&engine, "maze-status").contains("MAZE CLEARED"));
    assert_eq!(engine.active_world_id(), Some("easy-room"));
    eprintln!(
        "collected 8 coins and cleared the maze in {:.1}s using movement only",
        engine.elapsed
    );
}

#[cfg(debug_assertions)]
#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_maze_debug_skip_reaches_the_current_exit() {
    let mut engine = load_maze();
    let skip = engine
        .ui
        .borrow_mut()
        .frame()
        .nodes
        .iter()
        .find(|node| node.id == "maze-debug-end")
        .unwrap()
        .clone();
    let x = skip.rect.x + skip.rect.width / 2.0;
    let y = skip.rect.y + skip.rect.height / 2.0;
    assert!(engine.ui_pointer_event(1, 0, x, y));
    assert!(engine.ui_pointer_event(1, 2, x, y));
    for _ in 0..60 {
        engine.step(STEP);
    }
    assert!(hud(&engine, "maze-status").contains("MAZE CLEARED"));
    assert!(hud(&engine, "maze-progress").contains("8/8 COINS"));
    assert_eq!(engine.active_world_id(), Some("easy-room"));
}
