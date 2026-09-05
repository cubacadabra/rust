# RFC: Magic Characters — Cubacadabra's enchanted toy people

Status: proposed implementation plan. This document does not implement or approve a renderer rollout.

Baseline: Rust checkout `6eb7cb4`, inspected September 5, 2026. Paths below are relative to this document. New APIs, modules, budgets, and asset identifiers are proposals unless explicitly described as current behavior.

## 1. Outcome and scope

Establish a recognizable Cubacadabra character family: soft geometric construction, tactile materials, playful proportions, graphic faces, and small magical separations between rigid body pieces. Internal art-direction name: **Soft Cubism**. Characters should resemble beautifully made enchanted toys and retain their cube ancestry.

The first release candidate is a playable showcase containing a kid-like person, a cat, and a little dragon, with six substantially different outfit silhouettes. All three share one character system and an excellent idle/walk/run/jump vocabulary. They must look related in silhouette, ordinary gameplay, close inspection, and motion on both native and browser renderers.

This is a character program with supporting renderer work, not a wholesale engine replacement. Keep simulation authoritative over movement and collision. Preserve existing world packages, client integrations, shared UI, and platform support throughout migration.

### Release boundaries

| Delivery | Required result | Deliberately later |
| --- | --- | --- |
| Shape proof | Rounded primitive, three bare bodies, hands/feet, graphic faces, visible gaps, simple seam light; fixed comparison scene | Detailed wardrobe, account persistence, advanced lighting |
| Playable vertical slice | Three bodies, six outfits, shared animation, material detail, grounded shadows, antialiasing, portable performance | Marketplace, arbitrary creator imports, full cloth simulation |
| Production identity | Per-player appearance, stable identity, persistence integration, compatibility and rollout completed | Unlimited species/anatomies and catalog scale |
| Fidelity expansion | Better shadows/AO, reflections, richer materials and selective world assets using the same style | Photorealistic humans |

Do not call the program complete after replacing the head mesh. Conversely, do not make reflection probes or a marketplace prerequisites for deciding whether the characters are appealing.

## 2. What the repository actually does today

| Area | Verified behavior | Consequence for this work |
| --- | --- | --- |
| [Avatar geometry](../src/renderer/geometry.rs) | `add_avatar` emits ten hard cuboids: torso, head, two arms, two legs, two shoes, two eyes. Head is `Vec3::splat(0.84)`; torso is `(1.1, 1.25, 0.64)`. Limbs rotate about their own centers. | Introduce an authored rig and pivots; increasing primitive smoothness alone preserves the old silhouette. |
| Animation | Pitch is `sin(walk_cycle) * 0.5`, except `assembled > 0.5` forces `0.03`. Bobbing uses world height near zero as its ground check. Shoes do not follow leg rotations. | Replace these with typed motion input, real parent transforms, and support-aware animation. |
| [Package appearance](../src/game_package.rs) and [style resolution](../src/renderer/scene.rs) | `AvatarDefinition` contains optional `skin`, `shirt`, `pants`, and `shoes` color strings. There is one player style and a list of NPC styles. | Existing authored palettes need deterministic migration and defaults. No existing modular wardrobe schema can simply be switched on. |
| [Draw path](../src/renderer/draw.rs) | Local and remote players use the same player style. `Scene.agents` and NPC styles are populated but the current dynamic draw path does not draw those agents. | Add an explicit showcase path; merely authoring three NPC styles will not display three characters. |
| [Entity snapshot](../src/engine/snapshot.rs) | Eight `f32` values per entity, with different meanings by entity kind. | Do not reinterpret the wire layout as a general character model. |
| [Engine limits](../src/engine.rs) | Total capacity is 18 including the local player. Local NPC simulation is disabled pending authority integration. | Baseline crowd testing is 18. A 50-character render stress scene must not silently change simulation/network capacity or enable production NPCs. |
| [Simulation state](../src/types.rs) | Local player has velocity, grounded, moving, sprinting. Remotes have position, yaw, movement flags and walk cycle, but no stable identity, vertical velocity, or grounded flag. NPCs have vertical motion and grounded state. | Local animation can improve internally first; reliable remote animation and appearance need additive inputs. |
| [Geometry upload](../src/renderer/draw.rs) | Static world vertices are cached. Dynamic geometry is rebuilt into a fresh CPU vector and uploaded each draw. Rendering is non-indexed. | Rounded pieces and clothing multiply CPU work and bandwidth unless meshes are cached and transforms instanced. |
| [Vertex/layout](../src/renderer/types.rs) | `Vertex` is shared by world and UI, with position, normal, color, texture coordinates and UI inversion data. | Use a dedicated character layout instead of growing the shared UI/world vertex indiscriminately. |
| [Shader](../src/renderer.wgsl) | World shading is diffuse directional light, a small rim term, and distance fog. World texture coordinates are unused; the textured path is for UI. | Clothing textures, roughness and emission require real character material support. |
| [Pipelines](../src/renderer/device.rs) | World uses alpha blending, depth writes, no culling, and single-sample rendering. Avatar shadow is a thin cylinder at world Y `0.018`. | Separate opaque, face, shadow and effect behavior; translucent glows cannot safely inherit the world pipeline. |
| Platform/color | Native uses Metal except Android's explicit GLES path; browser requests WebGPU or GL. No optional GPU features are requested. Browser prefers sRGB surfaces, native prefers non-sRGB; UI atlas intentionally uses unorm. | Preserve the portable path, probe capabilities, and explicitly resolve color management before judging material matching. |
| Camera/collision | `BODY_HEIGHT = 3.15`, `PLAYER_RADIUS = 0.52`; current head top is Y `3.43`. First-person eye is hardcoded to `3.4`, third-person target to `1.78`; avatar is hidden at camera distance `<= 0.75`. | Body changes require camera/framing review, not automatic collider resizing. Existing visual and collision envelopes already differ. |

### Snapshot hazard to resolve early

The current snapshot suffix has these meanings:

| Entity | Slot 5 | Slot 6 | Slot 7 |
| --- | --- | --- | --- |
| Local player | grounded | moving | sprinting |
| Local NPC | phase code | meeting index | assembled at meeting |
| Remote player | constant `1.0` | constant `-1.0` | constant `0.0` |

