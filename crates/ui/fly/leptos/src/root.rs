//! Leptos adapter crate root.
//!
//! `Cargo.toml` declares `[lib] path = "src/root.rs"`, so this file is the crate root and
//! [`foundation`] is an ordinary module. It used to be reached through
//! `#[path = "lib.rs"] mod foundation;`, which made a file named `lib.rs` something other than the
//! crate root — an IDE and onboarding trap with no compensating benefit.

// No hand-written unsafe in this crate. `deny` rather than `forbid` because framework proc-macros
// (`#[component]`, wasm-bindgen glue) may expand to generated unsafe guarded by their own
// `#[allow(unsafe_code)]`, which `forbid` would reject.
//
// Crate-level, and deliberately not on `foundation` alone: while `foundation` was included through
// `#[path]`, the attribute covered that module and nothing else, so the feature-gated browser
// modules below were outside the lint the comment claimed.
#![deny(unsafe_code)]

mod foundation;

pub use foundation::*;

// The browser surface is re-exported through explicit, feature-gated modules rather than left to
// the `fly_web::*` glob in `foundation`, so the SSR-first contract is visible in this file: the
// modules below exist only when the caller opts into `wasm-client`. `scripts/verify/
// verify-fly-ssr-first.mjs` asserts exactly that shape.
#[allow(unused_imports)]
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_interaction;
#[allow(unused_imports)]
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_interaction::*;

#[allow(unused_imports)]
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_runtime;
#[allow(unused_imports)]
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_runtime::*;
