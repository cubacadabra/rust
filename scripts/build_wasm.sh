#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
crate_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
web_dir=$(CDPATH= cd -- "$crate_dir/../web" && pwd)
target_dir="$crate_dir/target"
output_dir="$web_dir/public/wasm"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to build the Cubacadabra WebAssembly engine." >&2
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown
  cargo_command="rustup run stable cargo"
else
  cargo_command="cargo"
fi

mkdir -p "$output_dir"
$cargo_command build --manifest-path "$crate_dir/Cargo.toml" --target wasm32-unknown-unknown --release
cp "$target_dir/wasm32-unknown-unknown/release/cubacadabra_engine.wasm" \
  "$output_dir/cubacadabra_engine.wasm"
echo "Built $output_dir/cubacadabra_engine.wasm"
