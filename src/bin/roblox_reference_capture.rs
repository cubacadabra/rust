use cubacadabra_engine::dev_showcase::{RobloxReferenceCaptureConfig, capture_roblox_reference};
use std::path::PathBuf;

const USAGE: &str = "usage: roblox_reference_capture --scene <reference-scene.json> --output <capture.png> [--report <report.json>] [--path-prefix <scene path>] [--width <pixels>] [--height <pixels>] [--no-antialias]";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}\n{USAGE}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut scene_path = None;
    let mut output_path = None;
    let mut report_path = None;
    let mut path_prefix = None;
    let mut width = 1600;
    let mut height = 900;
    let mut antialias = true;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--scene" => scene_path = Some(PathBuf::from(value(&mut args, "--scene")?)),
            "--output" => output_path = Some(PathBuf::from(value(&mut args, "--output")?)),
            "--report" => report_path = Some(PathBuf::from(value(&mut args, "--report")?)),
            "--path-prefix" => path_prefix = Some(value(&mut args, "--path-prefix")?),
            "--width" => width = dimension(value(&mut args, "--width")?, "--width")?,
            "--height" => height = dimension(value(&mut args, "--height")?, "--height")?,
            "--no-antialias" => antialias = false,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {argument:?}")),
        }
    }
    let scene_path = scene_path.ok_or_else(|| "--scene is required".to_owned())?;
    let output_path = output_path.ok_or_else(|| "--output is required".to_owned())?;
    let report_path = report_path.unwrap_or_else(|| output_path.with_extension("report.json"));
    let config = RobloxReferenceCaptureConfig {
        scene_path,
        output_path,
        report_path: report_path.clone(),
        path_prefix,
        width,
        height,
        antialias,
    };
    let report = capture_roblox_reference(&config)?;
    println!(
        "wrote {} geometry ({} triangles) to {}",
        report.rendered_counts["geometry"],
        report.rendered_counts["triangles"],
        report.output_image
    );
    println!("report: {}", report_path.display());
    Ok(())
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn dimension(value: String, flag: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}
