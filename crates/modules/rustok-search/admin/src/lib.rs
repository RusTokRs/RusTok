//! # rustok-search-admin
//!
//! SSR-first Leptos admin UI package for the `rustok-search` module.
//!
//! Provides the canonical `SearchAdmin` root view, search preview playground,
//! dictionary and synonym management, query analytics, and index diagnostics.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod model;
mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::SearchAdmin;
pub use ui::leptos::SearchCatalogFilterOption;
