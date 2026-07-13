#[path = "lib.rs"]
mod foundation;

pub use foundation::*;

#[cfg(target_arch = "wasm32")]
mod browser_interaction;
#[cfg(target_arch = "wasm32")]
pub use browser_interaction::*;

#[cfg(target_arch = "wasm32")]
mod browser_runtime;
#[cfg(target_arch = "wasm32")]
pub use browser_runtime::*;
