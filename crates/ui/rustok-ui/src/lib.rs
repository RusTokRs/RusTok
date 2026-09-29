/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Framework-agnostic design system primitives, variants, contracts, and styling resolvers.
//!
//! The crate is wrapped by the framework adapters `rustok-ui-leptos` (Leptos 0.8)
//! and `rustok-ui-dioxus` (Dioxus 0.6). Hosts that only need deterministic class
//! strings — for example a server-side renderer that emits HTML itself — can
//! depend on this crate directly.
//!
//! ```
//! use rustok_ui::{ButtonVariant, Size, button_classes};
//!
//! let classes = button_classes(ButtonVariant::Default, Size::Md, None);
//! assert!(classes.contains("bg-primary"));
//! ```
//!
//! Component *state* (disabled, loading, invalid, checked) is expressed by the
//! adapters through native HTML attributes (`disabled`, `aria-invalid`, …) and
//! the matching `disabled:`/`aria-` Tailwind variants, which is why the class
//! resolvers only take the static variant/size configuration plus optional
//! caller classes.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod classes;
pub mod contracts;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod tests;

pub use classes::*;
pub use contracts::*;
pub use tokens::*;
pub use types::*;
