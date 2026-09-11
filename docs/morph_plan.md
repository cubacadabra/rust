# Morph system plan

Status: in progress; last updated September 10, 2026

## Progress

- [x] Record the target architecture and phased delivery plan.
- [x] Scaffold the portable `cubacadabra-morphs` crate without renderer, filesystem, or platform dependencies.
- [x] Add shared bounded asset IDs, published-ID rules, versioned capability IDs, and deterministic diagnostics.
- [x] Add a bounded V2 `MorphLoadout` contract and explicit V1 appearance compatibility mapping.
- [x] Make the engine consume shared morph asset-ID validation without changing V1 rendering behavior.
- [x] Add the shared asset/catalog schema and seed the current compatibility catalog and person presets.
- [x] Add a Studio-side catalog inspection adapter using the shared crate, without depending on engine internals.
- [x] Add the Studio-only `.morph.json` rigid-accessory source manifest and attachment validation contract.
- [x] Add dependency-free GLB container/JSON inspection for declared rigid-accessory LODs and triangle budgets.
- [x] Add a Studio authoring CLI for validating a sidecar against a GLB export.
- [x] Record Phase 0 terminology and baseline capture inventory.
- [ ] Produce and review the Phase 0 PNG/JSON baseline artifacts.
- [x] Complete Phase 1 catalog resolution and fit/conflict resolution.
- [x] Build the rigid Blender accessory vertical slice.
- [x] Add a checked-in three LOD rigid accessory fixture and deterministic `.morphpack` compiler.
- [x] Add a bounded shared-runtime `.morphpack` decoder and validation CLI.
- [x] Add an additive engine renderer registry for bounded rigid morph-pack GPU resources.
- [x] Select registered rigid morphs from V1-compatible equipment loadouts, attach them to the animated rig, and draw the matching Near/Mid/Far mesh.
- [x] Add Studio-side publish flow for validated rigid accessory packs.
- [x] Generate a deterministic PNG thumbnail from the shaded preview mesh.
- [x] Add the Morphs workspace shell MVP with catalog library, search, and inspector.
- [x] Add the first Studio GLB import/preview loop with bounded mesh decoding and a native file picker.
- [x] Add source-structure inspection for imported drafts, including node/mesh/material counts and LOD candidates.
- [x] Add draft `.morph.json` sidecar export from the Studio mapping controls.
- [x] Reopen a `.morph.json` sidecar, restore its mappings, and revalidate its referenced GLB.
- [x] Save and reopen incomplete Studio morph drafts without weakening publish validation.
- [x] Make the imported preview readable with a shaded surface pass and wireframe toggle.
- [x] Add source/Near/Mid/Far preview selection for uniquely mapped GLB nodes.
- [x] Apply imported PBR base color to the CPU shaded preview when available.
- [x] Keep the Morphs preview and inspector headers collision-free at the compact Studio width.
- [x] Build the Morphs workspace MVP.
- [x] Add package-host asset discovery and automatic pack registration for deployed iOS, Android, and web sessions.
- [x] Make the D1 catalog return canonical shared asset definitions, including pack-less procedural compatibility assets.
- [x] Route Studio base, face, hair, outfit, and accessory selection through one V2 loadout with a temporary V1 renderer projection.
- [ ] Replace the temporary V1 renderer projection with the first Blender-authored skinned Person base.

### Work log

