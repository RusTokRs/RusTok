//! # rustok-seo-admin
//!
//! Leptos admin UI package for the `rustok-seo` module.
//!
//! Provides the canonical `SeoAdmin` control-plane workstation for redirects,
//! bulk SEO metadata management, sitemaps, robots.txt, and site-wide SEO diagnostics.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod sections;
pub mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::leptos::SeoAdmin;
