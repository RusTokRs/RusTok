//! # rustok-order-admin
//!
//! Leptos admin UI package for the `rustok-order` module.
//!
//! Exposes order operations, status tracking, line item inspection, and lifecycle
//! actions for customer orders.

#![recursion_limit = "256"]

pub mod core;
mod helpers;
mod i18n;
mod model;
mod transport;
pub mod ui;

pub use ui::OrderAdmin;

#[cfg(test)]
mod contract_tests;