- 2026-09-10: Started Phase 0/1 with the portable contract crate. The first engine integration deliberately replaces only the duplicate equipment asset-ID validator; animation, rendering, FFI, snapshots, and appearance fallback remain on the V1 path.
- 2026-09-10 verification: `cubacadabra-morphs` has 14 passing unit tests and passes strict Clippy; all 171 normal Rust workspace tests and all 10 Studio tests pass; normal Metal, host Android-feature, and Studio checks pass. The WebAssembly target is not installed locally. The existing `dev-showcase` build is currently blocked outside the morph changes by stale renderer validation fixtures (`texture_bounds` and `world_pipeline` arguments), so new baseline captures have not yet been recorded.
- 2026-09-10: Added `MorphCatalog`, asset metadata validation, compatibility fixture data, and the Studio inspection adapter. The catalog is metadata-only at this stage; GLB vertex-buffer ingestion and compiled packs remain the next vertical slice.
- 2026-09-10: Added the Studio-only authoring crate and metadata-only rigid accessory manifest validation. GLB vertex-buffer decoding and `.morphpack` compilation remain intentionally deferred until this sidecar contract is stable.
- 2026-09-10 verification: the authoring crate has 5 passing tests and passes strict Clippy; Studio has 10 passing tests and `cubacadabra-morphs` has 14 passing tests. No new runtime or mobile/web dependencies were introduced.
- 2026-09-10: Added a dependency-free GLB v2 boundary inspector to the Studio authoring crate. It validates the container, JSON nodes/meshes/accessors, declared near/mid/far node mapping, supported primitive modes, and manifest triangle budgets; it intentionally does not decode vertex buffers or publish a runtime pack yet.
- 2026-09-10: Added `morph_validate <manifest.morph.json> <asset.glb>` for repeatable local artist checks. Running it against `~/Downloads/test_top_hat.glb` confirmed the export is GLB v2 with one `Cylinder` node/mesh and 248 triangles, but correctly rejected it because the required distinct near/mid/far LOD nodes are not present.
- 2026-09-10 verification: the authoring crate now has 6 passing tests (including duplicate-LOD rejection), strict Clippy passes, the CLI returns exit code 1 with three actionable LOD-node diagnostics for the supplied top-hat export, and the full Rust workspace remains at 171 passing tests.
- 2026-09-10: Added shared deterministic loadout resolution. It validates base/part references, supported bases, rig and fit-profile intersections, occupied-slot collisions, conflict tags, and engine capability requirements; presets route through the same resolver. Studio now calls that shared resolver through its catalog adapter.
- 2026-09-10 verification: `cubacadabra-morphs` has 18 passing tests, Studio has 11 passing tests, and strict Clippy/formatting pass. The full workspace remains platform-safe because resolution is CPU-only data work with no renderer or per-frame dependencies.
- 2026-09-10: Added all 21 current V1 analytic face presets to the shared compatibility catalog so migrated V1 appearances can resolve their `MorphLoadout.face` reference instead of stopping at an unknown catalog asset.
- 2026-09-10: Recorded the Phase 0 terminology, V1-to-V2 identity map, authored/runtime inventory, web thumbnail inventory, and 15-scenario capture matrix in [`morph_baseline_inventory.md`](morph_baseline_inventory.md). The inventory is complete; PNG/JSON capture artifacts remain blocked by the pre-existing `dev-showcase` renderer validation compile errors.
- 2026-09-10 verification: the complete workspace now passes 175 tests (10 app, 4 client, 143 engine, 18 morphs); normal engine and host Android-backend checks pass. The WASM check remains unavailable because `wasm32-unknown-unknown` is not installed locally.
- 2026-09-10: Added the shared Morphs workspace shell entry. It loads the same catalog used by the resolver, provides searchable kind-grouped assets, and shows a compact identity/fit/capability inspector beside the existing preview surface. Import, reimport, diagnostics, and production rendering remain the next UI slices.
- 2026-09-10 verification: Studio has 11 passing tests after the Morphs tab was added; the existing logo texture and shared platform menu boundaries remain unchanged.
- 2026-09-10: Added the first usable Studio import loop. The Studio-only authoring crate now decodes bounded POSITION/index data from the first GLB primitive; the Morphs inspector opens a native GLB picker and the center panel renders a CPU wireframe with source, vertex, index, and triangle counts. Import errors stay inline and do not enter the engine/runtime path.
- 2026-09-10 verification: the authoring crate has 7 passing tests and passes strict Clippy; Studio builds, its 11 tests pass, and the release binary builds. `test_top_hat.glb` is ready to inspect in the GUI; it will preview despite lacking the distinct near/mid/far LOD nodes required for publish validation. Strict Studio Clippy still reports pre-existing warnings in `main.rs`/`network.rs`; the new preview code is clean.
- 2026-09-10: Added raw-GLB source inspection for the Morphs draft review. Imported files now expose node, mesh, material, aggregate triangle, and heuristic Near/Mid/Far LOD-candidate data in the inspector, with an explicit preview-only warning when the source is not publish-ready.
- 2026-09-10: Added editable in-memory draft mapping controls for the imported source. Artists can enter an attachment joint and three LOD node names, then validate that joints are safe, nodes exist in the GLB, and LOD mappings are distinct.
- 2026-09-10 verification: the authoring crate has 8 passing tests and passes strict Clippy; Studio has 11 passing tests, formatting and diff checks pass. The next slice is sidecar export/persistence and then shaded/material preview.
- 2026-09-10: Added draft `.morph.json` export from the Studio mapping controls. A raw import receives a bounded in-memory headwear draft identity derived from its filename, and the exporter writes that asset, a safe relative GLB filename, rigid head attachment, mapped Near/Mid/Far nodes, and bounded triangle counts through the same sidecar validator used by the CLI.
- 2026-09-10: Improved the raw GLB preview with depth-offset projection, a CPU shaded surface pass, and a wireframe toggle so imported meshes remain readable while topology is inspected.
- 2026-09-10 verification: Studio has 12 passing tests, the authoring crate has 8 passing tests, Studio `cargo check` passes, and `git diff --check` passes. Local `rustfmt` and `clippy` components are unavailable in the installed Rust toolchain.
- 2026-09-10: Added sidecar reimport through a native `.morph.json` picker. Studio resolves the sidecar’s safe relative GLB path, validates the full source contract and exact mapped LOD counts, restores the draft asset and mapping fields, then replaces the preview atomically.
- 2026-09-10 verification: Studio has 14 passing tests, the authoring crate has 8 passing tests, Studio `cargo check` passes, and both repositories pass `git diff --check`. Local `rustfmt` and `clippy` components remain unavailable in the installed Rust toolchain.
- 2026-09-10: Added bounded Near/Mid/Far preview decoding and selector controls. Raw imports use heuristic unique LOD candidates; sidecar imports use their explicit node mappings, with Source retained as the fallback when an export has no LOD meshes.
- 2026-09-10 verification: Studio has 14 passing tests, the authoring crate has 8 passing tests, Studio `cargo check` passes, and both repositories pass `git diff --check`. Local `rustfmt` and `clippy` components remain unavailable in the installed Rust toolchain.
- 2026-09-10: The CPU preview now reads a valid first-primitive PBR `baseColorFactor` from each decoded GLB mesh and uses it for shaded rendering, with the Studio accent color as a safe fallback.
- 2026-09-10 verification: Studio has 14 passing tests, the authoring crate has 8 passing tests, Studio `cargo check` passes, and both repositories pass `git diff --check`. Local `rustfmt` and `clippy` components remain unavailable in the installed Rust toolchain.
- 2026-09-10: Fixed compact Morphs header collisions by replacing inspector text actions with tooltip-backed icon controls and giving the Source/Near/Mid/Far selector its own painted row and layout space.
- 2026-09-10 verification: Studio has 14 passing tests, Studio `cargo check` passes, and both repositories pass `git diff --check`. Local `rustfmt` and `clippy` components remain unavailable in the installed Rust toolchain.
- 2026-09-10: Added a checked-in `TopHat_Near` / `TopHat_Mid` / `TopHat_Far` GLB fixture with a matching sidecar and a small fixture generator. The authoring crate now compiles validated LOD meshes plus manifest metadata into a deterministic bounded `.morphpack` envelope.
- 2026-09-10: Added the Studio `Publish .morphpack` action. It reruns sidecar and GLB validation, writes the pack through a native save dialog, and reports the published asset ID and byte size inline.
- 2026-09-10: Added deterministic PNG thumbnail generation from the current bounded shaded preview, using imported base color when available and the same CPU mesh data as pack compilation.
- 2026-09-10: Added resumable `.morph.draft.json` save/reopen for incomplete imports. Drafts preserve the current asset, attachment, partial LOD mapping, and known triangle counts; strict `.morph.json` export and `.morphpack` publishing still require validated distinct LOD nodes.
- 2026-09-10 verification: Studio has 17 passing tests and the authoring crate has 9 passing tests after the draft persistence change; `git diff --check` passes.
- 2026-09-10 verification: Studio has 15 passing tests, the authoring crate has 9 passing tests, the three LOD fixture passes `morph_validate`, Studio `cargo check` passes, and both repositories pass `git diff --check`. Shared runtime renderer registration remains next.
- 2026-09-10: Added a bounded, renderer-neutral `.morphpack` decoder to `cubacadabra-morphs`. It owns the shared magic/schema/resource limits, validates the embedded asset and rigid attachment, bounds vertex/index allocations, rejects malformed floats, invalid indices, mismatched triangle counts, unsupported color flags, and trailing bytes, and returns deterministic diagnostics. Studio authoring now reuses the shared pack constants and exposes `morph_pack_validate` for local pack checks.
- 2026-09-10 verification: `cubacadabra-morphs` has 20 passing tests, Studio has 18 passing tests, the full Rust workspace has 143 passing engine tests, normal and Android-backend checks pass, and `/Users/aa/Downloads/test_top_hat.morph.morphpack` passes the shared decoder at 14,625 bytes with Near 248, Mid 124, and Far 48 triangles. The WASM check remains unavailable because `wasm32-unknown-unknown` is not installed locally.
- 2026-09-10: Added an additive engine-side morph registry. Native, WebRenderer, and C FFI entry points now decode a compiled pack once and upload its three immutable LOD meshes, with generated normals, replacement-by-asset-ID, a 32-pack limit, and a 16 MiB GPU residency cap.
- 2026-09-10: Completed the live rigid-accessory vertical slice. Studio now registers a validated pack immediately after publish, applies its asset ID to the local V1-compatible hat slot, and the shared renderer resolves that ID per character, applies the authored attachment transform after the animated joint matrix, and batches the matching Near/Mid/Far GPU mesh with the existing opaque character pass. An explicit `Load .morphpack` action also exercises an existing compiled pack directly.
- 2026-09-10 verification: the shared engine tests (143), Studio tests (18), morph crate tests (20), authoring crate tests (9), normal engine check, Android-backend check, Studio check, formatting checks, and the supplied 14,625-byte top-hat pack validation pass. The Web/WASM check remains unavailable because the `wasm32-unknown-unknown` target is not installed locally. Deployed non-desktop hosts still need to discover package-declared pack files and call the already-exposed registration APIs.
- 2026-09-10: Reviewed the first live Blender-to-game result and fixed attachment calibration end to end. Studio now preserves translation, rotation, and scale through draft save, sidecar export/reimport, pack compilation, and runtime loading; shows source and runtime dimensions; previews the transformed mesh against a standard person-head reference; offers a deterministic `Fit to person head` action; and blocks obviously implausible headwear widths. Strict GLB publishing now requires applied Blender node transforms, one primitive per LOD, and triangle-list geometry, matching what the pack compiler actually consumes.
- 2026-09-10: Recalibrated the supplied top hat from a 3.00-unit identity-scale brim to a 1.35-unit runtime brim (`0.451` uniform scale, `0.657` head-local Y offset) and rebuilt `/Users/aa/Downloads/test_top_hat.morph.morphpack`.
- 2026-09-10: Added a second Blender-free rigid proof asset at `/Users/aa/Downloads/headphones.glb`, with Near/Mid/Far nodes and a matching sidecar/pack. The runtime compositor now retains all bounded V1 equipment entries, allowing the headphones to coexist with the top hat through separate slots.
- 2026-09-10: Removed duplicate triangle-count authority from published sidecars. `asset.lod` is now the budget; legacy `geometry.triangleCounts` is accepted only for compatibility and rejected when it disagrees.
- 2026-09-10: Added package-host morph discovery and registration for web, iOS, and Android. Hosts validate bounded `.morphpack` paths, load packs before the first draw, and register them through the existing renderer APIs. The Rust `dev-showcase` validation fixtures were also repaired so baseline capture work can resume.
- 2026-09-10 verification: the generated headphones asset validates at Near 84 / Mid 60 / Far 36 triangles and its compiled pack validates at 4,504 bytes. Rust (143 engine tests), Studio (19 tests), morph authoring (10 tests), and portable morphs (21 tests) pass; Android Gradle and iOS Simulator builds pass; the web app contract check passes. The browser WASM rebuild remains unavailable because the installed toolchain cannot provide `core` for `wasm32-unknown-unknown`.

