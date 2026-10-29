//! # RusToK Fulfillment Admin
//!
//! Leptos admin UI package for the `rustok-fulfillment` module.
//! Provides shipping option management, delivery profile configuration, and fulfillment administration.

#![recursion_limit = "256"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::FulfillmentAdmin;

#[cfg(test)]
mod contract_tests;
