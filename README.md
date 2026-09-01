# Cubacadabra engine

This crate owns the platform-neutral game simulation. The first client is the
browser, where the crate is compiled to WebAssembly and Three.js consumes a
compact frame snapshot for rendering. The simulation API deliberately avoids
browser or renderer types so a future `ios_app` can call the same engine from a
native Rust bridge.

## Build the web engine

From `web/`, run:

```sh
npm run build:wasm
```

The command writes `web/public/wasm/cubacadabra_engine.wasm`. The generated
binary is a checked-in runtime asset so a fresh checkout can run the web app;
the source of truth remains this crate.

## Native direction

The exported functions use a C-compatible handle and scalar ABI, documented in
`include/cubacadabra_engine.h`, following the same ownership pattern as
Groupicorn's iOS renderer: create one engine, submit input, advance it, read
the frame, then destroy it. The core data model is independent of that ABI and
can later be wrapped by an iOS static library without changing gameplay code.
