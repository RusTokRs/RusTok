//! # RusToK Cache Admin
//!
//! Leptos admin UI package for the `rustok-cache` module.
//! Provides cache configuration, diagnostics views, and dual-path transport adapters.

pub mod core;
pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{
    build_cache_settings_payload, parse_cache_settings_json, CacheAdminFormDraft, CacheAdminLabels,
    CacheDiagnosticsViewModel, CacheModeOption,
};
pub use model::{
    CacheHealthPayload, GraphqlCacheHealthResponse, GraphqlPlatformSettingsResponse,
    GraphqlUpdatePlatformSettingsResponse, PlatformSettingsData, UpdatePlatformSettingsPayload,
};
pub use transport::{fetch_cache_health, fetch_cache_settings, update_cache_settings};
pub use ui::CacheAdmin;

#[cfg(test)]
mod contract_tests;
