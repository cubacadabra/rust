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

