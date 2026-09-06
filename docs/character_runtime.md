# Character runtime and verification

Art decisions live in [Character art direction](character_art_direction.md).
These reusable engine contracts are independent of visual approval.

## Rendering and assets

The renderer owns a fixed indexed-mesh catalog, batches instances by
mesh/material and selects three LODs by projected size. Bounds: 50 renderer-only
characters, 48 parts per recipe, 384 meshes, 32 MiB of character mesh/instance
buffers. These are engineering limits, not demonstrated mobile frame-time budgets.

The hierarchy has 15 joints. The person hoodie uses authored contours in
`src/renderer/hero_geometry.rs` and a fitted presentation pose in
`hero_character.rs`. Its continuous sleeve bends around the existing elbow
in the vertex shader; spare normal-row w lanes carry axis-angle. The 128-byte,
11-attribute instance layout is unchanged. Other garments retain rigid
attachments. No new texture download or runtime mesh regeneration is needed.

Analytic faces use a depth-tested alpha pass. Solids write depth; event effects
test without writing depth. Character lighting works in linear space, then
returns display-encoded RGB to the existing scene target. Capability-selected
MSAA keeps a 1x fallback.

`legacy` and `magic` remain renderer/API values. Existing species/outfit IDs,
fit metadata, legacy colors, snapshot layout, physics dimensions and movement
speeds remain compatible. Asset fixtures do not establish an approved style.

## Presentation and identity

Typed motion drives presentation independently of simulation. Stable
key/generation and sequences prevent duplicate events or animation transfer
on roster reorder. Discontinuities reset cosmetic state. Reduced effects
suppress event sparks. People do not expand joint gaps in ordinary movement.

Local appearance changes are bounded and atomic, with monotonic revisions.
The person's gait shares distance/phase timing between simulation and fitted
presentation. Walking uses half-cycle contacts; running lengthens the stride
and adds flight time. World ankle targets continue through recovery and settle
after stopping. Both leg bones retain their lengths during the 3D solve;
unreachable targets are bounded instead of stretching the ankle attachment.
Torso/head and hair lag use bounded analytic springs. The hair cap remains
attached to the head; individual locks rotate around buried roots.

The version-1 remote JSON envelope is capped at 64 KiB and 17 remote slots.
Packets carry stable opaque IDs, generation, sequence, optional motion/support
data and appearance revisions. Older packet/motion sequences are rejected;
emotes have their own sequence and are not replayed by retransmission.

`engine_reset_remote_session` resets packet lifetime while retaining the bounded
appearance cache. World IDs hide other-world rosters without losing identity.
See [the native header](../include/cubacadabra_engine.h) for buffer ownership,
exact limits, status values and JSON entry points. Existing setters and the
eight-float snapshot remain valid. Browser helpers use the same bounded buffers.

Host persistence, reconnect and two-client behavior require host integration
tests. Rust renderer success does not establish those results.

## Reproduce current evidence

Write bulk captures under `target/` or `/tmp/`. Keep selected current images
and compact measurements in docs.

```sh
cargo test --features dev-showcase --lib
cargo run --features dev-showcase --bin validate_character_assets
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 9 --width 1440 --height 900 --output /tmp/person-review
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 4 --width 1280 --height 720 --output /tmp/person-motion
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 3 --output /tmp/character-validation/metal
sh scripts/build_character_validation.sh
node scripts/validate_character_browser.mjs webgpu /tmp/character-validation/webgpu
node scripts/validate_character_browser.mjs gl /tmp/character-validation/gl
```

Phase numbers remain CLI compatibility names. Phase 9 contains person views,
proportion studies and greeting. Phase 4 stages shared walk/run/jump/landing/
wave motion. Presentation replays at 60 Hz and saves 30 fps frames. Phase 0
checks gameplay cameras and raised supports; phase 5 checks wardrobe fits;
phase 3 exercises color, occlusion, effects, 1x/AA, resource limits and surfaces.

Review 390×844, 768×1024, 1280×800 and 1440×900, including actual portrait
letterboxing, daylight, first/third person and raised platforms. Sustained
phone performance and input feel need physical devices. Submit/readback
capture time is not a GPU-only frame-time measurement.