Deployed game packages declare compiled morphs under `assets.morphPacks`; the key is the published asset ID and the path must stay inside `assets/`:

```json
{
  "assets": {
    "morphPacks": {
      "cuba:headwear/test-top-hat.v1": {
        "path": "assets/morphs/test_top_hat.morphpack"
      },
      "cuba:headwear/headphones.v1": {
        "path": "assets/morphs/headphones.morphpack"
      }
    }
  }
}
```

Hosts load these files before the first draw and pass only the bounded compiled format to the renderer. The renderer enforces its 32-pack and 16 MiB residency limits; hosts enforce the same package limits before upload.
- 2026-09-10 verification: Studio has 19 passing tests, morph authoring has 10 passing tests, the portable morph crate has 21 passing tests, the shared engine has 143 passing tests, Android-feature compilation passes, both repositories pass formatting and diff checks, and the rebuilt 14,633-byte pack passes the shared decoder with Near 248, Mid 124, and Far 48 triangles.
- 2026-09-10: Started the clean V2 migration. The portable crate now owns a single V2-to-V1 compatibility projection for the current procedural renderer; Studio stores and edits one V2 loadout across base, face, hair, outfit, and equipment selections. This keeps the renderer replacement isolated behind one boundary.
- 2026-09-10: Added backend migration `015_seed_morph_catalog.sql` for the complete 34-asset compatibility catalog. D1 now serves canonical `MorphAssetDefinition` objects; procedural entries have no pack URL, while the two proof headwear entries retain their R2 URLs. V2 account appearance payloads are accepted and validated without removing V1 compatibility.
- 2026-09-10 verification: local D1 contains 36 published morph rows (34 catalog assets plus two proof packs), the local catalog endpoint returns all 36 with two packs, Rust has 144 passing tests, Studio has 20 passing tests, and backend tests plus JavaScript syntax checks pass.

