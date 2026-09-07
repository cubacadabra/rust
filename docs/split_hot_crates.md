Yes—but there are **three different ideas mixed together**, and Rust treats them differently.

For Cubacadabra, I would **not** try to split the game into a bunch of separately built binaries and then “link the binaries together.” Instead:

1. Let Cargo/rustc incremental compilation do its job inside crates.
2. Split genuinely stable subsystems into **library crates** in a Cargo workspace.
3. For art iteration like hair, move tunable character data out of Rust entirely so changing hair often requires **zero compilation**.

That third one is probably the biggest win for what you're doing right now.

## What Cargo already does

Suppose you currently have one large crate:

```text
cubacadabra
├── renderer/
├── character/
├── physics/
├── world/
├── player/
└── ...
```

You edit:

```rust
hero_geometry.rs
```

Cargo does **not normally throw every compiled machine-code object away and start from zero**.

Rust has incremental compilation. `rustc` tracks dependencies internally and divides code generation into units, so unchanged work can often be reused.

That's why you see things like:

```text
Finished `dev` profile ... target(s) in 1.18s
```

Your last example is actually important: **the Rust build itself took only 1.18 seconds.**

Then:

```text
Running target/debug/magic_characters_capture ...
```

and generating 632 captures takes forever.

So for the thing you were complaining about a moment ago, **compilation isn't your main bottleneck at all.**

The rendering/capture workload is.

---

## But crate boundaries do give Cargo stronger caching boundaries

You can absolutely create a workspace like:

```text
rust/
├── Cargo.toml
├── crates/
│   ├── core/
│   ├── math/
│   ├── character/
│   ├── character-assets/
│   ├── renderer/
│   ├── physics/
│   ├── world/
│   └── gameplay/
└── apps/
    ├── game/
    ├── character-preview/
    └── capture/
```

And dependencies might look like:

```text
game
 ├── gameplay
 ├── world
 ├── physics
 └── renderer
       └── character
            └── character-assets
```

Cargo compiles these crates independently.

So if you modify only:

```text
character-assets
```

Cargo can reuse already-built:

```text
physics
world
math
...
```

That's a very real advantage.

But there's a catch.

If:

```text
renderer -> character-assets
```

and you change `character-assets`, then `renderer` is downstream of what changed. Cargo/rustc may need to revisit/recompile some of `renderer`, and ultimately your executable still has to be linked.

So breaking something into a crate does **not** mean:

> Compile hair.o and surgically plug it into an already completed game binary.

Rust doesn't generally operate like that at the Cargo level.

The final executable is still linked from compiled libraries/code.

---

# You don't normally link executable binaries together

This:

```text
hair binary
physics binary
renderer binary
game binary
```

isn't normally how you'd structure a game.

Instead:

```text
libhair.rlib
libphysics.rlib
librenderer.rlib
       ↓
    game executable
```

Cargo handles those libraries.

You *could* use dynamic libraries:

```text
hair.dylib
physics.dylib
renderer.dylib
```

and load them independently.

Then theoretically you could replace one without relinking everything.

But I would **not do that for Cubacadabra just for build speed**.

Rust's native Rust-to-Rust dynamic ABI isn't intended as a stable plugin ABI, and you introduce complexity around:

* ABI compatibility
* lifetime ownership across modules
* traits across boundaries
* allocator behavior
* version matching
* dynamic loader paths
* platform differences on macOS/iOS/Windows/Android/WASM

For a cross-platform game, you'd be buying yourself a lot of pain to save seconds that probably aren't your real bottleneck.

---

# Hair is a special case: it shouldn't necessarily be code

Here's where I think you can get something substantially better.

Right now you have things like:

```rust
const LOCK: &[[f32; 3]] = &[
    [0.0, 0.40, 0.42],
    ...
];
```

and probably:

```rust
hair_lock(
    Vec3::new(...),
    Vec3::new(...),
    ...
)
```

Every time someone tweaks the haircut:

```text
edit Rust
↓
cargo compile
↓
link
↓
launch
↓
capture
```

For active art development, I would move those parameters into an **external authoring file**.

For example:

```text
assets/characters/person/hair/casual_sweep.ron
```

Maybe:

```ron
HairStyle(
    cap: (
        front_lift: 0.40,
        contour: 0.040,
    ),

    locks: [
        (
            root: [-0.34, 0.36, -0.21],
            tip: [0.12, 0.06, -0.47],
            width: 0.25,
            depth: 0.18,
            bend: 0.45,
        ),
        ...
    ],
)
```

Your preview program loads it at runtime.

Then the workflow becomes:

```text
edit hair file
↓
run preview
```

or even:

```text
edit hair file
↓
preview automatically reloads
```

**No Rust compilation whatsoever.**

