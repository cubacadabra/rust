#[cfg(not(target_arch = "wasm32"))]
use cubacadabra_engine::dev_showcase::{
    CaptureAvatar, CaptureConfig, CapturePalette, CaptureQuality, HeroCaptureSet,
    MotionCaptureMode, capture_phase0_baseline, capture_phase2_shape_proof, capture_phase3,
    capture_phase4_motion_with_mode, capture_phase5_outfits, capture_phase6_report,
    capture_phase8_rollout, capture_phase9_hero_with_set,
};
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let mut config = CaptureConfig::default();
    let mut output = None;
    let mut phase = 0_u8;
    let mut capture_set = HeroCaptureSet::Full;
    let mut motion_mode = MotionCaptureMode::Staged;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--phase" => phase = parse_value(&mut arguments, "--phase"),
            "--output" => output = Some(PathBuf::from(next_value(&mut arguments, "--output"))),
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
            "--capture-set" => {
                capture_set = match next_value(&mut arguments, "--capture-set").as_str() {
                    "full" => HeroCaptureSet::Full,
                    "stills" => HeroCaptureSet::Stills,
                    "motion" => HeroCaptureSet::Motion,
                    "review" => HeroCaptureSet::Review,
                    "hair" => HeroCaptureSet::Hair,
                    value => usage(&format!("unknown capture set {value:?}")),
                };
            }
            "--motion-mode" => {
                motion_mode = match next_value(&mut arguments, "--motion-mode").as_str() {
                    "staged" => MotionCaptureMode::Staged,
                    "moving" => MotionCaptureMode::Moving,
                    "moving-raised" => MotionCaptureMode::MovingRaised,
                    value => usage(&format!("unknown motion mode {value:?}")),
                };
            }
            "--palette" => {
                config.palette = match next_value(&mut arguments, "--palette").as_str() {
                    "current" => CapturePalette::Current,
                    "high-contrast" => CapturePalette::HighContrast,
                    value => usage(&format!("unknown palette {value:?}")),
                };
            }
            "--avatar" => {
                config.avatar = match next_value(&mut arguments, "--avatar").as_str() {
                    "legacy" => CaptureAvatar::Legacy,
                    "rounded" => CaptureAvatar::Rounded,
                    "shape-proof" => CaptureAvatar::ShapeProof,
                    "magic" => CaptureAvatar::Magic,
                    value => usage(&format!("unknown avatar path {value:?}")),
                };
            }
            "--help" | "-h" => usage(""),
            value => usage(&format!("unknown argument {value:?}")),
        }
    }

    let output =
        output.unwrap_or_else(|| PathBuf::from(format!("target/character-review/phase{phase}")));
    if phase != 9 && capture_set != HeroCaptureSet::Full {
        usage("--capture-set is only supported for Phase 9");
    }
    if phase != 4 && motion_mode != MotionCaptureMode::Staged {
        usage("--motion-mode is only supported for Phase 4");
    }
    if phase == 3 {
        match capture_phase3(&output) {
            Ok(report) => println!(
                "Phase 3 verified on {} ({}) with {} measurements; report={}",
                report.adapter,
                report.backend,
                report.measurements.len(),
                output.join("phase3_report.json").display()
            ),
            Err(error) => {
                eprintln!("Phase 3: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if phase == 4 || phase == 5 || phase == 9 {
        let result = if phase == 4 {
            capture_phase4_motion_with_mode(&output, config, motion_mode)
        } else if phase == 9 {
            capture_phase9_hero_with_set(&output, config, capture_set)
        } else {
            capture_phase5_outfits(&output, config)
        };
        match result {
            Ok(report) => println!(
                "wrote {} capture(s) to {} (adapter={} backend={})",
                report.captures.len(),
                output.display(),
                report.adapter.name,
                report.adapter.backend
            ),
            Err(error) => {
                eprintln!("Phase {phase}: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if phase == 6 {
        match capture_phase6_report(&output) {
            Ok(report) => println!(
                "wrote Phase 6 quality policy ({} outfits, {} materials) to {}",
                report.catalog.outfits,
                report.catalog.materials,
                output.join("phase6_report.json").display()
            ),
            Err(error) => {
                eprintln!("Phase 6: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if phase == 8 {
        match capture_phase8_rollout(&output, config) {
            Ok(report) => println!(
                "wrote Phase 8 legacy/magic rollout captures ({} + {}) to {}",
                report.legacy_capture.captures.len(),
                report.magic_capture.captures.len(),
                output.display()
            ),
            Err(error) => {
                eprintln!("Phase 8: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if phase != 0 && phase != 2 {
        usage("supported phases are 0, 2, 3, 4, 5, 6, 8 and 9");
    }
    let result = if phase == 2 {
        capture_phase2_shape_proof(&output, config)
    } else {
        capture_phase0_baseline(&output, config)
    };
    match result {
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
            let report_name = if phase == 2 {
                "phase2_report.json"
            } else {
                "phase0_report.json"
            };
            println!("report={}", output.join(report_name).display());
        }
        Err(error) => {
            eprintln!("magic_characters_capture: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> String {
    arguments
        .next()
        .unwrap_or_else(|| usage(&format!("missing value for {flag}")))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_value<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    flag: &str,
) -> T {
    let value = next_value(arguments, flag);
    value
        .parse()
        .unwrap_or_else(|_| usage(&format!("invalid value {value:?} for {flag}")))
}

#[cfg(not(target_arch = "wasm32"))]
fn usage(error: &str) -> ! {
    if !error.is_empty() {
        eprintln!("error: {error}");
    }
    eprintln!(
        "usage: magic_characters_capture [--phase 0|2|3|4|5|6|8|9] [--output DIR] [--seed N] [--pose-time SECONDS] \
         [--width PX] [--height PX] [--portrait-width PX] [--portrait-height PX] \
         [--quality full|half] [--capture-set full|stills|motion|review|hair] [--motion-mode staged|moving|moving-raised] \
         [--palette current|high-contrast] \
         [--avatar legacy|rounded|shape-proof|magic]"
    );
    std::process::exit(if error.is_empty() { 0 } else { 2 });
}
