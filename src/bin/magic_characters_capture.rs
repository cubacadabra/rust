use cubacadabra_engine::dev_showcase::{
    CaptureConfig, CapturePalette, CaptureQuality, capture_phase0_baseline,
};
use std::env;
use std::path::PathBuf;

fn main() {
    let mut config = CaptureConfig::default();
    let mut output = PathBuf::from("docs/baselines/magic-characters/phase0");
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => output = PathBuf::from(next_value(&mut arguments, "--output")),
            "--seed" => config.seed = parse_value(&mut arguments, "--seed"),
            "--pose-time" => config.pose_time = parse_value(&mut arguments, "--pose-time"),
            "--width" => config.width = parse_value(&mut arguments, "--width"),
            "--height" => config.height = parse_value(&mut arguments, "--height"),
            "--portrait-width" => {
                config.portrait_width = parse_value(&mut arguments, "--portrait-width")
            }
            "--portrait-height" => {
                config.portrait_height = parse_value(&mut arguments, "--portrait-height")
            }
            "--quality" => {
                config.quality = match next_value(&mut arguments, "--quality").as_str() {
                    "full" => CaptureQuality::Full,
                    "half" => CaptureQuality::Half,
                    value => usage(&format!("unknown quality {value:?}")),
                };
            }
            "--palette" => {
                config.palette = match next_value(&mut arguments, "--palette").as_str() {
                    "current" => CapturePalette::Current,
                    "high-contrast" => CapturePalette::HighContrast,
                    value => usage(&format!("unknown palette {value:?}")),
                };
            }
            "--help" | "-h" => usage(""),
            value => usage(&format!("unknown argument {value:?}")),
        }
    }

    match capture_phase0_baseline(&output, config) {
        Ok(report) => {
            let total_vertices = report
                .captures
                .iter()
                .map(|capture| capture.vertex_count)
                .sum::<usize>();
            println!(
                "wrote {} captures to {}",
                report.captures.len(),
                output.display()
            );
            println!(
                "adapter={} backend={} 18-character scene + isolated 50-character render stress",
                report.adapter.name, report.adapter.backend
            );
            println!("total captured vertices={total_vertices}");
            println!("report={}", output.join("phase0_report.json").display());
        }
        Err(error) => {
            eprintln!("magic_characters_capture: {error}");
            std::process::exit(1);
        }
    }
}

fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> String {
    arguments
        .next()
        .unwrap_or_else(|| usage(&format!("missing value for {flag}")))
}

fn parse_value<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    flag: &str,
) -> T {
    let value = next_value(arguments, flag);
    value
        .parse()
        .unwrap_or_else(|_| usage(&format!("invalid value {value:?} for {flag}")))
}

fn usage(error: &str) -> ! {
    if !error.is_empty() {
        eprintln!("error: {error}");
    }
    eprintln!(
        "usage: magic_characters_capture [--output DIR] [--seed N] [--pose-time SECONDS] \
         [--width PX] [--height PX] [--portrait-width PX] [--portrait-height PX] \
         [--quality full|half] [--palette current|high-contrast]"
    );
    std::process::exit(if error.is_empty() { 0 } else { 2 });
}
