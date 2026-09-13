use std::{env, fs, path::Path};

use cubacadabra_client::ClientSession;

fn main() {
    let packages: Vec<_> = env::args_os().skip(1).collect();
    if packages.is_empty() {
        eprintln!("usage: validate_game_packages package...");
        std::process::exit(2);
    }

    for package in packages {
        let path = Path::new(&package);
        let manifest = fs::read_to_string(path.join("manifest.json"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let script = fs::read_to_string(path.join("game.luau"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        ClientSession::load(&manifest, &script)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        println!("native loaded {}", path.display());
    }
}
