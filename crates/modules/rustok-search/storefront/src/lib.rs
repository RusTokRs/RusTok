//! # rustok-search-storefront
//!
//! SSR-first Leptos storefront UI package for the `rustok-search` module.
//!
//! Provides the canonical `SearchView` component, instant autocomplete suggestions,
//! facet navigation, and channel-scoped search execution.

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use transport::{
    SearchTransportError, fetch_filter_presets, fetch_forum_search_by_authors,
    fetch_forum_search_with_current_channel, fetch_forum_search_with_date_window,
    fetch_forum_search_with_filters, fetch_search, fetch_suggestions,
};
pub use ui::leptos::SearchCatalogFilterOption;
pub use ui::leptos::SearchView;
