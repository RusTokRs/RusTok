//! # RusToK Groups Admin
//!
//! Leptos admin FFA package for social group administration in the `rustok-groups` module.
//! Provides group directory, governance, membership application review, and visual policy editors.

pub mod application_bulk_core;
pub mod application_bulk_transport;
pub mod application_core;
pub mod application_model;
pub mod core;
mod i18n;
pub mod model;
pub mod policy_locale_compat;
pub mod transport;
mod ui;

pub use ui::root::GroupsAdmin;

#[cfg(test)]
mod contract_tests;
