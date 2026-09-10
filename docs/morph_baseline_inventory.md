# Morph Phase 0 baseline inventory

Status: inventory captured September 10, 2026. Rendered baseline images and
device timings are still pending the existing `dev-showcase` validation-fixture
build failure documented in `morph_plan.md`.

## Terminology and compatibility map

| V1 concept | Current IDs / source | V2 morph representation |
| --- | --- | --- |
| `BodyId::Person` | `cuba:person.v1` | `cuba:base/person.v1` plus `cuba:hair/swept.v1` for the default person preset |
| `BodyId::PersonGirl` | `cuba:person-girl.v1` | `cuba:base/person.v1` + `cuba:hair/side-ponytail.v1` |
| `BodyId::PersonNonbinary` | `cuba:person-nb.v1` | `cuba:base/person.v1` + `cuba:hair/shag.v1` |
| `BodyId::Cat` | `cuba:cat.v1` | `cuba:base/cat.v1` |
| `BodyId::Wolf` | `cuba:wolf.v1` | `cuba:base/wolf.v1` |
| `BodyId::Dragon` | `cuba:dragon.v1` | `cuba:base/dragon.v1` |
| `OutfitId` | six Rust enum values and `assets/characters/catalog.json` | independent catalog parts/presets in `assets/characters/morph_catalog.json` |
| face preset | V1 face string, default `happy` | `cuba:face/<name>.v1` in `MorphLoadout.face` |
| equipment | V1 slot-to-asset map, max 32 | independent `MorphLoadout.parts` IDs; slot and conflict validation is catalog-driven |

The migration implementation is in
`crates/morphs/src/legacy.rs`. Unknown V1 bodies remain errors; they do not
silently fall back to a different morph.

## Current authored/runtime inventory

- Bases in the compatibility catalog: person, cat, wolf, and dragon.
- Analytic face presets in the compatibility catalog: all 21 current V1 face
  names, represented as `cuba:face/<name>.v1` assets requiring
  `face.analytic.v1`.
- Person compatibility hair assets: swept, side ponytail, and shag. The
  renderer still has the original procedural hair JSON sources under
  `assets/characters/person/hair/`.
- Procedural outfit examples: everyday hoodie, puffer explorer, glossy
  raincoat, star wizard, toy knight, and fuzzy pajamas.
- Existing renderer equipment slots: hat, glasses, ear accessory, neck, back,
  waist, left hand, right hand, tail, and wings.
- Existing web profile images are three generated/legacy thumbnails:
  `../web/public/images/player_boy_001.png`,
  `player_girl_001.png`, and `player_nb_001.png`. They are presentation
  artifacts, not morph source data.
- The first external authoring probe is
  `~/Downloads/test_top_hat.glb`: one `Cylinder` node/mesh, one material,
  and 248 triangles. It currently lacks the required distinct LOD nodes.

## Capture matrix to freeze before renderer migration

The existing `capture_phase0_baseline` fixture produces 15 deterministic
scenarios at 640×360 plus 390×844 portrait variants where applicable:

- local and remote idle, walk, sprint, and jump in third-person;
- local and remote idle in first-person;
- raised platform;
- 18-character landscape crowd;
- 50-character render-only landscape crowd;
- 18-character portrait crowd;
- 50-character render-only portrait crowd.

Capture records must retain actor count, vertices, triangles, draw/instance
counts, mesh uploads, resident bytes, estimated upload/resource bytes, and CPU
and GPU timing fields. The capture code already writes these to
`phase0_report.json`; the missing artifact is a successful run on the current
renderer build.

## Phase 0 exit checks

- [x] Every current person/creature identity has an explicit V2 base mapping.
- [x] The three person presentation variants are independent hair selections,
  not new body classes, in the compatibility adapter.
- [x] Existing outfit and equipment concepts have catalog/loadout homes.
- [x] Capture scenarios and required measurements are enumerated.
- [ ] Produce and review the PNG/JSON baseline artifacts on a working
  `dev-showcase` build.
