#[path = "lib.rs"]
mod foundation;

pub use foundation::*;

mod real_dom_inline;
pub use real_dom_inline::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_interaction;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_interaction::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_runtime;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_runtime::*;
