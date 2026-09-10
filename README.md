# cubacadabra Rust workspace

This workspace is the platform-neutral runtime shared by Studio, iOS,
Android, and the browser. It owns simulation, movement, collision, world and
launch-pad behavior, Luau, rendering, and the engine-facing multiplayer client
session and native-presented application state.

It has three crates:

- `cubacadabra-engine` (the root package) owns deterministic game/runtime state.
- `cubacadabra-client` (`crates/client`) owns package startup, multiplayer
  protocol decoding, remote-player projection, launch/world routing, and the
  Luau network outbox. It exposes Rust types to Studio, C to iOS/Android, and a
  `wasm-bindgen` class to the browser.
- `cubacadabra-app` (`crates/app`) owns semantic product state outside active
  gameplay. Its first slice centralizes account username validation and save
  state while SwiftUI, Compose, and the DOM remain native.

The repositories are separate by responsibility:

```text
game packages -> declarative world manifests and portable Luau rules
rust          -> engine + shared client/app state + C/WASM adapters
studio        -> direct Rust host and native socket/window integration
ios_app       -> Swift UI, Apple services, socket, and C adapter
android_app   -> Kotlin UI, Android services, socket, and JNI adapter
web           -> JavaScript UI, browser services, socket, and WASM adapter
backend       -> multiplayer Worker and world WebSockets
```

Rust deliberately does not fetch packages or open sockets. Each host provides
manifest/script text and transports the `SetWorld` and `SendText` actions
returned by `ClientSession`; every received socket text frame is passed back to
that session. See [docs/client-runtime.md](docs/client-runtime.md) for the
boundary and integration contract.

The app crate follows the same host-driven boundary for product features: a
host dispatches typed actions, renders a serializable snapshot, performs queued
effects with its native services, and returns typed results. See
[docs/app-runtime.md](docs/app-runtime.md) for the current username/profile
slice and migration order.

## Build the browser renderer

The browser build is driven from `web/`. Install the one-time binding tool,
then run the web script:

```sh
cargo install wasm-bindgen-cli
cd ../web
npm install
npm run build:renderer
```

The script adds the `wasm32-unknown-unknown` target when needed, builds the
client crate with the `web-renderer` feature, and writes generated files to
`web/public/wasm/renderer/`. These are local build artifacts; the Rust source
and `scripts/build_web_renderer.sh` remain the source of truth. `npm run dev`
and `npm run build` in `web/` invoke this command automatically and use a
debug WASM build. The deployment helper uses `npm run build:release` so only
deployed builds use the optimized release WASM.

For workspace checks:

```sh
cargo test
cargo check
```

## Git hooks

Enable the repository's pre-commit hook once per checkout:

```sh
git config core.hooksPath .githooks
```

When a commit includes Rust source, the hook runs `cargo fmt --all` and
auto-stages formatting changes for Rust files that were already staged. Files
with separate unstaged changes must be staged or discarded before committing.

## Build for iOS

Xcode invokes `ios_app/scripts/build_rust_engine.sh` as a build phase. It
compiles `cubacadabra-client` and `cubacadabra-app` for the selected device or
simulator architecture and produces native static libraries under Xcode's
derived data. The Swift app creates a client session through
`include/cubacadabra_client.h` and application state through
`include/cubacadabra_app.h`; the client's borrowed engine pointer continues to
use the lower-level engine/rendering ABI.

The native lifecycle is: create one client from a manifest and script, submit
transport events and input, dispatch client actions, advance/read the engine,
sync/draw the renderer, and destroy the client. The engine pointer is owned by
the client and must not be destroyed separately.

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

See [docs/network-runtime.md](docs/network-runtime.md) for the generic
game-owned message and retained-state contract. The runtime transports opaque
JSON and does not contain rules for a particular game.

See [docs/audio-runtime.md](docs/audio-runtime.md) for package-owned one-shot
WAV assets, the Luau playback call, validation limits, and host polling ABI.

See [docs/effects-runtime.md](docs/effects-runtime.md) for versioned,
manifest-authored world effects and the small Luau state/play API. Rust owns
bounded rendering primitives; games own their visual recipes.

## Scripting status

All targets execute `game.luau` through the host in `scripting.rs`. Native
builds use `mlua` with vendored Luau; the `wasm32-unknown-unknown` build uses
the pure-Rust `luaur-rt` Luau runtime so the browser can run the same lifecycle
callbacks without a separate JavaScript scripting implementation. Both hosts
expose the same sandboxed `lobby`, `session`, `interactions`, `effects`, and
lifecycle API.
Packages keep lobby routing enabled by default. Set `"lobby": false` in the
manifest, or call `api.lobby:set_enabled(false)` from `on_start`, to enter the
configured experience world directly. Direct worlds use the normal per-world
instance allocator and capacity rules. Worlds can declare generic interaction
zones with an id, kind, label, position, radius, and optional palette color.
Rust tracks proximity and player counts; Luau receives `on_interaction` enter
and exit callbacks and reads `api.interactions:get_state()`. Game rules such as
spells, treasures, doors, and checkpoints remain entirely in Luau.

## Source layout

- `engine.rs` — simulation lifecycle, camera state, and frame snapshot
- `crates/client` — shared package/session/protocol facade and C/WASM adapters
- `crates/app` — shared non-game application state, snapshots, and host effects
- `renderer.rs` — shared `wgpu` primitive renderer
- `player.rs` — locomotion, gravity, and collision resolution
- `npc.rs` — agent spawning, roaming, separation, and assembly behavior
- `engine/interactions.rs` — generic world interaction zones and enter/exit events
- `game_package.rs` — manifest/world data model
- `scripting.rs` — native Luau lifecycle host and browser seam
- `ui.rs` — retained UI document, layout, hit testing, and event queue
- `world.rs` — starter-world bounds and navigation points
- `types.rs` and `math.rs` — shared simulation primitives
- `ffi.rs` — C/WASM engine entry points
- `web_renderer.rs` — `wasm-bindgen` wrapper used by the browser

## Game categories

  1. Obby / precision platformer — jumps, ladders, checkpoints, falling.
  2. Survival / crafting / base defense — gather, manage resources, return to safety, improve the base.
  3. Exploration / adventure — discover areas, quests, secrets, NPCs.
  4. Co-op puzzle / escape room — synchronized switches, logic, shared objectives.
  5. Combat / PvE / PvP — enemies, weapons, abilities, arenas.
  6. Racing / time trial — laps, checkpoints, timers, ghosts.
  7. Tycoon / management — production, upgrades, automation.
  8. Social sandbox / roleplay — shared spaces, identity, emotes, player-driven activities.
  9. Collection / pets / creature raising — collect, upgrade, care for, trade.
  10. Round-based party games — short competitive or cooperative mini-games.
  11. Builder / creative sandbox — place, modify, and collaboratively build.
  12. RPG / progression adventure — quests, stats, inventory, unlocks.

## Where to look next

- [web/README.md](../web/README.md) — generated WASM binding, browser shell,
  and package loading
- [ios_app/README.md](../ios_app/README.md) — C ABI integration and Xcode build
- [first-game/README.md](../first-game/README.md) — the content and Luau rules
