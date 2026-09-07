# Game-owned network messages

The runtime exposes a small transport seam for game packages. It does not
interpret the channel name or payload, and it must not grow types for a game's
theme or rules.

```luau
api.network:publish("round-events", { kind = "sparkle" })
api.network:set_state("round-progress", { score = 4 })

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

The host adapts the runtime outbox to these WebSocket messages:

```json
{"type":"game_message","channel":"round-events","payload":{}}
{"type":"game_state_set","channel":"round-progress","payload":{"score":4}}
```

Payloads are JSON values, channels are 1–64 bytes, and a complete message is
limited to 64 KiB. Treat this as a cooperative MVP contract: clients can send
game messages, so competitive games will eventually need an authoritative
server-side rules module or service.
