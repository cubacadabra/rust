# Game-owned network messages

The runtime exposes a small transport seam for game packages. It does not
interpret the channel name or payload, and it must not grow types for a game's
theme or rules.

```luau
api.network:publish("round-events", { kind = "sparkle" })
api.network:set_state("round-progress", { score = 4 })
api.network:compare_set_state("shared-round", 7, { round = 3, score = 5 })

function Game.on_network_message(api, message)
    -- message.type is "game_message" or "game_state"
    -- message.channel and message.payload belong to the game
end
```

`publish` is an ephemeral broadcast to everyone currently in the same world.
`set_state` replaces the latest JSON value for a channel, broadcasts it, and
causes it to be sent to players who join that world later. The backend stores
the latest retained value in the world instance but does not validate its
meaning.

`compare_set_state` is the race-safe retained-state primitive. Its second
argument is the last server sequence observed by the script. The Durable Object
accepts the new opaque payload only when that sequence still matches, increments
the sequence, persists the value, and broadcasts the resulting `game_state`.
On a conflict, the sender receives the current `game_state` with
`conflict = true`; the game can merge its still-pending intent and retry. Once a
channel has been written with compare-and-set, legacy `set_state` writes cannot
overwrite it.

State messages also include `updatedAt` and `ageMs`. Games should use `ageMs`
to resume short timers after reconnecting rather than trusting a client wall
clock. A newly connected player receives every retained channel, not only
players reconnecting on an existing socket identity.

Game packages built with the shared tools can opt into the versioned
`@cubacadabra/shared-state-v1.luau` helper. It layers bounded intent queuing,
conflict rebasing, retries, and reconnect handling over `compare_set_state`
without changing this transport contract. The helper and its reducer API are
documented in
[the shared-state SDK v1 guide](../../tools/docs/shared-state-v1.md).

The current v1 contract is cooperative rather than competitive. `updatedAt`
and `ageMs` are server-derived, and live broadcasts carry a transient
`senderId`, but clients still propose both intents and resulting payloads.
These fields are useful for synchronized feedback and short timers; they do
not prove that a player legitimately completed a race or earned a score.
Cheat-resistant leaderboards require a future generic server-rules boundary,
not game-named handlers or client-authored timestamps. Competitive authority
is intentionally outside the cooperative MVP API.

The host adapts the runtime outbox to these WebSocket messages:

```json
{"type":"game_message","channel":"round-events","payload":{}}
{"type":"game_state_set","channel":"round-progress","payload":{"score":4}}
{"type":"game_state_compare_set","channel":"shared-round","expectedSequence":7,"payload":{"round":3,"score":5}}
```

Payloads are JSON values, channels are 1–64 UTF-8 bytes, and a complete message
is limited to 64 KiB. Compare-and-set makes the Durable Object authoritative
for ordering and prevents lost updates, while payload meaning remains entirely
game-owned. It is still a cooperative MVP contract: untrusted competitive games
will eventually need a generic sandboxed server-rules mechanism rather than
putting game names or rules into the platform backend.
