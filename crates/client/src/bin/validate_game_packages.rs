use std::{env, fs, path::Path};

use cubacadabra_client::ClientSession;
use serde_json::{Value, json};

const TRACE_INPUTS: [[f32; 8]; 3] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.25, 1.0, 0.0, 0.0, 0.15, -0.1, 0.0],
    [-0.5, -1.0, 0.0, 1.0, 0.0, -0.2, 0.05, 0.1],
];

fn action_to_json(action: cubacadabra_client::ClientAction) -> Value {
    match action {
        cubacadabra_client::ClientAction::SetWorld(world_id) => {
            json!({ "type": "set_world", "worldId": world_id })
        }
        cubacadabra_client::ClientAction::SendText(source) => {
            json!({ "type": "send_text", "source": source })
        }
    }
}

fn sample(client: &ClientSession, actions: Vec<cubacadabra_client::ClientAction>) -> Value {
    let engine = client.engine();
    json!({
        "actions": actions.into_iter().map(action_to_json).collect::<Vec<_>>(),
        "snapshotWords": engine.snapshot().iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
    })
}

fn trace(package: &Path) {
    let manifest = fs::read_to_string(package.join("manifest.json"))
        .unwrap_or_else(|error| panic!("{}: {error}", package.display()));
    let script = fs::read_to_string(package.join("game.luau"))
        .unwrap_or_else(|error| panic!("{}: {error}", package.display()));
    let mut client = ClientSession::load(&manifest, &script)
        .unwrap_or_else(|error| panic!("{}: {error}", package.display()));
    let initial_actions = client.poll_actions();
    let mut samples = vec![sample(&client, initial_actions)];
    for input in TRACE_INPUTS {
        client.engine_mut().set_input_values(
            input[0],
            input[1],
            input[2] != 0.0,
            input[3] != 0.0,
            input[4] != 0.0,
            input[5],
            input[6],
            input[7],
        );
        client.engine_mut().step(0.016);
        let actions = client.poll_actions();
        samples.push(sample(&client, actions));
    }
    println!(
        "CONFORMANCE_TRACE {}",
        serde_json::to_string(&samples).unwrap()
    );
}

fn main() {
    let mut args: Vec<_> = env::args_os().skip(1).collect();
    let trace_mode = args.first().is_some_and(|arg| arg == "--trace");
    if trace_mode {
        args.remove(0);
    }
    let packages = args;
    if packages.is_empty() {
        eprintln!("usage: validate_game_packages [--trace] package...");
        std::process::exit(2);
    }

    for package in packages {
        let path = Path::new(&package);
        if trace_mode {
            trace(path);
            continue;
        }
        let manifest = fs::read_to_string(path.join("manifest.json"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let script = fs::read_to_string(path.join("game.luau"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        ClientSession::load(&manifest, &script)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        println!("native loaded {}", path.display());
    }
}
