# Magic Characters Phase 6: fidelity and sustained-performance foundation

Implemented September 5, 2026.

Phase 6 adds the bounded quality policy used by the production character draw
path. It keeps simulation, collision, the 18-character engine capacity and the
public snapshot unchanged.

## Runtime changes

- Characters select immutable near/mid/far mesh tiers from projected height:
  180 px and 70 px boundaries, 12 px hysteresis, and 4/2/1 curved-region
  subdivisions. Far characters retain the body, silhouette appendages, eyes
  and mouth while omitting brows and seam cores.
- Rest-pose part bounds are expanded for secondary motion and used as a
  conservative camera-space sphere. Culling is presentation-only and uses the
  actual world viewport aspect ratio and the existing 0.05–240 unit camera
  range.
- Batches continue to be shared by immutable mesh/material keys. The catalog
  remains bounded at 256 meshes and 32 MiB of character mesh/instance
  residency; normal frames upload only the selected instances.
- Effects are admitted deterministically in draw order, with at most eight
  seam effects per character and 128 live effects total. Reduced effects and
  far LOD omit the decorative seam cores without removing the character
  silhouette or expression.
- Support shadows now use a wide low-alpha penumbra plus a smaller contact
  core. They remain receiver-aligned, depth-tested, and depth-write-free;
  raised-block edge clipping and uncertain remote support retain the existing
  conservative behavior.
- Cloth/denim receive a restrained procedural weave cue, rainwear receives a
  broad highlight, and soft metal receives a bounded stylized environment
  response. These use the existing material parameters and add no texture
  bindings or optional GPU features.
- The development GPU validation fixture cycles supported body/outfit
  combinations in its 18- and 50-character scenes, so batching and residency
  checks exercise the Phase 5 catalog rather than a hoodie-only crowd.

## Policy report and verification

Generate the deterministic policy artifact with:

```sh
cargo run --features dev-showcase --bin magic_characters_capture -- --phase 6
```

The output is [`baselines/magic-characters/phase6/phase6_report.json`](baselines/magic-characters/phase6/phase6_report.json).
The policy report is intentionally separate from device timings; Phase 3's
production validation harness remains the source of native/browser GPU and
upload measurements. Run the relevant host/browser validation after building
the web fixture. A post-Phase-6 native Metal run on the available Apple M4 Max
completed the mixed-outfit 18/50 scenes with 212 mesh variants, 2.75 MiB
character residency, 47 draws, 74,244 triangles and 74,880 uploaded bytes for
the 18-character scene; the 50-character scene used 47 draws, 205,588
triangles and 207,488 uploaded bytes. These numbers are desktop evidence, not
mobile ratification.

Automated coverage includes threshold hysteresis, invalid-bound rejection,
effect admission limits, rounded mesh LOD recipes, existing geometry/rig/
appearance checks, and the full native/dev-showcase test suites.

The bounded directional-shadow, AO, HDR bloom, reflection probes, and texture
delivery alternatives remain explicitly deferred until a named baseline device
ratifies their sustained cost. Physical Android/iOS sustained sessions are not
available from this checkout, so those host captures remain an integration
gate rather than being represented as completed evidence here.