`scene::render_entity` reads slot 7 as `assembled` for every entity. Consequently local sprinting selects the nearly fixed limb pitch. The NPC term “assembled” means gathering at a launch pad; it is unrelated to magical body construction. The new model must name these concepts separately, and a regression test must cover sprint versus NPC gathering.

## 3. Art direction contract

### Shape and proportions

Author one dimensioned proportion sheet before clothing. Review front, side, back, three-quarter and solid-black views. The following values are starting hypotheses, to be tuned during the shape proof; they are not measurements in meters.

Use a default sole-to-head height `H` around 3.1–3.3 engine units, excluding ears, hats and horns. Preserve the established feet origin and gameplay scale while redistributing visual mass.

| Element | Initial target |
| --- | --- |
| Head | Roughly 30–35% of `H` tall; slightly wider than tall and shallower than wide; broad planar face with rounded edges; optional modest lower taper |
| Torso | Roughly 28–32% of `H` tall; shorter and visibly tapered; no wide rectangular shoulder bar |
| Shoulders/arms | Dropped shoulders, separate upper/lower chunky segments with intentional joint pivots; small taper rather than long sticks |
| Hands | Separate rounded mitten forms, visibly larger than wrists; no individual finger rig required |
| Legs | Short, chunky upper/lower segments; stance and foot placement readable at gameplay distance |
| Feet | Broad toe, thicker sole, distinct heel; visibly oversized sneakers/boots with forward projection along local `-Z` |
| Edge softness | Head/body fillet radius initially 12–20% of the shortest full dimension, preserving wide flat regions; smaller details follow the same family |
| Magical gaps | Rest separation roughly 0.5–1.5% of `H`; motion can expand to about 3% at selected joints, with clamped travel |

The user's 1–3 cm description specifies perceived subtlety, not a new world-unit conversion. Measure actual surface-to-surface gaps, including sleeves/cuffs, rather than just separating joint origins. Neck, shoulders, wrists, waist, knees and ankles need authored clearances. Avoid making the body look damaged or dismembered; a quiet energy core communicates cohesion.

### Species

All initial species are upright toy characters using the common locomotion rig. This is not an attempt to support arbitrary quadrupeds or winged flight in the first implementation.

| Body | Species-specific construction | Shared construction |
| --- | --- | --- |
| Kid-like person | Soft cube head; optional small nose and simple later hair pieces | Face language, torso/limb proportions, mitten hands, exaggerated shoes, magical joints |
| Cat | Rounded triangular ears, short squircle muzzle, graphic nose, articulated chunky tail | Same outfit anchors and locomotion semantics; species-fit head/face coordinates |
| Little dragon | Rounded muzzle, small horns, chunky tail, compact wings and optional back plates | Same toy scale and rig, with bounded secondary appendage motion; wings are cosmetic |

A fox, dog, rabbit, bear, unicorn or robot should later be a body definition plus optional appendages/materials, not a copy of the renderer. Truly different anatomy requires a new rig family and its own fit/animation validation.

### Faces and visual identity

Use slightly vertical rounded eyes, one restrained highlight, clear eyebrows and a graphic mouth. Avoid anatomical eyeballs, realistic lips, detailed skin and human facial proportions. Skin/fur colors must not determine emotion readability.

Start with neutral, happy, surprised, determined, sad and laughing poses plus blinking and look direction. Build toward approximately twenty authored poses: neutral, smile, grin, laugh, curious, surprised, amazed, determined, angry, sad, crying, worried, embarrassed, sleepy, squinting, wink, smirk, confused, excited and unimpressed. These are combinations of a small face parameter set, not twenty unrelated head meshes.

Face recognition must survive the normal camera distance, muted lighting, dark/light skin and fur, and a non-emissive silhouette review. Lower levels of detail simplify features while retaining the eyes and a readable mouth where their projected size allows it.

### One imaginary toy company

Official assets share edge softness, limited material families, exaggerated useful shapes and small magical details. After character approval, prove transfer with a squircle-foliage tree, a rounded toy prop/vehicle panel and one creature accessory. Do not round every collision block or rewrite the world during the avatar migration. Creator experiences may choose a different style; official starter assets provide the coherent reference.

## 4. System boundaries and proposed code organization

Keep appearance definitions, motion samples and CPU pose evaluation independent of `wgpu`. GPU resources remain renderer-owned. The same renderer serves native and browser clients; no JavaScript or Swift reimplementation of the rig.

```text
package defaults + host-supplied appearance
                    |
             resolve and validate
                    v
Engine identity ----> CharacterAppearance (stable asset IDs and tints)
Engine motion ------> CharacterMotionSample (typed, per entity)
                    |
          presentation state + pose evaluation
                    v
      CharacterPose + face + attachment transforms
                    |
          renderer mesh/material caches
                    v
       instance batches -> character/effect passes
```

Introduce files when their phase needs them; this is a responsibility map, not a request to scaffold empty modules.

| Proposed location | Responsibility |
| --- | --- |
| `src/character.rs`, `src/character/definition.rs` | Pure definitions, appearance resolution, asset IDs, schema version/defaults and fit validation |
| `src/character/rig.rs`, `src/character/animation.rs` | Parent hierarchy, anchors, motion samples, bounded pose evaluation and presentation state |
| `src/character/face.rs` | Expression parameters, authored presets and deterministic blink/look timing |
| `src/renderer/character.rs` | Translate resolved characters/poses into batches; character cache ownership |
| `src/renderer/rounded_geometry.rs` | Indexed rounded/tapered primitive generation and geometry tests |
| `src/renderer/character_material.rs`, `src/renderer/character.wgsl` | Dedicated vertex/instance/material layouts and portable character shading |
| `src/renderer/character_effects.rs` | Bounded seam effects and support shadows when this outgrows the main character module |
| `assets/characters/` | Versioned official body/outfit recipes, face/material textures, source/license metadata |
| `tests/fixtures/characters/` and a development capture example/tool | Deterministic showcase recipes, motion timelines and visual/performance captures |

The renderer currently uses `include!` for geometry and types. New character code should use ordinary modules with narrow visibility; avoid an unrelated conversion of all existing renderer code.

### Identity, motion and presentation

