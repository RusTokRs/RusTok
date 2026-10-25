//! # rustok-index-admin
//!
//! Leptos admin UI package for the `rustok-index` module.
//!
//! Provides the canonical `IndexAdmin` operator diagnostics surface, schema catalogs,
//! storage layout diagnostics, replay runner controls, and job monitoring for operators.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::IndexAdmin;
