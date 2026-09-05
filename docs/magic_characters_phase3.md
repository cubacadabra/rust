# Phase 3: production character submission and base materials

Implemented September 5, 2026. The three Phase 2 bodies now use indexed,
instanced GPU submission in native and browser gameplay. The eight-float
snapshot, engine capacity, collision behavior and appearance schema are
unchanged. Phase 2's CPU-expanded comparison remains development-only.

## Submission and resource contract

- Character vertices are 32 bytes (position, normal, UV). Instances are 128
  bytes (three affine rows, three inverse-transpose normal rows, tint and
  material parameters). The layout uses two buffers and 11 of the requested
  16 attribute locations. Draws use buffer slices with zero `first_instance`
  for the GL path; no storage, compute, indirect or optional features are needed.
- Renderer construction compiles all three bundled bodies into a closed catalog
  of 35 immutable indexed meshes. Batches share meshes/materials across species
  and colors. Pose evaluation uses fixed joint arrays; CPU staging and instance
  GPU buffers are reserved once. Normal frames upload instances only.
- The current catalog is bounded at 64 meshes, 48 parts per body, 50 render-only
  characters and 32 MiB of character buffers. Actual geometry plus instance
  capacity is 548,784 bytes, with 633,600 bytes of CPU instance staging capacity.
  Unused character slots do not produce draws. The engine still supports 18.
- Package changes and color changes reuse this closed catalog; they cannot add
  cache entries. All GPU handles belong to the renderer and drop with it. Future
  external wardrobe assets need Phase 5's validation/residency policy rather
  than admitting arbitrary mesh keys into this cache.

## Materials and draw order

Toy skin, cloth, denim and rubber have separate roughness/specular presets.
Lighting is diffuse plus restrained rim and a bounded specular approximation;
seam cores use unlit emission. This is the base material foundation. Texture
detail, garment materials, wardrobe recipes and advanced lighting remain in
their later RFC phases.

The 3D pass draws opaque world geometry, culled character solids, head-local
face geometry, additive seam cores, then sorted translucent world triangles.
Solids and faces test/write depth. Seams and conventional translucency test
depth without writing it. Alpha world triangles are sorted back to front in
view space. Translucent world surfaces composite over emission, so a receiver
in front of a seam still covers/tints it. This establishes explicit ordering
for the current core effects; intersecting future particle systems need their
own transparency policy.

`visible-emission.png` deliberately enlarges one seam in front of a head.
The acceptance fixture verifies that it visibly changes pixels, is fully
occluded by an opaque wall, and can be overwritten by a later opaque draw
(proving it does not write depth). Ordinary character captures retain the
authored small cores.

## Color and antialiasing contract

Authored palettes/tints are display-encoded sRGB values. Character shading
decodes tints for linear lighting, then explicitly encodes the result. The
existing world shader retains its historical encoded-value lighting.

Both write to an `Rgba8Unorm` compatibility target. The unchanged unorm UI atlas
and authored UI colors blend into that target after 3D MSAA resolves. A final
single-sample fullscreen pass copies encoded values to an unorm surface, or
decodes them before writing to a hardware-encoding sRGB surface. This preserves
native world/UI appearance and gives browser surfaces the same encoded result.
It deliberately does not change the world's historical shading/blending model.

MSAA selects 4x only when the adapter advertises color 4x plus resolve and depth
4x support. Otherwise it uses 1x. All 3D pipelines share the selected count;
UI/presentation are always 1x. Resize and lost/outdated-surface recovery recreate
matching resolved color, optional multisample color, depth and presentation
bindings. Color/depth targets are separate from character residency: at
1280×800 they total 7.81 MiB at 1x or 35.16 MiB at 4x, excluding the surface.

Browser execution also verified/fixed three existing portability issues:
explicit LOD sampling for the one-mip UI atlas after a varying branch, an
explicit browser display handle for GL surface creation, and WebGL2 device
limits without compute requirements. Browser startup now probes usable WebGPU
before selecting its GL fallback.

## Reproduce the acceptance suite

```sh
cargo run --release --features dev-showcase --bin magic_characters_capture -- --phase 3
sh scripts/build_character_validation.sh
node scripts/validate_character_browser.mjs webgpu
node scripts/validate_character_browser.mjs gl
```