Separate three categories explicitly:

- `CharacterAppearance`: schema version, body ID, face style, equipped asset IDs, color channels, appearance revision. Changes rarely; contains no frame-to-frame transforms.
- `CharacterMotionSample`: stable entity key, motion sequence/time, position, facing/look yaw, planar velocity, vertical velocity, grounded/support information, movement intent and locomotion/emote events. Fields whose source lacks data carry an explicit unknown/estimated status.
- `CharacterPresentationState`: previous sample, stride phase, blend state, joint springs, blink schedule, expression and effect lifetimes. Cosmetic only; cannot move the collider or mutate gameplay RNG.

Prefer a Rust-only typed engine accessor for renderer synchronization first. Both C-driven rendering and `WebRenderer::sync_engine` already receive an engine reference internally, so better local state does not require replacing the public float snapshot. Retain the old snapshot writer and its exported length/stride/semantics for existing consumers.

Use an engine-owned presentation tick or sequence to advance cosmetic state once per engine update. Repeated `sync_engine`/`draw` calls at the same tick must not advance animation. Renderer recreation reconstructs a stable pose from current state; it must not replay a landing or emote. If smooth render interpolation is needed later, introduce explicit previous/current samples and interpolation time rather than reading wall time inside geometry generation.

Stable entity keys must include a session/spawn generation. Vector indices alone are unsafe when remote slots are reused. Until clients supply stable keys, legacy remote slots get explicit replacement/reset semantics and a conservative fallback; they cannot promise identity continuity after reorder. Clear presentation/effect state on despawn, world transfer, teleport, generation change and large reconciliation correction. Preserve appearance across world transitions separately from motion state.

## 5. Geometry and rendering foundation

### Rounded cuboid implementation

Implement a true filleted box with planar faces, edge strips and rounded corner patches. Generate vertices/normals in final local dimensions: a single rounded unit cube stretched nonuniformly produces inconsistent corner radii and is insufficient for the official shape language.

The builder accepts full dimensions, radius, bevel subdivisions and an optional limited taper profile. Suggested LOD subdivision tiers are 4/2/1 across curved regions; choose exact tessellation from silhouette measurements. Clamp radius below half the shortest dimension; reject non-finite/nonpositive dimensions and bound subdivision counts before allocation. Radius zero must produce a valid hard box, not degenerate bevel patches.

Use indexed triangle lists. Define consistent winding and outward normals so back-face culling can be enabled for character solids. Duplicated vertices are acceptable for UV seams; positions and normals must agree across smooth geometric seams. If taper modifies positions after construction, recompute appropriate normals. For transformed normals use inverse-transpose or an equivalent correct representation; the existing `transform_vector3` shortcut is insufficient for new nonuniformly scaled curved geometry.

Keep shape generation pure and cache by bounded, canonical recipe keys: dimensions/proportion preset, radius, taper and LOD. Do not cache arbitrary continuously changing float combinations. Animate transforms, not mesh recipe dimensions, for routine squash/stretch.

### Prototype-to-production transition

The first shape proof may expand cached meshes into the existing CPU vertex path for a few characters. Label that path temporary and record its cost. Before detailed outfits or production crowds, upload each immutable character mesh once and submit indexed instanced draws containing transform, normal transform, tint/material selection and any required face/effect parameters.

Batch by mesh, material, LOD and pipeline. Use instance vertex attributes within the requested device's actual limits; budget attribute locations and vertex-buffer stride before settling the GPU layout. Do not require storage-buffer indexing, bindless textures, compute skinning, indirect draws or optional GPU features for the baseline. If a layout exceeds downlevel limits, simplify/pack it or split batches, not platform support.

Keep the existing static world path initially. Reuse CPU instance vectors and GPU buffer capacities; avoid regenerating geometry, decoding images, compiling pipelines or allocating GPU resources during normal drawing. Build an appearance's GPU resources before swapping it into the scene; use a deterministic fallback while an asset is unavailable.

### Passes, faces, emission and grounding

1. Opaque world and character solids: depth test/write; character solids use correct culling. Split existing translucent world items as necessary before inserting effects so their ordering is explicit.
2. Receiver-aligned support shadow: separate blended, depth-tested geometry without depth writes; rendered over the receiver and correctly occluded by nearer objects.
3. Graphic face surfaces: initially shallow rounded eye/brow meshes and a small mouth ribbon on the planar face region; later an authored texture atlas or analytic masks if it materially improves expression quality. Attach them to head-local anchors with controlled surface offsets. Avoid camera-facing HUD faces and z-fighting.
4. Seam cores and sparse sparks: dedicated unlit/emissive material, depth tested. Additive sparks have depth writes disabled; any conventional alpha particles are sorted. Never make every seam a dynamic light.
5. Optional quality postprocessing resolves before the existing UI overlay. UI stays crisp and does not bloom.

Emission can appear bright without bloom. Baseline seam glow must work on the ordinary color target; bloom is a later capability/budget decision. Render a tiny core at selected visible seams plus short-lived jump/landing sparks. Start with no more than eight live particles per character and 128 total, with deterministic priority for the local/nearby characters. Cull effects by projected size and stop spawning offscreen; no high-frequency flashing.

Replace the current floor-fixed shadow with support-aware placement. Local support comes from collision queries against ground/obstacle tops; expose support height without altering collision behavior. Query static receivers conservatively for legacy remotes, fading when support is uncertain. A shadow on a raised block must stay on that receiver, shrink/fade with height, and avoid visibly spilling down an edge. Real shadow maps later supersede this where enabled.

## 6. Rig and animation

### Shared rigid-piece rig

Start with root/pelvis, torso, head, upper/lower arms, hands, upper/lower legs and feet, with optional ear/tail/wing/horn anchors. Approximately 16–24 transforms should cover the initial bodies; count and bound the actual shipped definitions. Use stable named joints in authored data and compact indices after validation. Validate parent order, cycles, missing anchors and maximum hierarchy depth.

Each body defines rest transforms, joint limits, gap axes/clearances, face placement, camera anchors, contact/foot anchors and attachment fit regions. Store costume attachment offsets relative to these anchors. Hands follow forearms, shoes follow feet, faces follow heads; rotations happen at joints rather than part centers.

