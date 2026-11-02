//! # rustok-tenant-admin
//!
//! Leptos admin UI package for the `rustok-tenant` module.
//!
//! Exposes tenant management, isolation policies, active module inspection,
//! and tenancy settings for platform administrators.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::TenantAdmin;

#[cfg(test)]
mod contract_tests;
