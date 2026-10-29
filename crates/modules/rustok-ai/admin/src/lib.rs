//! # RusToK AI Admin
//!
//! Leptos administration and operator UI package for the `rustok-ai` capability.
//!
//! Provides the AI control plane, provider profiles, interactive sessions, tool policies,
//! and diagnostic observability interfaces for administrator consoles.

mod core;
mod i18n;
mod model;
pub mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::AiAdmin;
