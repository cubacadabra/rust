#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
crate_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
web_dir=$(CDPATH= cd -- "$crate_dir/../web" && pwd)
target_dir="$crate_dir/target"
output_dir="$web_dir/public/wasm/renderer"

wasm_bindgen_command=$(command -v wasm-bindgen || true)
if [ -z "$wasm_bindgen_command" ] && [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen" ]; then
  wasm_bindgen_command="${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"
fi
if [ -z "$wasm_bindgen_command" ]; then
  echo "wasm-bindgen is required to build the browser Rust renderer." >&2
  echo "Install it with: cargo install wasm-bindgen-cli" >&2
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown
  cargo_command="rustup run stable cargo"
else
  cargo_command="cargo"
fi

mkdir -p "$output_dir"
$cargo_command build \
  --manifest-path "$crate_dir/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --release \
  --features web-renderer
"$wasm_bindgen_command" \
  "$target_dir/wasm32-unknown-unknown/release/cubacadabra_engine.wasm" \
  --target web \
  --no-typescript \
  --out-dir "$output_dir" \
  --out-name cubacadabra_renderer
echo "Built $output_dir/cubacadabra_renderer.js"
