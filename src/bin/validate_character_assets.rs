#[cfg(not(target_arch = "wasm32"))]
use cubacadabra_engine::dev_showcase::{validate_phase5_catalog, validate_style_examples};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let mut arguments = std::env::args().skip(1);
    let style_examples = matches!(arguments.next().as_deref(), Some("--style-examples"));
    let path = arguments
        .next()
        .map(PathBuf::from)
        .or_else(|| (!style_examples).then(|| PathBuf::from("assets/characters/catalog.json")));
    if style_examples {
        let path = path.unwrap_or_else(|| PathBuf::from("assets/characters/soft_cubism_examples.json"));
        match validate_style_examples(&path) {
            Ok(report) => println!(
                "valid Phase 8 style examples: schema={} examples={}",
                report.schema_version, report.example_count
            ),
            Err(error) => {
                eprintln!("character style validation failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let path = path.unwrap_or_else(|| PathBuf::from("assets/characters/catalog.json"));
    match validate_phase5_catalog(&path) {
        Ok(report) => println!(
            "valid Phase 5 catalog: schema={} outfits={} materials={} licenses={} texture_bytes={}",
            report.schema_version,
            report.outfit_count,
            report.material_count,
            report.license_count,
            report.texture_bytes
        ),
        Err(error) => {
            eprintln!("character asset validation failed: {error}");
            std::process::exit(1);
        }
    }
}
