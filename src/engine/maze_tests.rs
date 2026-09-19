//! Opt-in checks against the real CLI-built examples/maze-101 package.
//! Set CUBACADABRA_TEST_MAZE_PACKAGE_DIR and run `maze_tests -- --ignored`.
use super::{BODY_HEIGHT, Engine, PLAYER_RADIUS, RUN_SPEED};
use crate::types::Input;
use crate::ui::{UiInsets, UiViewport};
use std::collections::VecDeque;

const STEP: f32 = 1.0 / 60.0;
// Connectivity is read from runtime collision, not copied from the builder.
const WIDTH: usize = 5;
const CELL_SIZE: f32 = 15.0;

fn load_maze() -> Engine {
    let directory = std::env::var("CUBACADABRA_TEST_MAZE_PACKAGE_DIR")
        .expect("set CUBACADABRA_TEST_MAZE_PACKAGE_DIR to a built maze-101 package");
    let directory = std::path::Path::new(&directory);
    let manifest = std::fs::read_to_string(directory.join("manifest.json")).unwrap();
    let script = std::fs::read_to_string(directory.join("game.luau")).unwrap();
    let mut engine = Engine::new();
    assert!(engine.load_package_source(&manifest));
    assert!(
        engine.load_script_source(&script),
        "{:?}",
        engine.last_script_error()
    );
    engine.set_ui_viewport(UiViewport {
        width: 1280.0,
        height: 800.0,
        scale: 1.0,
        safe_area: UiInsets::default(),
    });
    for _ in 0..120 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-world"));
    assert!(engine.player.grounded, "spawn must settle on the floor");
    engine
}

fn cell_center(cell: usize) -> [f32; 3] {
    cell_center_for(cell, WIDTH)
}

fn cell_center_for(cell: usize, width: usize) -> [f32; 3] {
    let first = -((width - 1) as f32 * CELL_SIZE) / 2.0;
    [
        (cell % width) as f32 * CELL_SIZE + first,
        0.1,
        (cell / width) as f32 * CELL_SIZE + first,
    ]
}

fn cell_at(position: [f32; 3]) -> usize {
    let x = ((position[0] + 30.0) / CELL_SIZE).round() as usize;
    let z = ((position[2] + 30.0) / CELL_SIZE).round() as usize;
    assert!(x < WIDTH && z < WIDTH);
    z * WIDTH + x
}

fn collision_graph(engine: &Engine) -> Vec<Vec<usize>> {
    collision_graph_for(engine, WIDTH)
}