The base rig is rigid mesh assembly, so it needs neither vertex skinning nor physics joints. Cloaks, skirts and tails can initially use a few articulated panels/segments with bounded spring motion. Reserve a separate skinning/deformation proposal for cases these methods cannot meet; do not add a general animation engine or cloth solver speculatively.

### Motion behavior

| State/transition | Implementation intent | Review criterion |
| --- | --- | --- |
| Idle | Small breathing offsets, occasional head/eye look, sparse blinks; settle limbs to rest | Stopping at any stride phase does not leave arms/legs frozen mid-swing |
| Walk | Phase from actual planar distance and authored stride length; relaxed arms, gentle bounce, slight head lag | Reasonable foot contact, no treadmill motion against a wall |
| Run | Blend by speed/intent; forward torso lean, stronger recoil, kicking feet, slightly wider gaps | Clearly distinct from walk; sprint never selects NPC meeting behavior |
| Takeoff | Brief cosmetic compression and extension triggered by takeoff event | Jump input applies the existing physics impulse immediately; anticipation must not add input latency |
| Rising/falling | Vertical-speed-driven pose and leg dangle, bounded head/limb lag | Falling off a ledge works without a jump event; no ground-height heuristic |
| Landing | Contact transition and pre-contact vertical speed drive a short compression/recovery | Landing on blocks works; duplicate samples and teleport corrections do not replay impacts |
| Turn | Look/head lead, torso follows, feet settle last; shortest-arc yaw blending | Correct facing in every cardinal direction, backward motion and strafing; no full spin across the yaw wrap |
| Wave/emote | Authored enthusiastic arm/head motion blended over locomotion with priority/interrupt rules | Wave remains readable while idle or moving and releases to the correct current pose |

Keep root motion in simulation. Cosmetic translation and rotation stay within an authored envelope. Use stable damped springs or analytic easing with delta clamping/substeps; verify behavior at 30/60/120 Hz and after resume. `Engine::step` currently clamps delta to 0.05; choose a documented presentation timebase rather than silently changing simulation timing.

Do not derive jump from `position.y > 0`, because raised surfaces exist. Use explicit grounded transitions locally. For remotes, prioritize supplied velocity/ground state and event sequence; legacy fallback estimates motion from timestamped position changes and must suppress false impacts after correction/slot replacement. Perfect remote jump timing remains an integration dependency until richer data arrives.

Separate movement heading, look heading and camera orbit in presentation. The current local snapshot yaw is camera yaw, and NPC yaw is derived from the target vector; validate the sign convention against the mesh's `-Z` forward before reusing either. Head-first turns must not change input movement direction or reconciliation semantics.

### Expression and secondary motion

Blend expression parameters for eye opening, look offset, brow tilt/height, mouth curvature/opening and optional tears/highlight. Blink is a short overlay with lower precedence than intentional closed-eye expressions. Use an entity-seeded cosmetic RNG isolated from gameplay randomness, so a crowd does not blink in unison and captures are reproducible.

Keep head lag, ears, tail, wing panels and gap springs subordinate to the main pose. A reduced-motion/effects preference disables decorative spark bursts and reduces spring amplitudes while retaining readable locomotion and expression. Browser/native hosts must eventually provide their preference through an explicit engine/renderer setting; there is no existing preference bridge to assume.

## 7. Appearance, wardrobe and content pipeline

### Versioned additive schema

Retain the current `avatars.player` and `avatars.npcs` outer structure. Add an optional versioned `character` member to each `AvatarDefinition`; keep existing four colors as the legacy fallback. Avoid changing `shirt` from a color string to an object or overloading it as an asset ID.

Illustrative package fragment, to become a tested fixture during schema implementation:

```json
{
  "avatars": {
    "player": {
      "skin": "#e8ae86",
      "shirt": "#2d6663",
      "pants": "#536a90",
      "shoes": "#293a43",
      "character": {
        "version": 1,
        "body": "cuba:person.v1",
        "face": "cuba:friendly.v1",
        "outfit": "cuba:everyday-hoodie.v1",
        "equipment": {
          "hat": "cuba:star-cap.v1"
        },
        "colors": {
          "skin": "#e8ae86",
          "primary": "#2d6663",
          "secondary": "#536a90",
          "sole": "#f6f1e7"
        }
      }
    }
  }
}
```

Resolve precedence explicitly: runtime player appearance overrides package defaults; a selected outfit expands into slot defaults; explicit equipment overrides those slots; explicit named color channels override recipe defaults. Legacy colors map to default body/hoodie/pants/shoes channels when no supported character definition exists. Invalid individual items fall back by slot. An unsupported version falls back to the four legacy colors and default body, without discarding the rest of a valid world package. Missing fields preserve old behavior; invalid present data produces bounded diagnostics.

Define IDs as stable, namespaced strings on the content/host boundary and resolve them to compact handles internally. Version recipe geometry independently of a player's appearance revision. Do not put strings, GPU handles or pointers into the float snapshot. Default assets must be bundled/available offline.

Initial validation limits should be explicit and tested: for example 4 KiB per runtime appearance, 96-byte ASCII asset IDs, at most 32 equipment entries and 16 tint channels, bounded rig/mesh/texture sizes, finite transforms, known slots, compatible body fits, and no cycles. These are initial policy values to ratify in the schema phase. Do not apply new tiny limits to an entire existing world manifest.

### Slots, fit and layers

The eventual slot vocabulary should cover body/skin, face/eyes/eyebrows/mouth, hair, hat, glasses, ear accessories, shirt, jacket, dress, pants, skirt, left/right hand items, shoes, back, waist, neck, tail, ears, horns and wings. Some are body/face configuration channels rather than independently stackable garments; model that distinction rather than placing everything in one draw-order list.

Start with the slots needed by the six outfits. Each wearable declares compatible rig families/body IDs, occupied slots, joint anchors, fit variant, body-region coverage mask, material references, LOD recipes, bounds and conflicts. An outfit is a reusable bundle of these pieces, not a new character class.