## Architecture and delivery plan

Yes: create a new portable cubacadabra-morphs crate, but do not move the whole renderer into it. The crate should own
  the morph vocabulary, schemas, resolution, compatibility, and validation. Blender importing and compilation should be
  Studio-only, while GPU rendering remains in the shared engine.

  The central product rule should be:

  > New art using existing morph capabilities requires data changes only. Rust changes are required only when the art
  > introduces a new rendering, deformation, rigging, animation, or material capability.

  ## Where we are now

  The current implementation is a strong prototype, but several things are fused together:

  - BodyId combines species, body, gender presentation, and hairstyle. The girl and nonbinary bodies are effectively
    person variants with hard-wired hair selection. See /Users/aa/cubacadabra/rust/src/character/definition.rs:7 and /
    Users/aa/cubacadabra/rust/src/character/hair.rs:47.

  - Bodies and outfits are Rust enums, and compatibility is implemented with match statements. Adding catalog content
    therefore requires recompiling the engine. See /Users/aa/cubacadabra/rust/src/character/definition.rs:105.

  - Useful catalog concepts already exist—occupied slots, coverage, conflicts, materials, LODs, and provenance—but
    validation requires the catalog to exactly match the Rust enums. See /Users/aa/cubacadabra/rust/src/character/
    catalog.rs:99.

  - The renderer precompiles every BodyId × OutfitId combination. That Cartesian model will not scale to hundreds of
    independent hairstyles, garments, hats, and accessories. See /Users/aa/cubacadabra/rust/src/renderer/
    character_gpu.rs:193.

  - Rendering is currently optimized around rigid pieces, procedural meshes, special hero geometry, and a one-off sleeve
    deformation. The documented runtime has a fixed 15-joint hierarchy, 48 rigid parts, 20 hair curves, 384 meshes, and
    32 MiB residency. See /Users/aa/cubacadabra/rust/docs/character_runtime.md:6.

  - Studio currently has a static shell and layout-preview state; even Save is not connected. Morph authoring therefore
    also needs a real document/command layer, not just additional widgets. See src/shell.rs:312.

  - The web account UI and backend API hard-code the three person choices. See /Users/aa/cubacadabra/web/my-cube/my-
    cube-page.js:10 and /Users/aa/cubacadabra/backend/worker/app/openapi-shared.js:8.

  - Multiplayer appearance persistence is already versioned, bounded, revisioned, and mostly opaque to the server. That
    is valuable infrastructure to preserve. See /Users/aa/cubacadabra/backend/worker/durable-objects/world-socket-
    appearance.js:22.

  The three PNGs should eventually become generated preset thumbnails, not the source of truth for available morphs.

  ## Proposed architecture

  Blender .blend
      │ export
      ▼
  GLB + .morph.json
      │
      ▼
  Studio morph importer/compiler ─────► validation and capability report
      │                                     │
      │ preview through production renderer │ missing engine capability
      ▼                                     ▼
  Compiled .morphpack                 reproducible engine fixture
      │
      ├── Studio preview
      ├── engine runtime
      ├── web/WASM
      ├── iOS
      └── Android

  Network/account state carries only stable asset IDs and parameter overrides,
  never meshes or Blender data.

  ### 1. Portable core crate

  Create:

  ../rust/crates/morphs/

  Package name: cubacadabra-morphs.

  It should be deliberately small and portable:

  - No wgpu.
  - No windowing or native file dialogs.
  - No filesystem watching.
  - No Blender or general glTF importer.
  - No per-frame work.
  - Dependencies limited to things already justified for shared targets, primarily serde, serde_json, and possibly glam.

  It should own:

  - Stable MorphAssetId and version rules.
  - Morph source and compiled-pack schemas.
  - MorphLoadout.
  - Rig-profile and fit-profile descriptions.
  - Slot, coverage, conflict, and material parameter definitions.
  - Capability identifiers.
  - Resource bounds.
  - Catalog resolution and validation.
  - V1 CharacterAppearance compatibility conversion.
  - Deterministic diagnostics with error codes.
  - Pack decoding into CPU-side immutable definitions.

  Initially move or adapt only the CPU/data portions of character/definition.rs, character/catalog.rs, and the hair
  asset schema. Keep animation, gait, renderer pipelines, engine state, FFI, and procedural compatibility geometry in
  cubacadabra-engine.

  This boundary is worthwhile because both Studio and the engine must interpret exactly the same morph definition.
  Duplicating the schema in Studio would become dangerous quickly.

  ### 2. Studio-only authoring crate

  Create something like:

  studio/crates/morph_authoring/

  This owns:

  - GLB ingestion.
  - Blender convention checking.
  - Coordinate and unit conversion.
  - Skeleton and joint mapping.
  - Mesh, skin-weight, material, and texture analysis.
  - LOD processing.
  - Source-file tracking and reimport.
  - Compilation into .morphpack.
  - Deterministic thumbnail/capture requests.
  - Human-readable repair guidance.

  Only this crate should need a general glTF dependency. Mobile, web, and the normal engine should load the constrained
  compiled format, not a general-purpose model importer.

  Start with timestamp polling for reimport. A file-watching dependency is unnecessary for the first version.

  ### 3. Renderer remains engine-owned

  The renderer should consume a resolved, renderer-neutral structure from cubacadabra-morphs, for example:

  ResolvedMorph {
      rig_profile,
      render_nodes,
      materials,
      bounds,
      camera_anchors,
      secondary_motion,
  }

  GPU meshes should be cached independently by content hash. A character loadout then becomes a short list of references
  to shared meshes and materials. Do not generate and upload every possible body/outfit/accessory combination.

  Keep the current renderer as the V1 compatibility path while the new path matures.

  ## Morph data model

  Separate four concepts that are currently mixed together.

  ### Morph base

  The foundational body and animation contract:

  - cuba:base/person.v1
  - cuba:base/cat.v1
  - cuba:base/wolf.v1
  - cuba:base/dragon.v1

  A base declares:

  - Rig profile.
  - Fit profile.
  - Body render assets.
  - Skin/material parameters.
  - Face anchors or face capability.
  - Camera and nameplate anchors.
  - Collision dimensions.
  - Supported animation set.
  - Optional anatomy anchors such as tail base, horn roots, and wing roots.

  “Boy,” “Girl,” and “Nonbinary” should become compatibility presets, not different body classes. Their legacy IDs can
  map to the person base plus the appropriate hair, colors, and other selections.

  ### Morph part

  An individually reusable asset:

  - Hair.
  - Headwear.
  - Facewear.
  - Top.
  - Outerwear.
  - Bottom.
  - One-piece garment.
  - Hands/gloves.
  - Footwear.
  - Neck, waist, or back accessory.
  - Tail, wings, horns, ears.
  - Held items.

  Each part declares:

  - Occupied slots.
  - Covered body regions.
  - Conflict tags.
  - Compatible rig and fit profiles.
  - Attachment or skinning mode.
  - LOD meshes.
  - Material slots and editable parameters.
  - Bounds.
  - Required engine capabilities.
  - Provenance and license.

  An outfit becomes an editing preset containing several parts, not a privileged renderer enum.

  ### Morph loadout

  The compact appearance sent through packages, accounts, and multiplayer:

  {
    "version": 2,
    "base": "cuba:base/person.v1",
    "parts": [
      "cuba:hair/side-ponytail.v1",
      "cuba:top/everyday-hoodie.v1",
      "cuba:bottom/cuffed-trousers.v1",
      "cuba:feet/low-sneakers.v1"
    ],
    "parameters": {
      "skin": "#e8ae86",
      "hair": "#65412f",
      "hoodie": "#2d6663"
    },
    "face": "cuba:face/soft-happy.v1",
    "revision": 17
  }

  Keep overrides bounded and transmit only values differing from defaults. The current 4 KiB appearance limit should
  remain a target.

  ### Rig profile versus fit profile

  These must be distinct:

  - A rig profile describes animation topology and semantic joints.
  - A fit profile describes body measurements, envelopes, garment landmarks, and attachment transforms.

  Several body shapes can use biped15.v1 while requiring different garment fits. Conversely, two visually similar
  creatures might require different animation rigs.

  Do not attempt universal automatic garment fitting. Provide canonical Blender templates for each fit profile and
  require an authored fit or explicitly supported adjustment range.

  ## Blender contract

  Use GLB as the exchange format, with a reviewable .morph.json sidecar as the authoritative Cubacadabra metadata.

  Provide Blender template files containing:

  - Canonical scale, axes, and forward direction.
  - Named skeleton and neutral bind pose.
  - Body-envelope reference meshes.
  - Named attachment empties.
  - Material naming conventions.
  - Near/mid/far LOD collections.
  - Coverage-region references.
  - Example hat, hair, garment, and tail.
  - Export checklist.

  The rigid-accessory exchange convention is one Blender/GLB unit per engine world unit. Mapped LOD objects must have
  Location, Rotation, and Scale applied on the object and its parent hierarchy, and contain exactly one triangle-list primitive. Geometry is authored in local
  accessory coordinates; the sidecar attachment transform is then applied relative to its semantic joint. For the
  standard person reference, the head is 1.10 × 0.92 × 0.78 units. Studio's headwear fit action targets a 1.35-unit brim
  and places its lowest point just inside the top of that head; artists can then adjust the persisted offset and scale.

  The importer should reject or flag:

  - Unapplied transforms.
  - Wrong scale or orientation.
  - Unknown or missing joints.
  - Excessive vertices, weights, materials, textures, or bones.
  - Non-finite geometry.
  - Unsupported external file references.
  - Missing LODs.
  - Unmapped material slots.
  - Invalid attachment names.
  - Clothing outside its declared fit envelope.
  - Unsupported shader or deformation requirements.

  Do not start with a Blender Python add-on. Stabilize the GLB and metadata contract first; an add-on can later automate
  export and validation.

  ## Studio Morphs workspace

  Add Morphs as a shared in-window workspace alongside World, Assets, Materials, and Test. Keep it a dense production
  tool.

  Visual thesis: a neutral, compact character workshop where the rendered morph dominates and diagnostics stay close to
  the thing they affect.

  Layout:

  - Left: searchable morph library grouped by base, hair, clothing, accessories, anatomy, and presets.
  - Center: production renderer preview with orbit, front/side/back cameras, LOD selector, wireframe, skeleton,
    coverage, and fit-envelope overlays.

  - Right: contextual inspector for the selected base or part.
  - Bottom, collapsible: validation issues, import log, performance budgets, and test matrix.

  Primary workflow:

  1. Create a Morph Asset.
  2. Choose its kind, target rig, and fit profile.
  3. Import a GLB using the platform-native file dialog.
  4. Review detected meshes, joints, materials, transforms, and LODs.
  5. Map ambiguous joints or material slots.
  6. Preview against supported bases.
  7. Exercise idle, walk, run, jump, land, wave, and expression states.
  8. Check clipping at representative poses and camera distances.
  9. Reimport after Blender edits while preserving mappings and metadata.
  10. Publish only when validation passes.
  11. Generate thumbnails and a compiled pack from the same production renderer.

  Reimport should show a useful diff: changed mesh counts, bounds, material slots, skeleton, LOD sizes, and newly
  introduced capabilities.

  Studio should save drafts even if they need engine support. “Cannot publish” is different from “cannot continue
  working.”

  Because shell.rs is already large, Morphs should live in focused modules such as:

  src/morphs/
      model.rs
      commands.rs
      workspace.rs
      library.rs
      inspector.rs
      diagnostics.rs
      preview.rs

  Studio state mutations should go through commands so Save, undo/redo, reimport, native menus, and tests share
  behavior.

  ## When art requires Rust changes

  Every compiled asset declares capabilities such as:

  mesh.rigid.v1
  skin.biped15.linear.v1
  face.analytic.v1
  secondary.chain.v1
  material.cuba-pbr.v1
  material.emissive.v1

  Studio compares those against the selected engine build.

  Results should be explicit:

  - Ready: all capabilities supported.
  - Needs art fix: bad mapping, fit, limits, or metadata.
  - Engine support required: valid asset asks for an unsupported capability.

  For the last state, Studio should create a small handoff bundle:

  - Original morph metadata.
  - Reduced GLB fixture.
  - Required capability.
  - Expected screenshot or written behavior.
  - Current render/error capture.
  - Automatic validator test case.

  The Rust workflow is then:

  1. Define the capability contract and limits.
  2. Add the failing fixture/test.
  3. Implement it behind a Studio-only incubation feature if it changes rendering.
  4. Verify in Studio.
  5. Validate Metal, desktop backends, Android, and WebGPU/WebGL.
  6. Enable it for published runtime content.
  7. Preserve the old path until existing clients and content have migrated.

  Assets should never carry executable Rust, shaders, or arbitrary scripts. New behavior is added to the engine as a
  bounded, named capability.

  ## Recommended delivery sequence

  ### Phase 0: contracts and baselines

  - Write the terminology and architecture decision.
  - Freeze V1 captures and performance/resource measurements.
  - Define V2 IDs, loadout schema, capabilities, rig profiles, and fit profiles.
  - Specify compatibility mappings for every current body/outfit/hair combination.

  Exit criterion: every current appearance has an unambiguous V2 representation.

  ### Phase 1: portable morph crate

  - Add cubacadabra-morphs.
  - Implement schemas, catalog, limits, resolution, diagnostics, and V1 adapters.
  - Let the engine consume the crate while continuing to render through the old path.
  - Add schema golden tests and migration tests.

  Exit criterion: no externally visible behavior changes, and Studio can inspect a catalog without depending on engine
  internals.

  ### Phase 2: rigid Blender vertical slice

  Use a hat or glasses as the first asset.

  - GLB import.
  - Named joint attachment.
  - Material mapping.
  - Three LODs.
  - Compilation and renderer registration.
  - CLI validation and thumbnail generation.

  Exit criterion: a second Blender-authored hat can be added without changing Rust.

  ### Phase 3: Morphs workspace MVP

  - Library, import/reimport, production preview, inspector, and diagnostics.
  - Persistent morph document model.
  - Native dialogs.
  - Pose and camera test controls.
  - Draft save and deterministic compilation.

  Exit criterion: an artist can complete the rigid-accessory workflow without using the terminal.

  ### Phase 4: compositional runtime

  - Replace Cartesian body/outfit precompilation with content-hash mesh registration.
  - Resolve independent parts, slot conflicts, and coverage.
  - Atomically swap compiled morphs during Studio reimport.
  - Preserve the V1 renderer adapter.

  Exit criterion: dozens of accessories and combinations do not multiply GPU meshes unnecessarily.

  ### Phase 5: skinned bodies and clothing

  This is the largest engine step.

  - Four-weight linear skinning against the established joint palette.
  - Blender bind-pose validation.
  - Skinned garment vertex format and pipeline.
  - Body-region hiding/coverage.
  - Fit overlays and pose-extreme clipping tests.
  - One complete person base, jacket/top, bottom, and shoes.

  Exit criterion: a Blender-authored garment bends correctly through the full motion suite on every renderer backend.

  ### Phase 6: hair, tails, wings, and richer materials

  - Bounded secondary-motion chains.
  - Hair roots/scalp coverage.
  - Tail and wing attachment profiles.
  - Texture/material atlas or arrays.
  - Restricted Cubacadabra material model.
  - Convert existing procedural examples where conversion improves iteration; retain valuable procedural primitives as
    supported asset types.

  Exit criterion: one hairstyle and one creature appendage can be authored without asset-specific renderer code.

  ### Phase 7: product and distribution migration

  - Generate profile thumbnails from published presets.
  - Make web, iOS, and Android selectors consume a catalog.
  - Add versioned full morph persistence to accounts.
  - Keep body_id as a derived compatibility field during migration.
  - Send only V2 loadout IDs and overrides through multiplayer.
  - Add catalog/version negotiation and deterministic fallback.

  Exit criterion: the three hard-coded player choices are replaced by published morph presets without breaking older
  clients.

  ## Three proving assets

  Before calling the system comprehensive, prove these in order:

  1. A hat: rigid attachment, slots, conflicts, materials, LOD, publishing.
  2. A jacket: skinning, coverage, fit profiles, clipping tests.
  3. A tail: optional anatomy, secondary motion, new capability workflow.

  Together they exercise the three major complexity tiers.

  ## Success measures

  - At least 80–90% of new cosmetic content requires no Rust changes.
  - No asset ID appears in renderer match statements.
  - Studio and runtime use the same resolver and validator.
  - Runtime never parses Blender files or arbitrary glTF.
  - Morph selection remains bounded and network-safe.
  - Reimport is deterministic and does not lose artist mappings.
  - Published morphs render consistently on Studio, macOS, Windows, Linux, iOS, Android, WebGPU, and WebGL.
  - The normal mobile/web engine paths incur no authoring dependencies, file watchers, or per-frame catalog work.
  - Existing IDs, snapshots, physics, networking, and the three legacy presets remain compatible throughout migration.

