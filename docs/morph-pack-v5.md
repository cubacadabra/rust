# Morph pack v5 (pre-launch)

The compiler emits **only v5** and the shared runtime accepts **only v5**.
Schemas 1–4 fail with `MORPH_PACK_UNSUPPORTED_SCHEMA`; rebuild from source.
There is no legacy-normal reconstruction, vertex welding, or smoothing path in
the runtime. Missing source normals are an authoring error, not a reason for the
compiler to guess shading.

## Binary layout

All scalar fields are little-endian. Existing bounded limits remain enforced.

- Header: `CUBAMORP` (8 bytes), schema `u16 = 5`, reserved flags `u16`,
  manifest length `u32`, manifest JSON bytes.
- Textures: count `u8` (0–4), then width/height `u16`, byte length `u32`, RGBA8
  pixels for each texture (maximum dimension 512).
- Three LODs, in Near/Mid/Far order:
  - triangle count, vertex count, index count (`u32` each);
  - surface count `u16`, followed by contiguous index spans and explicit
    color/avatar-tint/texture flags, optional color and texture index;
  - `vertex_count` float VEC3 positions;
  - `vertex_count` float VEC3 **authored normals**;
  - `vertex_count` float VEC2 UVs (zero UVs for untextured source geometry);
  - for skinned attachments, `vertex_count` bindings: four `u16` joints and
    four float weights;
  - `index_count` `u32` indices.

Normals must be finite unit vectors (squared length tolerance 0.98–1.02).
They use the same local coordinate space as positions. The importer preserves
the float vectors, including different normals on coincident vertices. A GLB
must provide non-sparse float `NORMAL` VEC3 accessors matching every primitive's
`POSITION` count. Skinning rotates those normals with the pose and normalizes
the weighted result; rigid attachments use the existing inverse-transpose
normal transform. No normal recomputation occurs per frame.

This does not change the existing joint-local skin-position convention or add
general glTF inverse-bind-matrix import. The study currently authors single
joint weights; its bent-elbow/knee fit is not production-ready.

## Shared asset semantics

`rig.canonical-rest.v1` on a skinned base opts out of the procedural hero's bind
pose refit. `face.authored-static.v1` on a skinned base suppresses bundled face
graphics. Both are interpreted by the shared renderer on desktop, web, iOS,
and Android, not by the `studio-ui` feature. Textured cloth, denim, skin, hair,
and footwear also share the same material response on those paths.

## Rebuild and deployment

First-party source generators now export normals. Existing hair/accessory GLBs
already did; the eight body/clothing GLBs and the compiler's top-hat fixture
were regenerated. Generated packs are not checked into the source repositories.
Both the main starter release and isolated study are rebuilt locally.

From `tools/`:

```sh
PYTHONPATH=src python3 -m cubacadabra morph build
python3 starter-set/review_study.py --generate --motion
```

Rebuild any additional local fixture packs from their source GLBs before use.
Old hash-addressed cache objects are not rewritten or silently upgraded. Local
backend catalogs/R2 must be refreshed with the existing `setup-local` workflow
before running against the new packs. No upload or database change is part of
this source-format migration.

The 409,600-vertex frame budget and CPU skinning remain unchanged. Whole-character
LOD fallback under crowd pressure and GPU skinning are separate follow-up work.

## Verification (2026-09-12)

- Engine workspace: 190 passing tests; engine also passes with `studio-ui`.
- Authoring compiler: 12 passing tests, including exact source-normal round
  trip, coincident hard edges, malformed accessors, and stale-schema rejection
  in the shared decoder tests. Studio: 28 passing tests.
- Tools: 34 passing tests, including required unit normals on every source LOD.
- Actual GPU captures: all 24 starters, plus the study at Near/Mid/Far and
  walk/elbow/jump-like stress poses. Study tests assert every part was admitted,
  canonical pose positions match, source normals are transformed directly, and
  bundled face graphics are suppressed. Studio/non-Studio still captures match
  byte-for-byte on the same machine.
- Compile checks pass for normal macOS, Studio, Android backend features,
  `wasm32-unknown-unknown` with `web-renderer`, `aarch64-apple-ios` with Metal,
  and `aarch64-linux-android` with `android-backends`.

Cross-target checks use the installed rustup toolchain (the default Homebrew
Rust installation lacks those sysroots). Android additionally uses NDK
28.2.13676358's `aarch64-linux-android24-clang/clang++` and `llvm-ar` via the
target-specific `CC`, `CXX`, and `AR` environment variables. These are compile
checks, not device performance or browser gameplay tests.
