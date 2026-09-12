# Label maker: one engine property description

This is the first small slice of a shared engine data model. It describes the
generic `InteractionZone` class once in Rust, including stable property IDs,
display names, value types, defaults, validation ranges, and runtime flags.

The same metadata currently has two consumers:

- `engine/interactions.rs` sanitizes manifest-authored zone values through the
  registry before simulation and rendering use them.
- `scripting.rs` exposes the registry through
  `api.interactions:get_schema("cuba:interaction-zone.v1")`, so tools and game
  scripts can inspect the same property names and types.

`second-game` demonstrates the seam in its HUD. Its four relay nodes and uplink
all travel through the same `InteractionZone` path, while the HUD reads the
shared `Label` and `Radius` metadata to show `ENGINE InteractionZone`.

This is deliberately not a general reflection rewrite. The next property or
class should only be added after identifying a second real duplicated contract.