fn collision_graph_for(engine: &Engine, width: usize) -> Vec<Vec<usize>> {
    let terrain = engine.terrain.as_ref().unwrap();
    (0..width * width)
        .map(|cell| {
            let from = cell_center_for(cell, width);
            (0..width * width)
                .filter(|next| {
                    (cell % width).abs_diff(next % width) + (cell / width).abs_diff(next / width)
                        == 1
                })
                .filter(|next| {
                    let to = cell_center_for(*next, width);
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
    let world = engine.active_world_id().unwrap().to_owned();
    for _ in 0..600 {
        let dx = target[0] - engine.player.position[0];
        let dz = target[2] - engine.player.position[2];
        if dx.hypot(dz) < 0.18 {
            return;
        }
        let yaw = engine.view_yaw;
        let scale = (1.0 / dx.hypot(dz)).min(1.0 / (RUN_SPEED * STEP));
        engine.set_input(Input {
            forward: (-dx * yaw.sin() - dz * yaw.cos()) * scale,
            strafe: (dx * yaw.cos() - dz * yaw.sin()) * scale,
            sprint: true,
            ..Input::default()
        });
        engine.step(STEP);
        assert_eq!(engine.player_deaths, 0, "fell through maze terrain");
        if engine.active_world_id() != Some(world.as_str()) {
            return;
        }
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
    assert!(engine.start_world_by_id("maze-very-easy-1"));
    engine.set_input(Input::default());
    for _ in 0..60 {
        engine.step(STEP);
    }
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
    let maze_world = engine.active_world;
    for index in 1..=8 {
        assert_eq!(
            engine
                .effects
                .states
                .get(&(maze_world, format!("maze-coin-{index:02}")))
                .map(String::as_str),
            Some("collected")
        );
    }
    for cell in route(&graph, cell_at(engine.player.position), finish, finish) {
        walk_to(&mut engine, cell_center(cell));
    }
    engine.set_input(Input::default());
    for _ in 0..2 {
        engine.step(STEP);
    }
    assert!(hud(&engine, "maze-status").contains("CLEARED"));
    assert!(
        hud(&engine, "maze-wallet").contains("133"),
        "eight coins plus 25 solo reward"
    );
    assert_eq!(engine.active_world_id(), Some("maze-world"));
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
    assert!(engine.start_world_by_id("maze-very-easy-1"));
    engine.step(STEP);
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
    assert!(hud(&engine, "maze-status").contains("CLEARED"));
    assert_eq!(engine.active_world_id(), Some("maze-world"));
}

#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_maze_timer_only_runs_inside_a_maze_and_timeout_returns_to_hub() {
    let mut engine = load_maze();
    for _ in 0..60 * 61 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-world"));
    assert_eq!(hud(&engine, "maze-wallet"), "COINS  100");
    assert!(engine.start_world_by_id("maze-very-easy-1"));
    for _ in 0..60 * 61 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-world"));
    assert!(hud(&engine, "maze-status").contains("TIMED OUT"));
    assert_eq!(hud(&engine, "maze-wallet"), "COINS  100");
}

fn walk_hub_to(engine: &mut Engine, target: [f32; 2], tolerance: f32, jump: bool) {
    for tick in 0..1800 {
        let dx = target[0] - engine.player.position[0];
        let dz = target[1] - engine.player.position[2];
        if dx.hypot(dz) < tolerance {
            engine.set_input(Input::default());
            return;
        }
        let yaw = engine.view_yaw;
        let scale = 1.0 / dx.hypot(dz);
        engine.set_input(Input {
            forward: (-dx * yaw.sin() - dz * yaw.cos()) * scale,
            strafe: (dx * yaw.cos() - dz * yaw.sin()) * scale,
            sprint: true,
            jump: jump && tick % 90 == 0,
            ..Input::default()
        });
        engine.step(STEP);
        assert_eq!(engine.player_deaths, 0, "fell on source bridge");
        assert_eq!(engine.active_world_id(), Some("maze-world"));
        assert_eq!(engine.last_script_error(), None);
    }
    panic!(
        "blocked to hub waypoint {target:?} at {:?}",
        engine.player.position
    );
}

#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_hub_walks_bridge_cancels_queue_and_enters_maze() {
    let mut engine = load_maze();
    walk_hub_to(&mut engine, [-20.9, -5.45], 1.75, false);
    walk_hub_to(&mut engine, [-25.062, -8.25], 0.5, true);
    walk_hub_to(&mut engine, [-84.0, -48.0], 1.75, false);
    // Enter just inside the waiting-zone edge, then leave before countdown.
    walk_hub_to(&mut engine, [-96.0, -51.0], 0.4, false);
    assert!(hud(&engine, "maze-queue").contains("QUEUE"));
    walk_hub_to(&mut engine, [-89.0, -49.0], 0.4, false);
    for _ in 0..60 * 5 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-world"));
    assert!(hud(&engine, "maze-status").contains("CANCELLED"));
    walk_hub_to(&mut engine, [-102.732, -52.032], 1.75, false);
    for _ in 0..60 * 22 {
        engine.step(STEP);
        if engine.active_world_id() != Some("maze-world") {
            break;
        }
    }
    assert_eq!(engine.active_world_id(), Some("maze-very-easy-1"));
    engine.step(STEP);
    assert!(hud(&engine, "maze-status").contains("FIND THE EXIT"));
    assert_eq!(hud(&engine, "maze-wallet"), "COINS  100");
    assert_eq!(engine.last_script_error(), None);

    // Finish that same queued run, then revisit the room to prove the next
    // baked variant is selected without restarting the script or package.
    for _ in 0..60 {
        engine.step(STEP);
    }
    let graph = collision_graph(&engine);
    let finish = cell_at(
        engine
            .interactions
            .world
            .iter()
            .find(|z| z.id == "maze-finish")
            .unwrap()
            .position,
    );
    for cell in route(&graph, 0, finish, finish) {
        walk_to(&mut engine, cell_center(cell));
    }
    engine.set_input(Input::default());
    for _ in 0..60 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-world"));
    assert!(hud(&engine, "maze-status").contains("CLEARED"));
    walk_hub_to(&mut engine, [-20.9, -5.45], 1.75, false);
    walk_hub_to(&mut engine, [-25.062, -8.25], 0.5, true);
    walk_hub_to(&mut engine, [-84.0, -48.0], 1.75, false);
    walk_hub_to(&mut engine, [-102.732, -52.032], 1.75, false);
    for _ in 0..60 * 22 {
        engine.step(STEP);
        if engine.active_world_id() != Some("maze-world") {
            break;
        }
    }
    assert_eq!(engine.active_world_id(), Some("maze-very-easy-2"));
}

#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_maze_variants_have_reachable_dead_end_exits_and_can_be_completed() {
    let mut engine = load_maze();
    for (world, width) in [
        ("maze-very-easy-1", 5),
        ("maze-very-easy-2", 5),
        ("easy-room", 10),
        ("maze-easy-2", 10),
        ("maze-normal-1", 15),
        ("maze-hard-1", 20),
    ] {
        assert!(engine.start_world_by_id(world));
        engine.set_input(Input::default());
        for _ in 0..60 {
            engine.step(STEP);
        }
        assert!(engine.player.grounded, "{world} spawn");
        let graph = collision_graph_for(&engine, width);
        let position = engine
            .interactions
            .world
            .iter()
            .find(|z| z.id == "maze-finish")
            .unwrap()
            .position;
        let finish = (0..width * width)
            .min_by(|a, b| {
                let distance = |cell| {
                    let p = cell_center_for(cell, width);
                    (p[0] - position[0]).hypot(p[2] - position[2])
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .unwrap();
        assert_eq!(
            graph[finish].len(),
            1,
            "{world}: finish must not cut off collectible branches"
        );
        for cell in route(&graph, 0, finish, finish) {
            walk_to(&mut engine, cell_center_for(cell, width));
        }
        engine.set_input(Input::default());
        for _ in 0..2 {
            engine.step(STEP);
        }
        assert_eq!(
            engine.active_world_id(),
            Some("maze-world"),
            "{world}: finish return"
        );
        assert!(
            hud(&engine, "maze-status").contains("CLEARED"),
            "{world}: {:?}",
            engine.last_script_error()
        );
        eprintln!("{world}: completed through actual terrain with movement inputs");
    }
}

#[test]
#[ignore = "requires CLI-built maze-101 package"]
fn configured_room_unlock_is_paid_once_and_requires_confirmation() {
    let mut engine = load_maze();
    let wallet = |engine: &Engine| {
        hud(engine, "maze-wallet")
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<u32>()
            .unwrap()
    };
    // Isolate the economy/UI contract with fixture placement at authored
    // finish/queue zones. Ordinary-input traversal is covered separately.
    for _ in 0..16 {
        assert!(engine.start_world_by_id("maze-very-easy-1"));
        engine.step(STEP);
        let finish = engine
            .interactions
            .world
            .iter()
            .find(|z| z.id == "maze-finish")
            .unwrap()
            .position;
        engine.player.position = [finish[0], 0.1, finish[2]];
        for _ in 0..3 {
            engine.step(STEP);
        }
        assert_eq!(engine.active_world_id(), Some("maze-world"));
    }
    assert_eq!(wallet(&engine), 500);
    for _ in 0..60 * 11 {
        engine.step(STEP);
    }
    engine.player.position = [23.3443, -3.0509, -116.3131];
    for _ in 0..60 * 22 {
        engine.step(STEP);
    }
    assert_eq!(
        engine.active_world_id(),
        Some("maze-world"),
        "locked room must not auto-unlock"
    );
    assert_eq!(wallet(&engine), 500);
    let button = engine
        .ui
        .borrow_mut()
        .frame()
        .nodes
        .iter()
        .find(|n| n.id == "maze-queue-action")
        .unwrap()
        .clone();
    let x = button.rect.x + button.rect.width / 2.0;
    let y = button.rect.y + button.rect.height / 2.0;
    assert!(engine.ui_pointer_event(1, 0, x, y));
    assert!(engine.ui_pointer_event(1, 2, x, y));
    engine.step(STEP);
    assert_eq!(wallet(&engine), 0);
    for _ in 0..60 * 22 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("easy-room"));
    let finish = engine
        .interactions
        .world
        .iter()
        .find(|z| z.id == "maze-finish")
        .unwrap()
        .position;
    engine.player.position = [finish[0], 0.1, finish[2]];
    for _ in 0..3 {
        engine.step(STEP);
    }
    assert_eq!(wallet(&engine), 125);
    for _ in 0..60 * 11 {
        engine.step(STEP);
    }
    engine.player.position = [23.3443, -3.0509, -116.3131];
    for _ in 0..60 * 22 {
        engine.step(STEP);
    }
    assert_eq!(engine.active_world_id(), Some("maze-easy-2"));
    assert_eq!(
        wallet(&engine),
        125,
        "unlocked tier must not charge another entry fee"
    );
    assert_eq!(engine.last_script_error(), None);
}
