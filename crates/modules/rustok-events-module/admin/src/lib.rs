//! # RusToK Events Admin
//!
//! Leptos admin UI package for the `rustok-events` module.
//! Provides event monitoring, delivery profile configuration, and transport status views.

mod core;
pub mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::EventsAdmin;

#[cfg(test)]
mod contract_tests;
