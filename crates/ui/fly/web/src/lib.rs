//! Framework-neutral browser runtime, geometry, hit-testing, and iframe bridge for Fly visual editors.
//!
//! This crate owns browser geometry, pointer interactions, drop target resolution, iframe protocol
//! envelopes, event listeners, and authenticated real-DOM inline editing.
//! It is completely independent of any specific UI framework (zero Leptos or Dioxus dependencies).

// No hand-written unsafe in this crate. `deny` rather than `forbid` because framework
// proc-macros (`#[component]`, wasm-bindgen glue) may expand to generated unsafe guarded by their
// own `#[allow(unsafe_code)]`, which `forbid` would reject.
#![deny(unsafe_code)]

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
