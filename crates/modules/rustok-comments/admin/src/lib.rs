//! # RusToK Comments Admin
//!
//! Leptos admin UI package for the `rustok-comments` module.
//! Provides moderation workspace, comment status management, and native server transport adapters.

mod core;
mod i18n;
mod transport;
mod ui;

pub use ui::leptos::CommentsAdmin;

#[cfg(test)]
mod contract_tests;
