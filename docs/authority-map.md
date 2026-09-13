# Multiplayer authority map

This is the current protocol map as implemented by the Rust client and the
Cloudflare `World` Durable Object. “Server-validated” means the server checks
or canonicalizes an input. “Server-authoritative” means clients consume the
server-produced result as the source of truth. CAS gives a retained channel a
single ordered writer; it does not validate the meaning of the payload.

## Transport and presence

| Message or mutation | Direction | Current classification | What that means |
| --- | --- | --- | --- |
| `session_identity` | server → client | server-authoritative | The World assigns the socket’s player identity. |
| `player_join`, `player_leave` | server → client | server-authoritative | Presence and the peer list come from the World. |
| `set_username` | client → server | server-validated | The request is normalized before it is stored and broadcast. |
| `player_name` | server → client | server-authoritative | Clients consume the accepted name from the server. |
| `set_appearance` | client → server | server-validated | The server validates revisions and appearance shape before persisting. |
| `appearance`, `appearance_updated` | server → client | server-authoritative | Clients consume the accepted appearance from the server. |
| `set_hidden` | client → server | server-validated | The World controls visibility and the corresponding presence broadcast. |
| connection, world routing, reconnect generation | host ↔ server | server-authoritative | Hosts request a route; the World owns socket membership and generations. |

## Movement and experience protocol

| Message or mutation | Direction | Current classification | What that means |
| --- | --- | --- | --- |
| local input, camera, local collision, local animation | client-local | client-local | These are presentation/input concerns and are not sent as state claims. |
| local player position, health, checkpoints, interaction enter/exit | client simulation | client-predicted | The Rust client simulates these for responsiveness. They are not proof of what happened in a competitive world. |
| `move` sent by the client | client → server | client-predicted | It is a movement proposal containing the client’s current simulation result. |
| `move` with `authoritative: true` | server → clients | server-authoritative | The World applies finite-value, respawn, rate, and travel-envelope checks, canonicalizes the position, then replicates it. |
| `experience_state`, `experience_launch` | server → client | server-authoritative | Build state, lobby occupancy, launch timing, and the selected session are produced by the Durable Object. |
| `build_action` / `build_save` | client → server | server-validated | The backend validates bounds, shape, color, block count, and target existence before broadcasting the next build state. |

## Game-owned network lanes

| Message or mutation | Direction | Current classification | What that means |
| --- | --- | --- | --- |
| `game_message` | both directions | client-local | The World validates only channel and JSON size, then broadcasts the opaque payload. |
| `game_state_set` | client → server | client-predicted | The client proposes the retained value; the World orders and stores it, but does not validate its meaning. |
| `game_state_compare_set` | client → server | server-validated | The World accepts only a matching sequence, then stores the still client-authored payload. |
| `game_state` | server → client | server-authoritative | Sequence, timestamps, and conflict results are server-produced; fields such as `captured`, `score`, or `health` remain untrusted claims. |
| `player_state` on `__player_state` | both directions | client-predicted | The backend strips identity fields and relays the latest live state; it explicitly marks the result `authoritative: false`. |
| `error`, `appearance_error` | server → client | server-authoritative | The backend reports malformed transport, sequencing, appearance, or reserved-channel requests. |

## The first authority candidate: Signal Run gate interaction

Signal Run is a useful small test because its current path is easy to point at:

```text
local zone enter
  → second-game.on_interaction(api, { id = "node-1" })
  → relay:request_interaction(api, id)
  → shared:dispatch({ type = "capture", node = 1 })
  → compare_set_state({ nextNode = 2, ... })
```

The client currently decides that the player captured the next node. The CAS
retry loop prevents two honest clients from losing each other’s update, but a
modified client can still dispatch `capture` without being near `node-1`.

The eventual request should instead be:

```text
client: Command { name = "interact", payload = { target = "node-1" } }
  → trusted world validates actor identity, canonical position, target, and radius
  → trusted world simulates the generic interaction
  → Event { name = "interaction_accepted", payload = { target = "node-1" } }
  → Signal Run turns that accepted event into its game-owned capture transition
```

The generic layer must not know that `node-1` advances `nextNode`, changes the
relay phase, plays a sound, or awards a link. Those remain Signal Run rules.
The trusted validator only establishes that an actor legitimately interacted
with an engine interaction target.

## Rust prototype

`cubacadabra_engine::authority::AuthorityBoundary` is the first implementation
of this seam. It owns a JSON state value, calls a game-owned
`CommandHandler::validate`, calls `simulate` only after validation, commits the
returned state atomically, and assigns monotonic event sequences. Its tests
model the gate action: an in-range `interact` request produces an accepted
state and `interaction_accepted` event, while an out-of-range request is
rejected without changing state or sequence.

The prototype is deliberately not wired to `World`, WebSockets, or
`second-game`. The current backend has no command handler registry and must not
be taught Signal Run rules as part of this step. A future integration can carry
the same generic envelope through a server-owned rules runtime and send the
resulting event/state back over a new protocol version.