Phase 3 runs a fixed acceptance suite (its CLI option is `--output`, independent
of Phase 0/2's palette/pose options). The browser script uses Node's standard
library and an isolated headless Chrome profile. Set `CHARACTER_CHROME` to a
different Chrome executable when necessary. It does not use the host site's
development server. The build writes opt-in WASM bindings under
`target/phase3-browser`; normal host builds exclude the capture exports.

Each backend captures 1x/4x lineups, 18/50-character crowds, 390×844 portrait,
768×1024 tablet, 1280×800 laptop and 1440×900 desktop targets, plus color and
effect probes. Portrait preserves the existing centered landscape world
viewport. Browser runs additionally submit eight real surface frames while
resizing and entering first person, on devices requesting zero optional
features. The offscreen measurements exclude simulation/networking.

Five warmup frames precede 60 measured frames per scene. Reports include
median/p95 CPU pose/batching/upload cost, draw/triangle/upload counts, catalog
uploads, GPU/CPU residency, target memory, CPU catalog initialization, and GPU
pass timings when reliable. CPU staging capacities and GPU catalog residency
must stay constant through the warm frames and viewport changes. GPU queries
are diagnostic only. Invalid timestamp pairs are excluded and a summary needs
at least 90% valid samples. Submit-and-readback completion includes CPU,
presentation, GPU work and browser scheduling; it is not isolated GPU time.

## Recorded evidence

Development device: Apple M4 Max, macOS 26.6.2 (25G83). Browser: headless Chrome
152. WebGPU adapter details are privacy-redacted; forced browser GL identifies
ANGLE's Metal renderer on the M4 Max. These are three backend paths on one
desktop, not three independent device baselines.

| Evidence | Report | Representative capture |
| --- | --- | --- |
| Native Metal | [measurements](baselines/magic-characters/phase3/native/phase3_report.json) | [18 characters, 4x](baselines/magic-characters/phase3/native/crowd-18-4x.png) |
| Browser WebGPU | [measurements](baselines/magic-characters/phase3/webgpu/phase3_report.json) | [color/UI](baselines/magic-characters/phase3/webgpu/color-srgb.png) |
| Browser GL | [measurements](baselines/magic-characters/phase3/gl/phase3_report.json) | [three bodies, 4x](baselines/magic-characters/phase3/gl/lineup-4x.png) |

The 18-character scene submits 546 instances, 35 draws, 68,760 triangles and
69,888 bytes/frame. Fifty mixed bodies submit 1,513 instances, still 35 draws,
and 193,664 bytes/frame. These are base bodies at the existing two-subdivision
recipe tier; projected-size LOD tuning and fully dressed budgets are later
phases. The base submission is well below the 100-draw/0.5-MiB upload ceilings.

Recorded 18-character CPU p95 is below 0.2 ms on all three paths, against the
provisional 2 ms budget. Browser WebGPU's full 3D pass is below 0.2 ms p95 at
1280×800/4x, already below the 4 ms incremental-character target. Native
submit-through-readback completion is below 2 ms, but Metal timestamp pairs
are frequently invalid, so incomplete timestamp samples cannot establish a
GPU percentile. GL has no timestamp queries on this adapter; its GPU-only
budget is unverified. Browser readback polling includes event-loop delays.

Within every backend, unorm/sRGB output comparisons, legacy direct-unorm
presentation comparisons, opaque occlusion and emission depth-write probes
have maximum channel error zero. The fixture checks within-backend regression;
cross-backend bit identity is not an acceptance requirement. The lineup and
portrait artifacts were visually inspected for culling, intact faces, material
color, letterboxing and crisp UI.

Validation also includes `cargo test`, `cargo test --features dev-showcase`,
`cargo check`, WASM checks with `web-renderer` (and the opt-in fixture), and both
`scripts/build_web_renderer.sh --debug` and `--release`. The latter regenerate
the sibling web checkout's browser bindings; release is the final output.
The installed rustup stable toolchain supplies WASM; the shell's Homebrew Rust
installation has no WASM standard library, so WASM checks use
`rustup run stable cargo check --target wasm32-unknown-unknown --features web-renderer`.

The four Phase 3 implementation items are complete. Platform-owner budget
ratification, physical iOS/Android captures, native surface recovery on actual
hosts, forced device-loss recovery and sustained mobile performance are not
claimed by this desktop evidence. The later rollout/default-selection and
wardrobe gates remain unchanged.