Resolve base body -> underlayer -> main garment -> outerwear -> accessories using explicit occupancy and coverage rules. Hide covered body/sleeve regions so clothing does not flicker through them. Never render two pieces in the same space solely because both slots are equipped. Whole-body dress conflicts with incompatible pants/outerwear; full boots replace shoe geometry; a hood/hat resolves hair and ear/horn conflicts using authored cutouts or fit variants. Tails and wings require garment openings or a declared incompatibility.

No universal automatic cloth fitting is promised. Author person/cat/dragon fit variants where anchor offsets cannot work. At minimum the hoodie and raincoat must fit all three bodies to demonstrate reuse; the remaining hero outfits have explicitly declared support. Test every supported combination, and reject/fallback unsupported combinations predictably.

### Six outfit acceptance set

| Outfit | Required silhouette change | Tactile detail | Hero pairing |
| --- | --- | --- | --- |
| Oversized everyday hoodie + denim + sneakers | Dropped shoulders, long hem, large mitten cuffs and chunky toe/sole | Pocket, drawstrings, seams, cloth weave, denim variation, laces | Person |
| Puffer explorer + large boots | Wider segmented torso, thick collar and cuffs, broad boot volume | Quilting, zipper, rubber tread and matte/gloss contrast | Cat |
| Glossy raincoat | Flared coat hem and large hood/collar | Waterproof sheen, closures and raised seam accents | Cat |
| Star wizard cloak + ridiculous hat | Tall soft pointed hat and swinging cloak panels | Embroidered stars, hem trim and restrained magical accent | Dragon |
| Toy knight armor | Chunky shoulder/forearm plates and armored boots | Soft metal highlights, rivets and padded underlayer | Dragon |
| Fuzzy pajamas | Soft rounded cuffs, roomy legs and slippers | Low-cost fuzz impression and small stitched motifs | Person |

This is six outfits total, not six palette swaps or eighteen entirely bespoke costumes. Show each hero pairing, both common garments on all species, and the compatibility matrix for the rest. Later skirts/dresses extend silhouette coverage; their slot/conflict model is designed now but they need not displace these six.

### Materials and asset authoring

Begin with toy skin/plastic, cloth, denim, rubber, waterproof/gloss, soft metal, fuzz and emission presets. Keep diffuse plus restrained rim as the stylistic foundation. Add controlled roughness/specular response and authored texture detail; normal maps/tangents arrive only when close-up inspection justifies their cost. Tiny weave/stitches belong in filtered textures; seams, pockets, soles and folds that affect silhouette belong in geometry. Fuzz initially uses texture/lighting, not expensive shell fur.

Do not claim metallic reflections from a shinier diffuse color. The first armor can use a stylized environment/ramp approximation, documented as such; quality-tier environment maps and roughness-prefiltered reflections are later work. Cloth simulation, skin subsurface scattering and strand hair are not required.

Bundle small official procedural recipes first and reuse existing image decoding infrastructure where suitable. Character textures need their own atlas/material lifetime, UV padding and mip strategy; do not pack them into the UI atlas. Set UV scale consistently so weave does not stretch between bodies. Use sRGB decoding for authored color where the color pipeline calls for it, linear data for roughness/normal masks, and mipmaps to prevent shimmer.

Each asset includes source/license provenance, recipe version, dimensions, pivots, bounds, compatible rigs, coverage masks, materials, LODs and a preview fixture. Add an offline validator/capture command before growing the library. Asset loading must validate decoded dimensions before large allocations and retain a known-good appearance on failure. Hosts fetch any future external assets and pass validated bytes/catalog references into Rust; the engine does not gain an implicit network client.

An external mesh/import pipeline (for example a future glTF-based authoring path) needs a separate decision after procedural recipes expose a real limitation. No new runtime importer or material dependency is required merely to begin this RFC.

## 8. Lighting, antialiasing and performance

### Fidelity sequence

First establish rounded normals, expressive faces and coherent material values. Then resolve color-space differences, introduce capability-selected antialiasing, improve contact grounding, and only then add more lighting passes.

Audit authored colors -> shader math -> surface encoding across native unorm and browser sRGB. Choose an explicit character/world color contract and verify swatches/skin tones on each path. UI's existing unorm atlas and authored colors require separate regression captures; a global gamma change is not a safe shortcut. A compatibility conversion may be necessary while the world pipeline migrates.

Use 4x MSAA for the 3D pass when the selected color/depth formats and adapter support it within budget; otherwise a valid 1x path and a separately evaluated lightweight post-AA option. Recreate matching color/depth attachments on resize and surface reconfiguration, resolve before UI, and keep sample counts consistent across pipelines. No platform may fail startup because an optional quality feature is unavailable.

For the higher quality tier, start with one bounded directional shadow map, filtered samples and stable light-space framing; tune bias to avoid detached feet and acne on bevels. Measure cost before considering cascades. Simple authored/analytic contact occlusion can precede screen-space AO. Screen-space AO, HDR bloom and environment reflections need explicit targets, quality fallbacks and temporal shimmer review; they are separate deliverables, not additions to an already overloaded shader change.

### Provisional budgets

These are planning ceilings, not measured current performance or hardware guarantees. Phase 0 must record named devices, browser/OS/backend, render resolution and a repeatable scene before ratifying them.

| Quantity | Initial gate |
| --- | --- |
| Real gameplay crowd | 18 visible characters including local; preserve current engine capacity |
| Render-only stress | 50 characters via showcase fixture, including mixed species/outfits |
| Near / mid / far triangles | Aim <= 8k / 3k / 800 per fully dressed character, including appendages and face |
| Rigid instances | Aim <= 48 visible parts per dressed character; <= 64 rig transforms allowed by content validation |
| Character CPU work | Target <= 2 ms p95 for pose evaluation, batching and uploads at 18 characters on the selected baseline device |
| GPU cost | Target <= 4 ms p95 incremental character/effect cost at the selected baseline resolution; full-frame budget still governs |
| Presentation | 60 fps target on the chosen baseline; explicit lower quality/30 fps sustained mode where required, without input/simulation changes |
| Character GPU residency | Initial <= 32 MiB mesh/texture/cache budget for the six-outfit showcase; track render targets separately |
| Texture delivery | Initial <= 8 MiB compressed character payload; report decoded bytes, mip overhead and total WASM/download change |
| Submission | Aim <= 100 character draws at 18 characters after batching; treat unique costume/material proliferation as a measured cost |
| Dynamic upload | Target <= 0.5 MiB/frame for character transforms/parameters at 18; no per-frame immutable mesh upload |
| Effects | <= 8 live sparks per character, <= 128 total; disable distant sparks first |

