//! # RusToK Channel Admin
//!
//! Leptos admin UI package for the `rustok-channel` module.
//! Provides channel administration, routing target management, and module bindings.

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use transport::ApiError;
pub use ui::leptos::ChannelAdmin;

#[cfg(test)]
mod contract_tests;
