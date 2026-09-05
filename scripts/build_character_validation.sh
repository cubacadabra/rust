#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
crate_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
binding_tool=$(command -v wasm-bindgen || true)
if [ -z "$binding_tool" ]; then binding_tool="${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"; fi
rustup run stable cargo build --manifest-path "$crate_dir/Cargo.toml" --lib --release \
  --target wasm32-unknown-unknown --features web-renderer,dev-showcase
"$binding_tool" "$crate_dir/target/wasm32-unknown-unknown/release/cubacadabra_engine.wasm" \
  --target web --no-typescript --out-dir "$crate_dir/target/phase3-browser" --out-name cubacadabra_renderer
