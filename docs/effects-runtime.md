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

The runtime accepts at most 64 templates, 32 nodes per template, 16 copies per
node, 64 queued script commands, and 64 recent one-shot instances. Names are
1–64 ASCII letters, numbers, dots, dashes, or underscores. Numeric inputs are
clamped to finite rendering limits. Reduced-effects mode substantially lowers
continuous movement without changing gameplay or script state.