Choose LOD from projected size in the actual world viewport, with hysteresis. Near characters keep facial/garment details; mid characters preserve silhouette and primary expression; far characters retain a readable body/head/feet and omit subpixel accents. Tune initial thresholds around 180 and 70 projected pixels in the showcase, not hardcoded world distance alone. Use conservative animated bounds for frustum culling, including tails, hats and maximum seam travel.

Reduce effect count, shadow quality and secondary-motion update rate before erasing the defining head/hands/feet silhouette. Cache shared textures and meshes by recipe, evict with bounded residency, and release device resources on renderer destruction. Device recreation may reload from retained recipes/bytes; repeated appearance switching and world changes must not grow caches without limit.

Measure warm steady-state and cold appearance swap separately. Capture median/p95 frame and CPU timings, GPU timings where supported, draw/triangle counts, upload bytes, allocations, cache residency, loading latency and sustained mobile behavior. Do not require timestamp-query support for correctness or baseline instrumentation. A smooth desktop screenshot is not evidence of mobile performance.

## 9. Clients, multiplayer and persistence

The [repository README](../README.md) assigns package fetching and multiplayer sockets to host clients. This RFC changes Rust contracts where needed and identifies companion work; it does not claim those repositories have been audited or updated.

| Owner | Required companion work | Rust boundary |
| --- | --- | --- |
| `first-game` | Author optional character definitions, showcase world/fixtures and official outfit references | Additive package schema with old-color fallback |
| `web` | Fetch selected appearance/assets, pass stable remote identity and motion metadata, propagate effects preference, rebuild WASM bindings | Existing renderer stays shared; additive engine APIs |
| `ios_app` | Match appearance/motion ABI, host preference bridge, asset delivery and device captures | Updated [C header](../include/cubacadabra_engine.h), matching static library |
| Android host, where maintained | Same ABI/content/preference integration; validate actual GLES path | Android support cannot be inferred from Vulkan feature flags alone |
| `backend` | Persist identity/appearance revision; validate allowed items; replicate appearance on change and locomotion metadata/events | Versioned messages adapted by hosts; no socket logic inside Rust |

Add new C/WASM entry points rather than silently extending the eight-float record. A concrete API proposal should include appearance buffer allocation/load with a status/error contract and revision, plus a versioned remote-state setter containing stable key/generation, timestamp/sequence and optional velocity/grounded/event data. Preserve old setters and document their reduced-fidelity fallback.

Specify buffer ownership, input length limits, UTF-8 validation, pointer validity and lifetime in the header. Follow the existing buffer-load pattern while bounding lengths before allocation. JSON appearance changes are infrequent; motion should use a compact typed ABI/batch rather than parsing appearance JSON per frame. Add one Rust adapter used by both ABI paths and test it independently of host rendering.

Network appearance should send stable asset IDs/tints and an appearance revision, not meshes, joint matrices or blink state. Apply revisions monotonically, deduplicate emote/landing event sequences, and reset state when session identity changes. Host interpolation and Rust presentation must have a single documented ownership boundary to avoid double smoothing. Sparse old-client data gets a neutral default body and conservative motion.

Persistent identity belongs to the account service/client storage integration. Rust keeps a resolved in-memory identity across worlds and can render offline defaults, but package-wide player colors are not account persistence. Experiences may provide defaults or explicitly allowed costume overrides; they should not silently overwrite saved identity. Backend entitlement/catalog policy is a companion product decision, not something the renderer can enforce by itself.

A full wardrobe picker/store is out of the renderer scope. First provide a compact development selector for species, outfit, expression, motion timeline and quality tier. A later product picker should reuse existing shared UI patterns, with preview/apply/cancel and concise loading/error feedback.

## 10. Delivery plan and review gates

Implement as small reviewable changes. Each phase ends with executable evidence and a visual or compatibility gate. Build the shape proof before expanding materials; complete the production draw path before filling the wardrobe. Estimates should be added after the first measured slice rather than assigning a speculative completion date now.

### Phase 0 — Baseline and repeatable review fixture

- [ ] Record current local/remote idle, walk, sprint, jump, first/third person and raised-platform captures, plus CPU/GPU/resource counters where available.
- [ ] Add a development-only deterministic character showcase/capture entry point: fixed camera/light, neutral floor, seed, pose time and selectable palette/quality. It must work without enabling local NPC simulation or connecting a multiplayer backend.
- [ ] Cover the current 18-character capacity and an isolated 50-character renderer stress scene. Record platform/device details and both landscape and current portrait letterboxed composition.
- [ ] Document the snapshot suffix hazard and preserve existing ABI fixtures.

Primary files: `renderer/draw.rs`, `renderer/scene.rs`, development fixtures/tool, `engine/tests.rs`.

Exit: reproducible before-images and measured baseline; device owners and performance targets recorded. A headless/offscreen renderer harness may be added for captures; surface creation currently expects platform handles, so this is real tooling work rather than an existing test command.

### Phase 1 — Typed motion boundary and rounded primitive

- [ ] Add the Rust-only typed character motion accessor and stop applying NPC gathering semantics to local sprinting. Preserve exported snapshots byte-for-byte in meaning.
- [ ] Implement and test indexed rounded/tapered geometry with valid normals, UVs, bounds, radius limits and LOD variants.
- [ ] Cache mesh recipes and temporarily adapt them to the existing draw path for a small comparison fixture.

Primary files: `engine.rs`, `types.rs`, new character types, `renderer/scene.rs`, new rounded geometry module; `engine/snapshot.rs` only where compatibility tests/documentation require it.

Exit: sphere-versus-cube/bevel comparisons accepted; correct lighting under rotation/nonuniform transforms; sprint animates rather than freezing. No ABI or physics behavior changes.

### Phase 2 — Shape proof: three related bodies

