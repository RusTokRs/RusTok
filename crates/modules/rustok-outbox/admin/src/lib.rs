//! # rustok-outbox-admin
//!
//! Leptos admin UI package for the `rustok-outbox` module.
//!
//! Provides the administrative interface for inspecting outbox relay health,
//! relay counters, and transactional outbox runtime notes.

pub mod core;
mod i18n;
mod transport;
pub mod ui;

pub use ui::leptos::OutboxAdmin;

#[cfg(test)]
mod contract_tests;
