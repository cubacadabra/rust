# Magic Characters Phase 5: wardrobe schema and six outfits

Implemented September 5, 2026.

Phase 5 adds an additive `avatars.*.character` package member. Legacy `skin`,
`shirt`, `pants`, and `shoes` remain unchanged and are used whenever the
character member is absent, unsupported, or invalid. The resolver validates
version, namespaced IDs, equipment slots, tint channels, and fit support before
returning one complete appearance. Unsupported versions use the legacy colors,
the person body, friendly face, and everyday hoodie. Invalid individual
equipment items are skipped without discarding the rest of the appearance.

The finite renderer catalog compiles all 18 body/outfit combinations before
normal drawing. It includes six silhouette-changing procedural outfits:

| Outfit | Authored fit |
| --- | --- |
| Everyday hoodie | Person, cat, dragon |
| Puffer explorer | Cat |
| Glossy raincoat | Person, cat, dragon |
| Star wizard | Dragon |
| Toy knight | Dragon |
| Fuzzy pajamas | Person |

Hoodie and raincoat are the common-fit proof. The other four are explicit hero
fits; unsupported pairings resolve to the hoodie. The recipes add bounded
geometry for hems, collars, hood/hat, armor plates, cuffs, boots, trim and
other silhouette cues, and use separate coat, soft-metal and fuzz material
pairs in addition to the existing toy/cloth/denim/rubber materials.

The offline contract is in [`assets/characters/catalog.json`](../assets/characters/catalog.json),
with the deterministic combination fixture in
[`assets/characters/phase5_fixture.json`](../assets/characters/phase5_fixture.json).
Run:

```sh
cargo test
cargo test --features dev-showcase
cargo run --features dev-showcase --bin validate_character_assets
```

This phase does not add network asset fetching, arbitrary mesh imports, cloth
simulation, or account persistence. Procedural material recipes deliberately
have zero external texture payload until the filtered authored textures are
ready; the validator still enforces the 8 MiB delivery ceiling and records
material/LOD/license metadata.
