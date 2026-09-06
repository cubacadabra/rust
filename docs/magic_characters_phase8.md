# Magic Characters Phase 8: rollout and art-language expansion

Implemented September 5, 2026.

## Renderer rollout boundary

The renderer now has a presentation-only `CharacterRenderMode` with two
values:

- `Magic` (`1`, default): the production indexed mesh, instanced batch,
  character-material and effect passes;
- `Legacy` (`0`): the hard-cuboid avatar with legacy package colors and the
  existing typed motion inputs.

The switch does not touch simulation, collision, identity, package parsing,
appearance revisions, or the public eight-float snapshot. It can be selected
before the first sync, and can be changed at runtime for staged comparison or
incident rollback. Native hosts use
`engine_renderer_set_appearance_mode`; browser hosts use
`WebRenderer.set_appearance_mode`. Invalid mode values are rejected without
changing the current mode.

`Magic` remains the default in this checkout because it is the current
production renderer. A client rollout may hold `Legacy` until its preceding
platform gates pass, then enable `Magic` by configuration. The legacy color
migration remains in place independently, so a visual rollback does not lose a
player's resolved identity.

## Executable comparison evidence

Generate the deterministic compatibility suite with:

```sh
cargo run --features dev-showcase --bin magic_characters_capture -- \
  --phase 8 --output docs/baselines/magic-characters/phase8
```

This writes `phase8_report.json` plus separate `legacy/`, `magic/`, and
`wardrobe/` capture folders. The first two cover ordinary gameplay poses,
first/third-person camera, raised support, 18/50 crowds and portrait
letterboxing. The wardrobe folder exercises the six outfit silhouettes using
the same indexed/instanced CharacterRenderer used by live magic draws. The
50-character scenes are render-only and do not change the engine's 18-player
capacity.

Before changing a host default, review both mode folders on the host's actual
Metal/WebGPU/GL/GLES path and record the compatibility window owner. Physical
mobile sustained performance and two-client production verification remain
platform integration gates.

## Official asset expansion

The final [`Soft Cubism style guide`](magic_characters_style_guide.md) ratifies
the proportions, clearances, materials, face language, outfit fit rules,
review matrix and camera/collision boundary. The bundled
[`soft_cubism_examples.json`](../assets/characters/soft_cubism_examples.json)
proves the language transfers to a squircle-foliage tree, rounded toy-cart
panel and star badge accessory. It is validated by the development asset
validator:

```sh
cargo run --features dev-showcase --bin validate_character_assets -- \
  --style-examples assets/characters/soft_cubism_examples.json
```

The CPU-expanded magic shape helpers remain available only to the opt-in
historical shape/capture fixtures. No live renderer draw uses them; production
magic draws use immutable indexed meshes and per-frame instance parameters.
They can be removed with the old capture fixtures after the compatibility
window is formally retired.
