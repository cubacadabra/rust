#[cfg(not(target_arch = "wasm32"))]
use cubacadabra_engine::dev_showcase::validate_phase5_catalog;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/characters/catalog.json"));
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
