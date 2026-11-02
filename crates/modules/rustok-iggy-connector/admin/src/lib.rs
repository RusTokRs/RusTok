//! # rustok-iggy-connector-admin
//!
//! Leptos admin UI package for the `rustok-iggy-connector` module.
//!
//! Exposes administrative controls for selecting and configuring bundled or
//! external Iggy message streaming endpoints.

mod core;
pub mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::IggyConnectorAdmin;

#[cfg(test)]
mod contract_tests;
