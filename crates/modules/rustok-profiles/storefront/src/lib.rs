//! # RusToK Profiles Storefront
//!
//! Storefront user interface and interaction controls for user profiles.
//!
//! Exposes public profile views, identity presentation, and follow relationship
//! management controls for customer-facing applications.

pub mod core;
mod i18n;
mod model;
mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::ProfilesView;
