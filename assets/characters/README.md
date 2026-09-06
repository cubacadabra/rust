# Phase 2 character shape proof

These are the authored, versioned inputs for the first Cubacadabra character
family. The runtime recipes live in `src/character/definition.rs`; this folder
records the proportion contract and the review fixture so art changes can be
compared without changing gameplay scale.

The three bodies share the 15-joint rigid-piece rig:

- `cuba:person.v1` — soft cube head, mitten hands and oversized sneakers.
- `cuba:cat.v1` — the shared body with rounded triangular ears, muzzle and a
  short articulated tail.
- `cuba:dragon.v1` — the shared body with a muzzle, horns, compact wings and a
  chunky tail.

Run the deterministic review capture with:

```sh
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 2 --output docs/baselines/magic-characters/phase2
```

The resulting front, side, back and black-silhouette PNGs are review evidence;
they are not a replacement for gameplay captures.

## Phase 5 wardrobe catalog

The six official outfits are finite procedural recipes in the renderer and
their fit/material/LOD/provenance contract is recorded in `catalog.json`.
`phase5_fixture.json` is the deterministic supported/unsupported combination
matrix used by the resolver tests. There are no external texture downloads in
this slice; `textureBytes: 0` is intentional until authored filtered textures
are added.

Validate the catalog offline before checking in an asset change:

```sh
cargo run --features dev-showcase --bin validate_character_assets
```

The validator enforces the six outfit IDs, declared body fits, non-empty
coverage/material/LOD metadata, provenance records, and the 8 MiB compressed
character payload ceiling. An unsupported body/outfit request falls back to
the bundled everyday hoodie atomically; it never partially equips a recipe.

## Phase 8 official transfer examples

[`soft_cubism_examples.json`](soft_cubism_examples.json) is the compact
non-character reference set: a squircle-foliage tree, a rounded toy-cart panel,
and a star badge that can attach to a character or prop. It uses the same
`soft-cubism.v1` language, explicit bounds/radii, LODs, anchors, materials, and
source/license provenance. Validate it with:

```sh
cargo run --features dev-showcase --bin validate_character_assets -- \
  --style-examples assets/characters/soft_cubism_examples.json
```

The validator command accepts the examples path with its normal positional
path argument as well; the explicit library helper is
`dev_showcase::validate_style_examples` for host/tool integration.
