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
back). Six `profile` values specify relative radii from root to tip; 0.5 gives
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
preview/reload tool are deferred until they have a concrete consumer.
