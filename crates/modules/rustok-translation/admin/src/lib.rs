//! # rustok-translation-admin
//!
//! Leptos admin UI package for the `rustok-translation` module.
//!
//! Exposes translation workbench, job tracking, glossaries, Translation Memory,
//! reviewer queues, and interchange artifact management.

#![recursion_limit = "256"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::TranslationAdmin;

#[cfg(test)]
mod contract_tests;
