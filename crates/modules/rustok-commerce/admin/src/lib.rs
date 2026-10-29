//! # RusToK Commerce Admin
//!
//! Leptos admin UI package for the `rustok-commerce` module.
//! Provides shipping profile registry, cart promotion orchestration, and order change workflows.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::CommerceAdmin;

#[cfg(test)]
mod contract_tests;
