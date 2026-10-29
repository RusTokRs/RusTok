//! # RusToK Groups Storefront
//!
//! Leptos storefront FFA package for social group discovery and membership applications in the `rustok-groups` module.
//! Provides public group browsing, membership application forms, and invitation acceptance.

pub mod application_core;
pub mod application_model;
pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

pub use ui::leptos::GroupsView;

#[cfg(test)]
mod contract_tests;
