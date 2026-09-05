# Magic Characters Phase 7: persistent local and remote identity

Implemented September 5, 2026 for the Rust engine boundary.

Phase 7 keeps identity separate from the legacy eight-float snapshot. Local
appearance is resolved once into the bounded `CharacterAppearance` model and
survives world changes. A local change is atomic and accepted only when its
revision is not older than the current revision. Invalid or unsupported fields
resolve to bundled person/hoodie defaults while preserving valid legacy colors;
the status API distinguishes applied, fallback, stale, duplicate and invalid
input.

## Remote contract

The additive remote message is UTF-8 JSON, version `1`, and is bounded to
64 KiB and the existing 17 remote slots. Hosts send stable opaque IDs,
generation, packet sequence, optional per-player motion sequence, appearance
revision and optional motion support/velocity/emote data. A representative
message is:

```json
{
  "version": 1,
  "sequence": 42,
  "worldId": "lobby",
  "players": [{
    "id": "account:alice",
    "generation": 7,
    "position": [1, 0, 2],
    "yaw": 0.2,
    "lookYaw": 0.1,
    "planarVelocity": [0, -6.4],
    "verticalVelocity": 0,
    "grounded": true,
    "supportHeight": 0,
    "moving": true,
    "sprinting": false,
    "motionSequence": 99,
    "emote": "wave",
    "emoteSequence": 4,
    "appearance": {
      "version": 1,
      "body": "cuba:cat.v1",
      "outfit": "cuba:everyday-hoodie.v1",
      "revision": 12
    }
  }]
}
```

Packet sequences reject late or duplicate batches. Per-player motion sequences
reject older motion while still allowing a newer packet to carry a newer
appearance. Appearance revisions are monotonic per stable ID. The renderer
keys presentation state by stable-ID hash plus generation, so roster reorder
does not move an outfit or animation state to another player; generation
changes reset motion lifetime. Emotes are edge-triggered by their own sequence
and are not replayed by retransmission.

`engine_reset_remote_session` starts a new packet lifetime but retains a bounded
17-entry appearance cache. This allows a reconnect or late join to omit
unchanged appearance content without losing the resolved outfit, while the new
generation still starts a fresh motion/presentation lifetime. World IDs hide a
roster from the wrong world without discarding its resolved identity.

## Host contracts

Native hosts can use the engine-owned appearance and remote-update buffers, or
the borrowed JSON convenience functions. Buffers are valid until their next
allocation or engine destruction; oversized allocations return null and
invalid UTF-8/JSON is rejected. `include/cubacadabra_engine.h` documents the
limits and status values. Existing remote setters, snapshot pointer, length,
stride and suffix meanings are unchanged.

Browser hosts receive JavaScript helpers that write the same bounded
engine-owned buffers for local appearance and remote updates, plus reconnect
reset support. They may continue using the existing engine pointer/snapshot
contract. No mesh, GPU handle, blink state or account credential crosses this
boundary.

## Verification

Rust tests cover:

- atomic local appearance changes and monotonic revisions;
- stable identity and appearance through remote roster reorder;
- stale packet rejection and emote deduplication;
- reconnect hydration from the bounded appearance cache with a new generation;
- legacy setter/snapshot compatibility fixtures.

The available companion paths now persist the package default in `first-game`,
hydrate the browser engine from package or local storage, replay appearance on
socket reconnect, and carry stable IDs, generations, motion sequences and
appearance revisions through the backend world Durable Object. The iOS bridge
also exposes the additive appearance and remote-update APIs while retaining its
legacy setter fallback; Android remains a legacy-compatible client until its
host adopts the richer JSON contract. The remaining gate is physical two-client
production verification across those hosts, including entitlement/catalog
policy and missing-content behavior.
