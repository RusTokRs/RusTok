//! # RusToK SEO Admin
//!
//! Leptos administration package for the RusToK SEO module, providing
//! URL-driven navigation, index repair/replay, sitemap management, and diagnostics.

mod core;
mod i18n;
mod sections;
mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::SeoAdmin;
