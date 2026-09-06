# Green-hoodie hero, v1

[Concept target](concept-v1.png) · [Engine view](hero-three-quarter.png) ·
[Face](hero-face.png) · [Side](hero-side.png) · [Greeting video](hero-greeting.mp4)

The concept was generated with the built-in imagegen tool, using the supplied
Groupicorn artwork as a reference for garment volume and expressive character
design. The [exact prompt](concept-prompt.txt) is preserved. The concept is
illustration; the other linked images and video are captures of the Rust
renderer. They should be compared as target and implementation, respectively.

## Implemented asset

The person wearing `EverydayHoodie` now uses a coordinated hero fit. Existing
appearance colors remain player-controlled; the review uses emerald cloth,
navy shorts, warm skin, and green sneakers.

- Authored cheek/jaw, shoulder, sleeve, pocket, hood, hair, hand, and shoe
  contours replace the corresponding rounded-box surfaces. The hood has a
  hollow rolled rim and a separate back. Sleeves have broad modeled folds;
  cuffs/hem have filtered ribbing. Pocket openings, blunt cords, thumbs, and
  sneaker laces give the outfit readable construction.
- Hero eyes have cream whites, teal irises, directional pupils, highlights,
  and a defined upper lid. Eye opening and brow height can vary by side. The
  wink now closes one eye more than the other. Brows and eye contours have
  filtered coverage; expressions retain the common presentation system.
- Warm key light and cool environment fill describe the hero's surfaces.
  The inside of the hood receives less fill. Cloth, skin, hair, and rubber
  have distinct responses. Fine material patterns fade at small screen sizes.
- A relaxed stance, head-first attention shift, eased wave, wrist motion,
  and damped hood movement play through the existing rig. Soft receiver-aligned
  shadow discs replace the previous two hard-edged shadow layers in gameplay.

The shapes are authored as profiles in
[`hero_geometry.rs`](../../../src/renderer/hero_geometry.rs), with the fit and
attachments in [`hero_character.rs`](../../../src/renderer/hero_character.rs).
Meshes are sampled once at three bounded LODs, shared in unit space, and scaled
by instance transforms. Small cords, cuffs, and laces receive fewer vertices
than heads. No runtime mesh regeneration, importer, texture dependency, or
physics/collider change was introduced.

## Reproduce the review

```sh
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 9 --width 1440 --height 900 --output /tmp/green-hoodie-hero-final
ffmpeg -framerate 30 -i /tmp/green-hoodie-hero-final/hero-%04d.png \
  -c:v libx264 -crf 18 -pix_fmt yuv420p -movflags +faststart \
  /tmp/green-hoodie-hero-final/hero-greeting.mp4
```

Phase 9 saves seven static views plus 301 frames at 30 fps. Presentation runs
at 60 Hz. The sequence notices the viewer, turns and smiles, waves at 3 seconds,
grins, winks at 5.5 seconds, and settles. It uses the gameplay animator and GPU
character path. The studio colors/camera are review fixtures. Antialiasing uses
the production capability check; the supplied captures use 4× MSAA. Each capture
records its sample count and includes multisample attachments in its resource
estimate. `--pose-time` is superseded by the review timeline.

## Verification and remaining gap

112 Rust library tests pass, including authored mesh bounds/normals/indices,
shadow receiver bounds and edge opacity, expression bounds, the greeting,
and existing engine/presentation regressions. The six-outfit catalog validates.
Metal, WebGPU, and GL validation cover color, occlusion, effects, and resource
limits. Native review includes 390×844, 768×1024, 1280×800, and 1440×900, using
the existing portrait world letterboxing, plus the shared walk/run/jump suite.
See [review metrics](review-metrics.json) for the measured catalog and captures.

This is the first engine asset against the concept. The concept's finer hair
sculpting, integrated cloth transitions, richer surface shading, and nuanced
facial acting remain beyond this version. Sleeves are still rigid articulated
pieces, not deforming cloth; hands remain mittens. Review those gaps before
expanding the fit to more species. Desktop rendering checks do not establish
sustained performance or interaction quality on a physical phone.
