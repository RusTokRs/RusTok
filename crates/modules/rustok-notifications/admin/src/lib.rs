//! # rustok-notifications-admin
//!
//! Leptos admin UI package for the `rustok-notifications` module.
//!
//! Exposes notifications operations, source registry status, and delivery
//! overview components for platform administrators.

pub mod core;
mod i18n;
mod transport;
pub mod ui;

pub use ui::leptos::NotificationsAdmin;

#[cfg(test)]
mod contract_tests;
