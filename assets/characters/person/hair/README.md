# Human hair authoring

`ponytail.json` and `shag.json` hold the girl's and nonbinary player's hair
geometry and color. The boy keeps the original swept hairstyle. All three
continue to use the existing body IDs and character-lab appearance path.

Each new hairstyle has a scalp cap and curved locks. A lock's four `points`
are the root, two Bezier controls, and tip, measured in head-local engine
units. The face points toward -Z; +Y is up. From the front, negative X appears
on the viewer's right. The person head is roughly 1.02 units wide and 0.94 tall.

`width` and `depth` scale the lock's elliptical cross section. `outward`
controls which way its broad surface faces (usually -Z for bangs, +Z at the
back, +/-X for side layers). Keep surface layers broad and shallow: `depth`
is usually much smaller than `width`. Six `profile` values specify relative radii from root to tip; 0.5 gives
the full authored width/depth. A small final value makes a tapered tip.
`sway` scales head-following motion about the root. Keep roots buried in the
cap or overlapping another lock, and put free ponytail/side tips outside the
head silhouette. Controls shape a curve; the curve does not pass through them.

Hair RGB colors are normalized floats in the same color space as existing
character tints. The files are bounded to 20 locks and 64 KiB and are parsed
once per process. Invalid development overrides log a warning and fall back
to the embedded asset.

Native **debug** builds read these source files at startup. After the first
build, edit the JSON and relaunch the existing executable directly to avoid
compilation. `CUBACADABRA_HAIR_DIR` can point to another directory containing
the same filenames. This is startup loading, not live reload; edits require
a process restart. WASM and release builds use embedded JSON and require a
rebuild to pick up changes.

The data types/loader live in `src/character/hair.rs`; curved mesh generation
lives in `src/renderer/hair_geometry.rs`. New locks need only data changes.
Registering another hairstyle currently requires adding an asset and mapping
it in the finite catalog; independent hairstyle selection can later replace
the body-to-style mapping without duplicating the rig or mesh generator.

This follows `docs/split_hot_crates.md`'s separation of art parameters from
algorithms. JSON uses the existing serde_json dependency; RON would add a
parser without changing the iteration workflow. Crate separation and a live
reload tool are deferred until they have a concrete consumer.

## Single-image visual loop

Build the existing capture executable once:

```sh
cargo build --features dev-showcase --bin magic_characters_capture
```

Then run only the hair capture set after each data edit:

```sh
target/debug/magic_characters_capture --phase 9 --capture-set hair --width 1440 --height 900 --output target/character-review/hair
```

This renders **one** `hair-review.png`: girl above nonbinary, with front,
side, and back views from left to right in one scene. It uses the production
meshes and materials at the mid LOD, with head-only framing. Each invocation
overwrites that same fixed-size image; inspect it before the next edit.
No motion sequence, size sweep, integration tests, or other capture sets run.
Rebuild only for Rust changes or to embed final JSON for packaged builds.
