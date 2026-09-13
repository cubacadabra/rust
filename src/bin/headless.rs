use std::{env, fs, process};

use cubacadabra_engine::Engine;

const DEFAULT_TICKS: usize = 1_000;
const DEFAULT_DELTA: f32 = 1.0 / 60.0;

#[derive(Clone, Copy)]
struct Config {
    ticks: usize,
    delta: f32,
    forward: f32,
    strafe: f32,
    sprint: bool,
    jump_tick: Option<usize>,
}

struct RunResult {
    hash: u64,
    position: [f32; 3],
    world: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("headless: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let (manifest_path, script_path, config) = parse_args()?;
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read manifest {}: {error}", manifest_path.display()))?;
    let script = fs::read_to_string(&script_path)
        .map_err(|error| format!("read script {}: {error}", script_path.display()))?;

    let first = simulate(&manifest, &script, config)?;
    let second = simulate(&manifest, &script, config)?;
    if first.hash != second.hash {
        return Err(format!(
            "simulation is not deterministic: first hash {:016x}, second hash {:016x}",
            first.hash, second.hash
        ));
    }

    println!("headless: deterministic run passed");
    println!("  ticks: {}", config.ticks);
    println!("  delta: {:.9}", config.delta);
    println!("  world: {}", first.world);
    println!(
        "  player: [{:.4}, {:.4}, {:.4}]",
        first.position[0], first.position[1], first.position[2]
    );
    println!("  state_hash: {:016x}", first.hash);
    Ok(())
}

fn simulate(manifest: &str, script: &str, config: Config) -> Result<RunResult, String> {
    let mut engine = Engine::new();
    if !engine.load_package_source(manifest) {
        return Err("engine rejected manifest".to_owned());
    }
    if !engine.load_script_source(script) {
        return Err(format!(
            "engine could not load script: {}",
            engine
                .last_script_error()
                .unwrap_or_else(|| "unknown script error".to_owned())
        ));
    }

    for tick in 0..config.ticks {
        engine.set_input_values(
            config.forward,
            config.strafe,
            config.sprint,
            config.jump_tick == Some(tick),
            false,
            0.0,
            0.0,
            0.0,
        );
        engine.step(config.delta);
    }
    if let Some(error) = engine.last_script_error() {
        return Err(format!("script failed during simulation: {error}"));
    }

    let snapshot = engine.snapshot();
    Ok(RunResult {
        hash: engine.state_hash(),
        position: [snapshot[0], snapshot[1], snapshot[2]],
        world: engine.active_world_id().unwrap_or("<none>").to_owned(),
    })
}

fn parse_args() -> Result<(std::path::PathBuf, std::path::PathBuf, Config), String> {
    let mut args = env::args().skip(1);
    let Some(manifest) = args.next() else {
        return Err(usage());
    };
    if manifest == "--help" || manifest == "-h" {
        println!("{}", usage());
        process::exit(0);
    }
    let Some(script) = args.next() else {
        return Err(usage());
    };

    let mut config = Config {
        ticks: DEFAULT_TICKS,
        delta: DEFAULT_DELTA,
        forward: 1.0,
        strafe: 0.0,
        sprint: false,
        jump_tick: Some(30),
    };
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--ticks" => config.ticks = parse_next(&mut args, "--ticks")?,
            "--delta" => config.delta = parse_next(&mut args, "--delta")?,
            "--forward" => config.forward = parse_next(&mut args, "--forward")?,
            "--strafe" => config.strafe = parse_next(&mut args, "--strafe")?,
            "--jump-tick" => config.jump_tick = Some(parse_next(&mut args, "--jump-tick")?),
            "--no-jump" => config.jump_tick = None,
            "--sprint" => config.sprint = true,
            "--help" | "-h" => {
                println!("{}", usage());
                process::exit(0);
            }
            _ => return Err(format!("unknown argument {argument}\n\n{}", usage())),
        }
    }
    if config.delta <= 0.0 || config.delta > 0.05 || !config.delta.is_finite() {
        return Err("--delta must be finite and in the range (0, 0.05]".to_owned());
    }
    if !config.forward.is_finite() || !config.strafe.is_finite() {
        return Err("--forward and --strafe must be finite".to_owned());
    }
    Ok((manifest.into(), script.into(), config))
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))?
        .parse()
        .map_err(|error| format!("invalid value for {option}: {error}"))
}

fn usage() -> String {
    "usage: headless <manifest.json> <game.luau> [--ticks N] [--delta SECONDS]\n\
     [--forward F] [--strafe F] [--sprint] [--jump-tick N|--no-jump]"
        .to_owned()
}
