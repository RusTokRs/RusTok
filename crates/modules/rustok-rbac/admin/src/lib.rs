//! # rustok-rbac-admin
//!
//! Leptos admin UI package for the `rustok-rbac` module.
//!
//! Exposes the RBAC runtime overview, permission catalog, role assignments,
//! and access control matrices for platform administrators.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::RbacAdmin;

#[cfg(test)]
mod contract_tests;