This is roughly a multi-month program, not a shell feature. The smallest credible first investment is Phases 0–3 plus
the Blender hat vertical slice. That validates the crate boundary, artist loop, compiled format, and renderer
injection API before committing to the much more expensive skinned-mesh work.

## MVP D1/R2 and shared preview checkpoint

The first real catalog slice is now implemented:

- Backend migration `013_morphs.sql` adds published `morph_assets` metadata and revisioned
  `user_appearances` persistence.
- `GET /morphs/catalog` applies bounded kind/base/rig filters and returns stable pack URLs.
- Published pack objects are served through the existing Worker R2 binding at
  `/morphs/assets/:assetId`, with immutable cache headers.
- Studio starts in Morphs, requests the catalog from the backend, displays remote rows, downloads
  a selected pack, and applies it to the shared runtime appearance.
- D1 migration `015_seed_morph_catalog.sql` seeds the complete current compatibility catalog. The
  API returns each canonical shared definition and uses `pack: null` for assets that are still
  rendered procedurally; the two Blender proof assets remain R2-backed.
- Studio edits one V2 loadout for every library category and projects it through one temporary
  compatibility boundary into the procedural renderer. This is the deliberate seam for the first
  Blender-authored skinned base.
- The Rust renderer exposes a neutral `avatar_preview_mode` through the native, web, iOS, and
  Android bridges. It reuses the normal character/equipment GPU path and leaves normal game mode
  unchanged.

The two proof rows are seeded by `014_seed_morph_assets.sql`. The current deployment token can
apply D1 migrations and deploy the Worker, but lacks R2 Object Write permission; the seed rows are
therefore waiting on the two corresponding immutable pack uploads before remote selection can
complete. The local `.morphpack` files remain valid test inputs.
