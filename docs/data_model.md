No — I’m **not** saying “we’ll soon move Signal Run from Luau into Rust.” I’m saying we’ll soon add a **Rust foundation underneath it**.

Think of it like React itself versus your React application.

React knows generic things like:

> “A piece of state changed. Notify whoever depends on it. Re-render what needs re-rendering.”

React does **not** know:

> “This shopping cart has three items and the user just applied a coupon.”

Your application owns that meaning.

Cubacadabra should have the same split.

In Signal Run, this belongs in **Luau**:

```lua
gate2.capturedBy = playerId
score.red = score.red + 1
```

Those are **rules of Signal Run**. Cubacadabra's engine shouldn't even know that a “gate” can be “captured.”

But suppose capturing the gate makes a physical beacon turn red, disables its collision, plays an animation, and moves a flag. Eventually Luau might do something conceptually like:

```lua
gate:Set("team", "red")
```

And underneath that call, **Rust's generic data model** handles:

```text
Entity 182
  Light.Color = red
  Collider.Enabled = false
  Flag.Transform.Position = ...
```

Then Rust can automatically tell:

* renderer
* physics
* networking
* save/load
* Studio inspector
* undo/redo
* change listeners

without Signal Run manually notifying each subsystem.

So the architecture becomes:

```text
                SIGNAL RUN LUAU
                     │
           "Red team captured gate 2"
                     │
                     ▼
              game reducer/state
                     │
       decides what should change visually
                     │
                     ▼
       GENERIC RUST DATA MODEL / MUTATIONS
                     │
        ┌────────────┼──────────────┐
        ▼            ▼              ▼
     renderer      physics       networking
        ▼                           ▼
      Studio                     save/load
```

### Why moving Signal Run itself into Rust would be bad

Imagine you make **100 Cubacadabra games**.

One has gates.

One has unicorn magic spells.

One has farming.

One has a pizza restaurant.

One has racing.

You absolutely do **not** want your Rust engine accumulating:

```rust
capture_gate()
cast_spell()
harvest_carrot()
cook_pizza()
finish_race()
```

That's Roblox's distinction too. The engine supplies things such as Parts, transforms, physics, events, networking and properties. The game developer supplies:

```lua
if playerTouchesFinishLine then
    player.score += 1
end
```

That is exactly where Luau shines.

And there's a huge practical benefit for Cubacadabra: **a creator can change their game without us recompiling and shipping a new Rust engine.**

### So what happens to the work you just did?

Mostly, **nothing**. It stays.

There may eventually be a little plumbing underneath it that changes.

Today it might conceptually be:

```text
Luau game state
    ↓
Luau notices state changed
    ↓
Luau explicitly updates effects/UI/etc.
```

Eventually it can become:

```text
Luau game state
    ↓
Luau expresses generic world mutations
    ↓
Rust data model
    ↓
everything updates automatically
```

That's an evolution, not a rewrite.

There is one nuance: if your `CubaSharedState` abstraction keeps becoming more and more generic—used identically by every game for replication, snapshots, subscriptions, conflict resolution, etc.—**some of its lowest-level machinery might eventually deserve to live in Rust**. But even then, its API would still be exposed to Luau.

For example, we might eventually replace a bunch of Luau implementation code with:

```lua
local state = Cuba.State.new(...)
state:dispatch({
    type = "captureGate",
    gate = 2
})
```

where Rust provides the robust state container underneath.

But this:

```lua
function reducer(state, action)
    if action.type == "captureGate" then
        ...
    end
end
```

should still be **the game's Luau code**.

So the clean rule I'd use is:

> **Rust owns nouns that every Cubacadabra game understands. Luau owns nouns and verbs invented by a particular game.**

Rust understands **Entity, Transform, Mesh, Collider, Sound, Property, StateChange**.

Signal Run understands **Gate, Capture, Team, Score, Round**.

That's why I would absolutely **not undo those commits**. What you built is on the correct side of the boundary. The next Rust work should make that Luau code more powerful and automatic underneath, not absorb it.

## Rust foundation now in place

The first engine increment lives in `src/data_model.rs` and is intentionally
generic. `DataModel` owns a stable `EntityId` graph rooted at `game`; entities
have a class, name, parent, and JSON-compatible property bag. The only mutation
paths are:

- `create_entity`
- `set_name`
- `set_parent`
- `set_property` / `remove_property`
- `destroy`

Every successful mutation emits a `DataModelChange` with a monotonic sequence,
the `MutationSource`, and a typed `DataModelEvent`. Setting an equal property,
keeping the current name/parent, or removing a missing property is a no-op and
emits nothing. Destroy events carry pre-despawn snapshots so networking, undo,
and persistence consumers can still inspect the removed object.

Consumers call `subscribe` or `subscribe_from_start`, then independently drain
the feed with `changes_since`. The retained history is bounded; a stalled
consumer receives an explicit cursor-too-old error instead of silently missing
mutations. `Engine::data_model()` and `Engine::data_model_mut()` expose this
resource to trusted engine integrations while keeping game-specific nouns in
Luau.

This is the substrate, not yet the Luau `Instance` API or renderer/physics
bridge. Those should be layered on this mutation path rather than creating
parallel setters or per-frame scans.
