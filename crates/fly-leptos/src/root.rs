#[path = "lib.rs"]
mod foundation;

pub use foundation::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_interaction;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_interaction::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_runtime;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_runtime::*;
