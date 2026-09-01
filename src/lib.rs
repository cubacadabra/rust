mod engine;
mod ffi;
mod math;
mod npc;
mod player;
#[cfg(any(not(target_arch = "wasm32"), feature = "web-renderer"))]
mod renderer;
mod scripting;
mod types;
#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
mod web_renderer;
mod world;

pub use engine::Engine;
