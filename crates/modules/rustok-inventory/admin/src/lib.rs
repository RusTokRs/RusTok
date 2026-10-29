//! # RusToK Inventory Admin
//!
//! Leptos admin UI package for the `rustok-inventory` module.
//! Provides stock visibility, inventory level adjustments, reservation tracking, and stock-health management.

#![recursion_limit = "256"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::InventoryAdmin;

#[cfg(test)]
mod contract_tests;
