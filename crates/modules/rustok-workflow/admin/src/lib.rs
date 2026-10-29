//! # rustok-workflow-admin
//!
//! Leptos admin UI package for the `rustok-workflow` module.
//!
//! Exposes workflow administration, step-by-step executions, templates, and
//! operational state tracking.

#![recursion_limit = "256"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::WorkflowAdmin;

#[cfg(test)]
mod contract_tests;
