# Bundled character assets

These files describe supported IDs, fits, provenance and validation fixtures.
They do not lock character proportions or visual direction. Follow
[Character art direction](../../docs/character_art_direction.md).

The catalog retains person, cat and dragon and six existing outfits for
compatibility. Expansion is paused while one casual person is reviewed.
Runtime recipes live in `src/character/definition.rs`; the person hoodie fit
uses `src/renderer/hero_character.rs` and authored mesh profiles.

```sh
cargo run --features dev-showcase --bin validate_character_assets
cargo run --features dev-showcase --bin validate_character_assets -- \
  --style-examples assets/characters/soft_cubism_examples.json
```

The validator checks IDs, declared fits, coverage/material/LOD metadata,
provenance and payload bounds. Unsupported combinations fall back atomically.
Stored `soft-cubism.v1` examples and proportion fixtures remain compatibility
data; their toy terminology and joint clearances are not art requirements.
No external texture payload is currently bundled.

See [runtime and verification](../../docs/character_runtime.md) for capture and
platform checks. Save bulk captures outside docs.
