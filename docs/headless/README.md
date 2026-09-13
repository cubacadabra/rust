# Headless engine proof

This fixture runs the same manifest/package and Luau lifecycle that an
interactive client loads, but the process creates no window, GPU device,
renderer, socket, or platform host.

From the repository root:

```sh
cargo run --release --no-default-features --features headless --bin headless -- \
  docs/headless/manifest.json docs/headless/game.luau
```

The runner advances a fixed 1,000-tick input trace twice and compares the
engine's deterministic state hash. Use `--ticks`, `--delta`, `--forward`,
`--strafe`, `--sprint`, `--jump-tick`, or `--no-jump` to vary the trace.

`--no-default-features` is deliberate: it verifies that the engine compiles
without the optional rendering, window, and GPU dependency path.
