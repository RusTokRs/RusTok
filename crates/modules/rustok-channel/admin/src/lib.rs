//! # RusToK Channel Admin
//!
//! Leptos admin UI package for the `rustok-channel` module.
//! Provides channel administration, routing target management, and module bindings.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::ChannelAdmin;

#[cfg(test)]
mod contract_tests;
