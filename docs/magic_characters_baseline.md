# Magic Characters Phase 0 baseline

This is the repeatable before-image fixture for the magic-characters RFC. It
captures the current legacy CPU-expanded avatar and shader path; it does not
enable local NPC simulation, alter the engine capacity, or connect a backend.

Generate the current baseline with:

```sh
cargo run --features dev-showcase --bin magic_characters_capture
```

The command writes PNG captures and `phase0_report.json` to
`docs/baselines/magic-characters/phase0/`. Use `--output` for a disposable
directory. The fixture is deterministic for `--seed`, `--pose-time`, palette,
quality, and dimensions; adapter metadata and measured timings are recorded by
the report.

The default suite includes local and remote idle/walk/sprint/jump captures,
first-person entry/exit coverage, a raised-platform capture, an 18-character
landscape scene, an isolated 50-character render-only stress scene, and both
scenes at 390x844 in the existing centered 16:9 portrait letterbox. Pass
`--portrait-width` and `--portrait-height` when a host's portrait viewport
differs.

## Baseline ownership and provisional targets

| Path | Capture owner | Phase 0 status | Initial target carried from RFC |
| --- | --- | --- | --- |
| Native Metal | Rust renderer owner | Recorded by the local capture report | 18 visible characters, 60 fps target; character incremental GPU <= 4 ms p95 |
| Browser WebGPU/GL | Web renderer owner | Requires sibling `web` checkout and browser capture | Same target; validate WebGPU and GL separately |
| iOS Metal | iOS host owner | Requires Xcode/device capture | Same target on the named baseline device |
| Android GLES | Android host owner | Requires maintained Android host/device capture | Same target on the named baseline device |

The report records CPU geometry/build time, vertex upload bytes, estimated
render-target/readback resources, draw vertex/triangle counts, and submit plus
readback time. GPU timestamp queries are intentionally not requested by the
current production renderer, so the report records them as unavailable rather
than treating CPU wall time as GPU time. The 50-character scene is a renderer
stress measurement only; the production `Engine` remains capped at 18 total
characters including the local player.

## Snapshot ABI fixture

The public snapshot remains 18 records of eight `f32` values (stride 8). The
suffix meanings are intentionally entity-specific and are covered by
`snapshot_abi_fixture_preserves_entity_suffix_meanings`:

| Entity | Slot 5 | Slot 6 | Slot 7 |
| --- | --- | --- | --- |
| Local player | grounded | moving | sprinting |
| Local NPC | phase code | meeting index | assembled at meeting |
| Remote player | constant `1.0` | constant `-1.0` | constant `0.0` |

Renderer code must not interpret slot 7 as a universal `assembled` flag. The
current legacy draw path still does so deliberately for the before-image; the
typed motion correction belongs to Phase 1.
