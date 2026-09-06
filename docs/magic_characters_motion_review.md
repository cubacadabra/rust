# Character review: expression and motion

This follow-up reviews the species proportions and graphic faces introduced
after the original [polish pass](magic_characters_polish.md). It fixes visual
regressions and adds a repeatable motion review using gameplay presentation.

The review found:

- Sparks followed the movement spring indefinitely and started inside shoes.
  They now emerge beside the shoulders for 0.36 seconds on takeoff, landing,
  and wave. They expire after a pause and are omitted in reduced-effects mode.
- Initial samples and teleports could replay landing/emote events. They now
  reset transient effects. Unknown ground support no longer implies flight.
- NPC/remote animation seeding overflowed in debug builds. Seed arithmetic
  now wraps explicitly, matching release behavior.
- The smile combined a line with an ellipse, leaving sharp disconnected
  corners. The new continuous mouth stays inside its quad across the supported
  opening/curvature range. Expression blending uses exponential damping.
- The old wave pointed mostly forward. It now raises the arm outward, moves
  the wrist, and eases into and out of the gesture.
- Enlarged animal torsos buried pockets, trim, and armor rivets. Front details
  now account for garment depth and taper. Rain hoods fit the species' head
  dimensions, and noses sit in front of the muzzle.
- The silhouette capture still contained colored details. Phase 2 now uses
  the production instanced renderer with faces/effects removed and uniformly
  black opaque pieces. Other Phase 2 views use the production face shader.

## Motion review

```sh
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 4 --width 1280 --height 720 --output /tmp/magic-character-review/motion
ffmpeg -framerate 30 -i /tmp/magic-character-review/motion/motion-%04d.png \
  -c:v libx264 -crf 18 -pix_fmt yuv420p -movflags +faststart \
  /tmp/magic-character-review/motion-review.mp4
```

Phase 4 saves 301 PNG frames at 30 fps and `phase4_report.json`. Each frame
replays deterministic inputs at 60 Hz through `CharacterPresentationState`,
then renders the resulting pose, face, and secondary motion. Frames can be
reproduced independently. `--pose-time` is replaced by the timeline.

| Time | Action |
| --- | --- |
| 0–1 s | Idle |
| 1–3 s | Walk |
| 3–5 s | Run |
| 5–6 s | Jump |
| 6 s | Landing and settle |
| 7 s | Wave |
| 8.5–10 s | Look around |

The person wears a hoodie, the cat a puffer, and the dragon armor. Travel is
staged in place for a fixed camera; the animator receives continuous world
distance, velocity, support, and event samples. This checks presentation,
not physics, collisions, network timing, or physical-device input feel.

## Verification

- 109 Rust library tests pass with `dev-showcase`, including event expiry,
  pause expiry, first samples, horizontal/vertical teleports, reduced effects,
  unknown support, and deterministic motion capture coverage.
- The six-outfit asset catalog validates. Native Metal captures include the
  motion timeline, ten wardrobe/orbit views, and four shape views.
- Metal, browser WebGPU, and browser GL validation pass on Apple M4 Max.
  Surface color, legacy color, opaque occlusion, and effect-depth errors are
  zero. These desktop checks do not establish mobile performance.
- The catalog still uses 319 meshes and 3,853,360 resident bytes (3.67 MiB),
  within the existing 384-mesh and 32 MiB limits. The timeline draws 122
  instances normally and 128 during bursts; six transient spark instances
  disappear when their lifetime ends. No new dependencies were added.

## Next visual priorities

The figures still need better grounding. Soft contact shadows, convincing
foot placement through the stride, and lighting that separates overlapping
pieces should precede more small outfit details. The motion review also makes
species-specific walk timing and landing follow-through easier to judge.
These remain follow-up work; the current pass does not establish the final
Cubacadabra art style or validate its appeal with children.