- [x] Author the proportion sheet, common rest rig and person/cat/dragon recipes.
- [x] Add mitten hands, articulated chunky legs/feet, expressive base faces and authored gap clearances.
- [x] Add minimal depth-tested seam cores; keep sparks simple until the dedicated effects pass lands.
- [x] Add body-defined camera anchors, first-person hiding and silhouette captures beside the legacy avatar.

Primary files: character definitions/rig/face, `renderer/character.rs`, `renderer/draw.rs`, `assets/characters/`.

Exit: front/side/back/black silhouettes and three-body lineup read as one toy family without outfits or special lighting. Bodies no longer rely on the rectangular torso/stick-limb silhouette. Review actual gameplay size and raised platforms, not only close-up renders.

### Phase 3 — Production character submission and base materials

- [x] Add dedicated character vertex/instance layouts, cached indexed buffers, batched draws and bounded resource lifetime.
- [x] Add character shader/material parameters, face rendering and correct opaque/translucent/effect depth behavior.
- [x] Establish the cross-platform color contract and protect UI appearance.
- [x] Add capability-selected MSAA and a working 1x fallback; use correct resize/resolve behavior.

Implementation and executable native/WebGPU/GL evidence: [Phase 3 submission report](magic_characters_phase3.md). The measured development baseline meets the submission/CPU budgets; platform-owner ratification and physical mobile-device performance remain explicit review gates.

Primary files: `renderer/types.rs`, `renderer/device.rs`, `renderer/draw.rs`, new character renderer/material/shader modules.

Exit: 18-character stress measurements meet ratified base budgets on native and browser; meshes are not regenerated/uploaded per frame; effects occlude correctly; UI and world colors have explicit regression evidence.

### Phase 4 — Excellent shared locomotion and expression

- [ ] Implement distance-driven walk/run blending, idle settling, head-first turns and responsive jump/fall/landing poses.
- [ ] Add bounded gap springs, tail/ear/wing motion, deterministic blinking/look behavior and the initial six expressions.
- [ ] Add wave and approximately twenty expression presets without duplicating body geometry.
- [ ] Reset state correctly on world transfer, despawn, teleport, pause/resume and reconciliation. Add support-aware shadows and reduced-effects settings plumbing within Rust.

Primary files: character animation/face/rig, typed motion accessor, `player.rs`/`npc.rs` only for exposing required state/events, character effects renderer.

Exit: shared 30/60/120 Hz motion replay passes; stops, collisions and raised-platform landings look intentional; all three species move as one family. Remote limitations are demonstrated and documented until Phase 7 supplies richer state.

### Phase 5 — Wardrobe schema and six finished outfits

- [ ] Add versioned optional `character` package data, legacy mapping, resolver and asset validation fixtures.
- [ ] Implement slots, coverage/conflicts, body-fit variants and atomic appearance swaps.
- [ ] Author all six silhouette-changing outfits and close-up tactile materials; hoodie and raincoat fit every initial species.
- [ ] Add catalog metadata/license records, LOD recipes, texture filtering/mips and offline validation/captures.

Primary files: `game_package.rs`, character definitions/resolver, material/cache modules, `assets/characters/`, fixtures/validation tooling.

Exit: every supported combination passes idle/motion/clipping review; unsupported combinations fall back clearly; old manifests still load. Six hero costumes are materially and geometrically distinct. This completes the playable vertical slice after the combined platform/performance gate.

### Phase 6 — Fidelity and sustained performance

- [x] Tune LOD thresholds, culling bounds, batching, cache limits and effect priorities using mixed outfits.
- [x] Improve contact/soft shadows; keep the bounded directional shadow tier conditional on a ratified device budget.
- [x] Evaluate richer cloth detail and soft metal environment response as bounded baseline changes; retain AO, directional shadows, HDR bloom and reflection probes as explicitly deferred alternatives.
- [ ] Run sustained mobile sessions, cold/warm outfit changes, resize/background/resume and device/surface recovery checks.

Implementation and policy evidence: [Phase 6 fidelity/performance report](magic_characters_phase6.md).
The native/browser validation harness covers cold/warm renderer creation,
resize and surface-frame recovery where its host is available. Physical
Android/iOS sustained sessions remain unverified from this checkout and are an
explicit platform-owner gate.

Primary files: `renderer/character_quality.rs`, `renderer/character_gpu.rs`,
`renderer/draw.rs`, `renderer/character_material.rs`, and platform capture
tooling.

Exit: ratified 18-character budget and documented 50-character stress behavior; no quality tier hides a core shape/face defect. Features that exceed budget are explicitly deferred rather than quietly making the portable baseline slower.

### Phase 7 — Persistent local and remote identity

- [ ] Add bounded appearance APIs, stable entity generation, revision handling and richer versioned remote motion inputs.
- [ ] Update header/WASM contracts and preserve old setters/snapshot exports.
- [ ] Integrate package/client/backend companion changes: persistence, replication, preferences and assets; test mixed client capability and missing content.
- [ ] Verify late join, reconnect, reorder/slot reuse, emote deduplication and identity continuity across worlds.

Primary files: `engine/state.rs`, `engine/worlds.rs`, `types.rs`, `ffi/control.rs`/`ffi/session.rs`, C header, `web_renderer.rs` where required, companion repositories.

Exit: two different clients show the same stable outfit/species for each player after reconnect and world transition; legacy clients retain a usable fallback. Rust-only completion cannot satisfy this integration gate.

### Phase 8 — Rollout and art-language expansion

- [ ] Ship a reversible renderer appearance mode (`legacy` / `magic`) through a small configuration boundary for comparison and staged rollout. Defaults switch only after the preceding production gates pass.
- [ ] Run mixed-content compatibility, ordinary gameplay/camera checks and regression captures before changing the default.
- [ ] Publish the final official asset style/proportion/fit guide and a tree/prop/accessory example using the same language.
- [ ] Remove the temporary CPU-expanded shape path after the instanced path is verified; retire legacy rendering only after the client/package compatibility window is agreed. Preserve old-color migration longer than the visual rollback switch.

Exit: rollout evidence, fallback behavior and ownership documented; all three bodies and six outfits are production identity choices, not just showcase props.

### Dependency order

`0 -> 1 -> 2 (shape approval) -> 3 -> 4 -> 5 (vertical slice) -> 6 -> 7 -> 8`

