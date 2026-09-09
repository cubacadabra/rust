mod protocol;
mod session;

#[cfg(not(target_arch = "wasm32"))]
mod ffi;
#[cfg(target_arch = "wasm32")]
mod web;

pub use cubacadabra_engine::Engine;

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub use cubacadabra_engine::native;

pub use protocol::{ClientAction, ClientMovement};
pub use session::{ClientError, ClientSession};

#[cfg(all(target_arch = "wasm32", feature = "web-renderer"))]
pub use cubacadabra_engine::WebRenderer;
#[cfg(target_arch = "wasm32")]
pub use web::WebClient;