That's the architecture I would want eventually for:

* hairstyles
* face proportions
* clothing profiles
* shoe dimensions
* material parameters
* color palettes
* accessory placement
* perhaps animation tuning parameters

Not gameplay algorithms—but **art data**.

---

# You could go one step further: hot reload

This would be fantastic for Cubacadabra.

Run:

```bash
cargo run --bin character_preview
```

once.

It opens a window.

Then you modify:

```text
casual_sweep.ron
```

The running preview watches the file.

It detects:

```text
assets/characters/person/hair/casual_sweep.ron changed
```

and reconstructs the character.

Instantaneously.

Now your iteration loop is:

```text
change 0.40 → 0.34
save
look at character
```

rather than:

```text
change
cargo run
wait
open PNGs
change
cargo run
...
```

That will matter much more than shaving 500 ms off incremental compilation.

You could even have hotkeys:

```text
1 front
2 3/4
3 side
4 back
5 gameplay distance

W wave
I idle
R run
```

and a screenshot key.

That becomes your little **Cubacadabra character studio**.

For the art work you're doing right now, this would be incredibly useful.

---

# I'd still split the Rust project into logical crates

There is a good architectural reason to do this, independently of hot reload.

Maybe eventually:

```text
cubacadabra-math
cubacadabra-platform
cubacadabra-assets
cubacadabra-character
cubacadabra-animation
cubacadabra-render
cubacadabra-physics
cubacadabra-world
cubacadabra-game
```

with tools:

```text
cubacadabra-character-preview
cubacadabra-capture
cubacadabra-game-client
```

But **don't go nuts**.

Rust projects sometimes get over-crated:

```text
cubacadabra-hair
cubacadabra-shoes
cubacadabra-hands
cubacadabra-eyes
```

That's not what I'd recommend.

Every crate boundary has costs:

* compilation metadata
* dependency management
* public API boundaries
* harder refactors
* less compiler visibility across modules
* potentially more monomorphization issues
* more complex project navigation

Use crates for architectural boundaries, not individual files.

Something like **character/rendering/assets** is reasonable.

**Hair as its own Rust crate is not.**

---

# There is also a very useful separate preview binary

You've already started doing this with:

```text
magic_characters_capture
```

I would lean further into that.

Instead of starting the whole Cubacadabra game to work on hair:

```text
cubacadabra-game
```

have:

```text
character-preview
```

depend on only:

```text
character
renderer
assets
wgpu
```

and not:

```text
networking
world simulation
server protocol
game UI
physics gameplay
...
```

Then its dependency graph is smaller.

That means both builds and startup are cheaper.

Something like:

```text
                cubacadabra-character
                        │
                        v
                  cubacadabra-render
                     /       \
                    /         \
      character-preview      game
```

That's a very normal and healthy Rust architecture.

---

# For your current situation, though, look at the numbers

You had:

```text
Finished `dev` ... in 1.18s
```

and then waited ages for:

```text
wrote 632 capture(s)
```

So imagine we completely perfected your Rust crate architecture and got:

```text
1.18 sec
```

down to:

```text
0.4 sec
```

You'd barely notice.

Whereas the change we discussed:

```text
632 captures
        ↓
8 captures
```

could turn a minutes-long iteration into seconds.

And runtime-loaded hair parameters could turn:

```text
compile + start + capture
```

into:

```text
save → instant update
```

That's where I'd invest.

---

## The architecture I'd aim for

Longer term, something like:

```text
                         ┌───────────────────────┐
                         │      game client      │
                         └───────────┬───────────┘
                                     │
             ┌───────────────────────┼─────────────────────┐
             │                       │                     │
             ▼                       ▼                     ▼
          gameplay                 world                renderer
                                                            │
                                                            ▼
                                                        character
                                                            │
                                                            ▼
                                                          assets
                                                            │
                                      ┌─────────────────────┴───────┐
                                      │                             │
                                      ▼                             ▼
                              character preview                 capture tool
```

And the character asset side becomes:

```text
Rust code
--------
how a HairLock works
how meshes are generated
how a hand attaches
how skinning works
how materials render

Data
----
this hairstyle's roots
this hairstyle's tips
this hoodie profile
this shoe dimensions
this character's colors
this expression settings
```

That's a really useful separation.

### The rule I'd use

**If changing something requires engineering knowledge, keep it in Rust.**

**If an artist/model is expected to repeatedly tweak numbers until it looks right, strongly consider making it runtime-loaded data.**

Your current hair work has crossed firmly into the second category.

So yes, Cargo is already doing much of what you're imagining for compiled code. **Crates can make the boundaries stronger and the architecture cleaner, but the really dramatic improvement for Cubacadabra's character iteration would be to stop compiling artistic parameters at all.**