Schema/host contract design for Phase 7 can start after Phase 1, and outfit authoring can begin after Phase 2's proportions stabilize. Shipping those changes still depends on the renderer, fit and compatibility gates. Do not freeze an enormous wardrobe against a rig still undergoing silhouette changes.

## 11. Verification and acceptance evidence

### Automated checks to add with implementation

| Layer | Meaningful checks |
| --- | --- |
| Geometry | Finite vertices; positive bounds; valid indices and winding; approximately unit outward normals; shared-edge position/normal continuity; zero/max radius; taper; invalid inputs and subdivision limits |
| Rig/pose | No cycles/missing anchors; correct parent transforms; bounded joint gaps; hands/shoes follow limbs; finite spring state; deterministic sampling; conservative animated bounds |
| Motion | Idle settling at arbitrary phase; sprint/NPC snapshot regression; walk into wall; zero/repeated tick; yaw wrap; jump/fall/landing on raised objects; 30/60/120 Hz agreement within defined tolerance |
| State lifecycle | Despawn/reuse resets; appearance survives world change; teleport/correction does not trigger landing bursts; repeated sync/draw and renderer recreation do not duplicate events |
| Appearance | Old four-color manifests; supported/unknown version; invalid colors/IDs; missing body/outfit; slot conflicts; coverage; fit variants; size limits; atomic failed-load fallback; monotonic revisions |
| ABI | Existing snapshot length/stride/suffix meanings; old remote setter behavior; new buffer/status/ownership contracts; native/WASM parsing equivalence |
| GPU | Shader/pipeline creation on required backends; 1x and supported MSAA; resize attachments; correct face/effect occlusion; no validation errors; bounded GPU resources |

Use tolerances and invariants rather than brittle exact float snapshots. Cross-backend pixel identity is not required; controlled image comparisons need per-backend baselines and perceptual thresholds. A successful `cargo check` does not execute WGSL rendering or prove visual quality.

Existing repo checks to run on implementation changes:

```sh
cargo test
cargo check
cargo check --target wasm32-unknown-unknown --features web-renderer
```

For browser integration, use the existing `scripts/build_web_renderer.sh --debug` and `--release` workflows with the documented sibling `web` checkout and installed binding tool. Those scripts write generated assets outside this repo. For iOS, build through the existing host Xcode/static-library integration and capture on a real device. For Android, use the maintained host's target/toolchain and verify the actual GLES renderer. Record unavailable targets/devices as unverified; compile success is not runtime evidence.

### Visual and interaction review matrix

- Species: person, cat, dragon; each bare body, six hero pairings, every declared wearable fit, mixed crowd, dark/light skin/fur and high-contrast garment combinations.
- Camera: front/side/back/three-quarter, black silhouette, close-up face/material, default third-person, maximum zoom, first-person entry/exit, tall hat/wings near camera and narrow spaces.
- Viewports: 390x844, 768x1024, 1280x800 and 1440x900, with representative device pixel ratios. Evaluate the actual letterboxed world viewport on portrait; keep shared UI readable and unclipped.
- Motion: idle, walk/run transition, stop, reverse, strafe, turn across yaw wrap, wave, jump, ledge fall, land on floor/block, walk against obstacle, teleport, correction and resume.
- Rendering: ordinary palette/daylight, adverse light angles, fog, small projected size, reduced effects, 1x/AA modes, missing asset, old package, representative WebGPU/GL/Metal/GLES paths.

Reviewers must be able to answer yes to: do the untextured silhouettes belong together; do outfit changes alter shape; are eyes/mouth readable in play; do gaps communicate magic without visual detachment; do feet feel grounded; does detailed cloth reward close inspection without distant shimmer; and does the scene remain coherent with existing world art?

Store reproducible capture settings and measurement summaries alongside the reviewed artifact references. Art direction needs recorded review, not a unit test asserting a character is appealing.

## 12. Risks, decisions and first implementation task

| Risk | Mitigation / decision gate |
| --- | --- |
| Rounded old anatomy still resembles a generic block person | Phase 2 black-silhouette review before wardrobe investment |
| Geometry/material detail overwhelms mobile | Phase 3 instancing before six outfits; bounded meshes/materials, LOD and sustained profiling |
| Eyes/mouth disappear or look pasted on | Head-local planar face region, projected-size review, deliberate contrast and controlled offsets |
| Gaps look broken or clothing fills them | Author clearances and energy cores; inspect neutral poses and every garment/joint extreme |
| Outfit/species combinatorics explode | Shared rig, explicit supported fit matrix, coverage masks, a few common garments before arbitrary combinations |
| New effects obscure world/UI or cause flicker | Separate depth/blend passes, conservative particles, reduced effects and AA review |
| Color/material mismatch between browser and native | Phase 3 explicit color contract with swatches and UI regression captures |
| Network indices attach identity/animation to the wrong player | Stable key/generation and additive protocol; conservative reset for old slot APIs |
| Visual changes alter gameplay fairness/camera comfort | Fixed collider/speeds, bounded cosmetic envelopes, camera anchors and collision-space review |
| Showcase success mistaken for persistent identity | Separate Phase 7 cross-repository gate with two-client/reconnect evidence |

Decisions to record during the work, with defaults that allow progress:

1. Art owner ratifies the proportion sheet and species silhouettes in Phase 2; start with the ratios in this RFC.
2. Platform owners name baseline/low-tier devices in Phase 0; keep Metal, browser WebGPU/GL and Android GLES within the existing supported architecture.
3. Renderer owner chooses the exact color-management migration and instance layout in Phase 3 after inspecting real adapter limits; retain a portable 1x/no-extra-effects tier.
4. Content owner ratifies fit coverage, asset limits and versioning in Phase 5; default to bundled procedural recipes and six hero outfits.
5. Client/backend owners agree stable IDs, motion timestamps, persistence authority and the compatibility window before Phase 7 ships. Until then, show package defaults and clearly bounded remote fallbacks.

The first implementation task should be **Phase 0's deterministic showcase and baseline capture, followed by Phase 1's typed-motion/sprint regression and rounded primitive**. Its review should contain legacy-versus-rounded geometry captures, unchanged snapshot compatibility tests, and measured costs. That establishes a trustworthy place to evaluate the shape language before committing to detailed content.
