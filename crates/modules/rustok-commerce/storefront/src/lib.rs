//! # RusToK Commerce Storefront
//!
//! Leptos storefront UI package for the `rustok-commerce` module.
//! Provides checkout workspace, shipping selection orchestration, and commerce views.

#![recursion_limit = "512"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::CommerceView;

#[cfg(test)]
mod contract_tests;
