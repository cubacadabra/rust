# Cubacadabra engine

This crate is the platform-neutral runtime shared by the browser and iOS
clients. It owns simulation, movement, collision, world and launch-pad
behavior, the native Luau host, and the shared `wgpu` primitive renderer.

The repositories are separate by responsibility:

```text
first-game  -> declarative world manifest and portable Luau rules
rust        -> this runtime and C/WASM entry points
web         -> vanilla-JavaScript browser client and WASM renderer binding
ios_app     -> Swift client and native static-library adapter
backend     -> multiplayer Worker and world WebSockets
```

The runtime does not fetch the game package or open the multiplayer socket.
Each client loads `first-game`, passes its manifest and script into Rust, sends
input to the engine, and presents the resulting frame. When starting here,
read [web/README.md](../web/README.md) next for the browser integration, then
[ios_app/README.md](../ios_app/README.md) for the native integration.

## Build the browser renderer

The browser build is driven from `web/`. Install the one-time binding tool,
then run the web script:

```sh
cargo install wasm-bindgen-cli
cd ../web
npm install
npm run build:renderer
```

The script adds the `wasm32-unknown-unknown` target when needed, builds this
crate with the `web-renderer` feature, and writes generated files to
`web/public/wasm/renderer/`. These are local build artifacts; the Rust source
and `scripts/build_web_renderer.sh` remain the source of truth. `npm run dev`
and `npm run build` in `web/` invoke this command automatically.

For engine-only Rust checks:

```sh
cargo test
cargo check
```

## Build for iOS

Xcode invokes `ios_app/scripts/build_rust_engine.sh` as a build phase. It
compiles this crate for the selected device or simulator architecture and
produces a native static library under Xcode's derived data. The Swift app
calls the functions declared in
`include/cubacadabra_engine.h` through the C-compatible ABI.

The native lifecycle is: create one engine, load the package and script, submit
input, advance it, read the frame snapshot, sync/draw the renderer, and destroy
the engine. The core data model does not depend on browser or native surface
types.

## Run modes

Rust has no standalone LAN or production server. The browser client selects
the backend with `VITE_BACKEND_WS_URL`, and the iOS client selects its package
and backend with Xcode environment variables. For a complete local or LAN
session, follow [web/README.md](../web/README.md) and
[backend/README.md](../backend/README.md); for an iOS session, follow
[ios_app/README.md](../ios_app/README.md). Production clients use the same
engine binaries but load the deployed package and connect to the deployed
Worker.

## Shared in-game UI

Experience HUDs and in-game modals can be declared by Luau and are owned by the
engine. Rust performs responsive safe-area layout, pointer hit testing, state
updates, and an orthographic `wgpu` overlay pass, so the same UI can render on
iOS, Android, and the browser. Native shells continue to own OS presentation
and forward host-service actions from the engine's UI event queue.

See [docs/ui-runtime.md](docs/ui-runtime.md) for the Luau document model,
semantic icons, header and bottom-center regions, menus/modals, responsive
layout rules, and C ABI integration.

## Scripting status

All targets execute `game.luau` through the host in `scripting.rs`. Native
builds use `mlua` with vendored Luau; the `wasm32-unknown-unknown` build uses
the pure-Rust `luaur-rt` Luau runtime so the browser can run the same lifecycle
callbacks without a separate JavaScript scripting implementation. Both hosts
expose the same sandboxed `lobby`, `session`, and lifecycle API.

## Source layout

- `engine.rs` — simulation lifecycle, camera state, and frame snapshot
- `renderer.rs` — shared `wgpu` primitive renderer
- `player.rs` — locomotion, gravity, and collision resolution
- `npc.rs` — agent spawning, roaming, separation, and assembly behavior
- `game_package.rs` — manifest/world data model
- `scripting.rs` — native Luau lifecycle host and browser seam
- `ui.rs` — retained UI document, layout, hit testing, and event queue
- `world.rs` — starter-world bounds and navigation points
- `types.rs` and `math.rs` — shared simulation primitives
- `ffi.rs` — C/WASM engine entry points
- `web_renderer.rs` — `wasm-bindgen` wrapper used by the browser

## Where to look next

- [web/README.md](../web/README.md) — generated WASM binding, browser shell,
  and package loading
- [ios_app/README.md](../ios_app/README.md) — C ABI integration and Xcode build
- [first-game/README.md](../first-game/README.md) — the content and Luau rules
