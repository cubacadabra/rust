# Cubacadabra engine

This crate owns the platform-neutral game simulation and shared primitive
renderer. The browser builds the crate to WebAssembly and uses the same `wgpu`
renderer as iOS, which calls it through the C ABI. The simulation API remains
independent of browser and native surface types.

Game packages are separate from this runtime. They provide declarative world
content and Luau rules; this crate provides physics, reusable platform patterns,
and the host API exposed to those rules. The first package is in the sibling
`first-game` repository.

## Build the web engine

From `web/`, run:

```sh
cargo install wasm-bindgen-cli
npm run build:wasm

# Build the shared browser renderer (requires wasm-bindgen-cli)
npm run build:renderer
```

The command writes `web/public/wasm/cubacadabra_engine.wasm`; the production
build also generates the browser renderer binding under
`web/public/wasm/renderer`. Generated binaries are local build artifacts; the
source of truth remains this crate.

## Native direction

The exported functions use a C-compatible handle and scalar ABI, documented in
`include/cubacadabra_engine.h`, following the same ownership pattern as
Groupicorn's iOS renderer: create one engine, submit input, advance it, read
the frame, then destroy it. The core data model is independent of that ABI and
can later be wrapped by an iOS static library without changing gameplay code.

## Source layout

- `engine.rs` owns the simulation lifecycle, camera state, and frame snapshot
- `renderer.rs` owns the shared `wgpu` primitive renderer used by native and
  browser clients
- `player.rs` owns locomotion, gravity, and collision resolution
- `npc.rs` owns agent spawning, roaming, separation, and assembly behavior
- `scripting.rs` hosts the Luau lifecycle API for native clients and preserves
  the same engine seam for the browser runtime. Native builds execute Luau;
  the current `wasm32-unknown-unknown` build keeps the ABI seam while a
  dedicated Luau-WASM runtime is integrated.
- `world.rs` owns starter-world bounds and navigation points
- `types.rs` and `math.rs` hold shared simulation primitives
- `ffi.rs` is the only module that exposes the C/WASM entry points
