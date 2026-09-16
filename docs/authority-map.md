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
| `move` with `authoritative: true` | server → clients | server-authoritative projection | The World applies finite-value, respawn, rate, and travel-envelope checks, canonicalizes the proposed position, then replicates it. It does not currently simulate Maze collision or prove that the player traversed the route. |
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

## Rust authority and Luau rules prototype

`cubacadabra_engine::authority::AuthorityBoundary` is the first implementation
of this seam. It owns a JSON state value, calls a game-owned
`CommandHandler::validate`, calls `simulate` only after validation, commits the
returned state atomically, and assigns monotonic event sequences. Its tests
model the gate action: an in-range `interact` request produces an accepted
state and `interaction_accepted` event, while an out-of-range request is
rejected without changing state or sequence. The client-facing `Command` has no
actor field; the trusted host supplies the authenticated actor separately when
calling `execute`. A bounded per-actor request cache makes retries within one
live boundary idempotent and rejects reuse of a request ID for a different
intent. Command, state, event-count, and result-size limits are enforced before
committing a transition.

The scripting layer now has a sandboxed Luau `CommandHandler` adapter. Its
rules module implements `validate_command(state, command)` and
`simulate_command(state, command)`, receives the actor ID only after the host
binds it to authenticated identity, and has an execution budget but no client
network, persistence, filesystem, or renderer APIs. The builder recognizes
optional `src/server.luau`, bundles it as a separate `authority.luau` entry,
and includes it in the package file hashes. This is an experimental artifact
contract; current clients and the Durable Object do not execute it.

Maze 101 now contains that first game-owned rules module. Tests run the actual
Luau source through the Rust adapter and cover per-player coin progress,
duplicate pickups, host-supplied positions, independent finish state, and
round timeout. The package builder optionally bundles `src/server.luau` as
`authority.luau` and includes the entry in the JSON manifest/package descriptor
and file hash list. This adds no binary prefab/scene format: authored world
descriptions remain JSON and runtime media remains separate package assets.
The client still uses its existing local script/network relay; these tested
rules are not yet connected to a live two-player session.

`cubacadabra_engine::server_runtime::ServerAuthority` now provides the portable
host adapter around the same boundary and Luau VM. It creates an initial
checkpoint from trusted state and processes each intent against a checkpoint
supplied by the host, returning the outcome with a candidate checkpoint that
contains the request receipt. This keeps failed storage writes from advancing
hidden in-memory game state. A wasm32 binding exposes that interface
synchronously to a JS host. This makes the execution interface available to
Cloudflare Workers and a future native headless host, but is not itself a
Durable Object integration: the host still needs a pinned package source,
authenticated command routing, atomic persistence, and trusted movement facts.

In particular, **do not use the current replicated `move` position as
anti-cheat evidence**. It is a bounded, server-canonicalized client proposal,
not a host-simulated Maze trajectory. Before production pickup/finish checks
can rely on proximity, the trusted authority needs its own validated movement
simulation/collision state (or an equally strong source of canonical position).
Likewise, request receipts currently live only in the authority snapshot
prototype, there is no atomic Durable Object persistence integration, profiles
and durable rewards are not implemented, and confirmed save/restart/rejoin
behavior remains unproven.

The backend must not be taught Maze World or Signal Run rules as a shortcut.
Integration should bind authenticated socket identity, use the package's
game-owned rules in a constrained trusted runtime, validate against trusted
world and movement context, persist round state and deduplication receipts
atomically, and only then publish accepted events. A profile reward needs its
own durable idempotency receipt and must not be described as saved until its
transaction has committed.
