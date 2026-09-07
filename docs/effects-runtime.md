# Game-owned world effects

Rust renders, bounds, and advances effects, while each game owns their visual
composition. An interaction without a `visual` uses a neutral ring. Setting
`"visual": "none"` makes the interaction invisible; any other value names a
template from the package-level effect library.

```json
{
  "effects": {
    "version": 1,
    "templates": {
      "checkpoint": {
        "duration": 1.2,
        "nodes": [{
          "shape": "ring",
          "position": [0, 0.2, 0],
          "size": [2.5, 0.1, 1],
          "color": "$interaction",
          "visibleStates": ["default", "open"],
          "animation": {
            "pulseAmount": 0.06,
            "pulseSpeed": 2.4
          }
        }]
      }
    }
  },
  "worlds": {
    "arena": {
      "interactions": [{
        "id": "checkpoint-a",
        "position": [4, 0, -8],
        "radius": 2.5,
        "color": "accent",
        "visual": "checkpoint"
      }]
    }
  }
}
```

Version 1 supports `box`, `cylinder`, `ring`, and `sphere` nodes. `box` uses
all three `size` values. A cylinder uses X as radius and Y as height, a ring
uses X as radius and Y as band width, and a sphere uses X as radius. Colors are
palette keys or hex colors; `$interaction` inherits the interaction color.

Animation properties are composable. `orbitRadius`, `orbitSpeed`, `bobAmount`,
`bobSpeed`, `pulseAmount`, `pulseSpeed`, and `spinSpeed` describe continuous
motion. Speeds are radians per second. `expandAmount`, `radialAmount`, and
`fade` use normalized progress when the template is played as a one-shot;
`radialAmount` moves copies away from or toward the origin. `count` creates
evenly phased copies of a node.

Luau changes an attached interaction's visual state or plays a template at a
world position:

```luau
api.effects:set_state("checkpoint-a", "open")
api.effects:play("finish-flash", { position = { 4, 1, -8 } })
```

`visibleStates` controls which nodes appear for the current state. An empty
list means the node is always visible. One-shot lifetime comes from the
template's `duration`; attached interaction templates continue until their
state or world changes.

When several states reuse the same geometry, put the shared properties on one
node and author compact `variants`:

```json
{
  "shape": "ring",
  "position": [0, 0.2, 0],
  "size": [3, 0.1, 1],
  "color": "$interaction",
  "variants": [
    {
      "visibleStates": ["default", "locked"],
      "opacity": 0.2
    },
    {
      "visibleStates": ["active"],
      "opacity": 1,
      "animation": {"pulseAmount": 0.1, "pulseSpeed": 3}
    },
    {
      "visibleStates": ["complete"],
      "opacity": 0.6
    }
  ]
}
```

Each variant inherits `position`, `size`, `color`, `opacity`, `count`,
and individual animation properties from the node, then overrides only the
fields it declares. A node uses either its original `visibleStates` or its
`variants`; when variants are present, the node-level list is ignored. This
is authoring shorthand for the same bounded render nodes, not a second effects
runtime.

The runtime accepts at most 64 templates, 32 resolved nodes per template, 16
variants and 16 copies per authored node, 64 queued script commands, and 64
recent one-shot instances. Names are 1–64 ASCII letters, numbers, dots, dashes,
or underscores. Numeric inputs are clamped to finite rendering limits.
Reduced-effects mode substantially lowers continuous movement without changing
gameplay or script state.
