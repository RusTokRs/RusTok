/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-navigation-storefront
//!
//! Leptos Fluid Frontend Architecture (FFA) storefront slot package for navigation menus.
//!
//! ## Responsibilities & Guarantees
//!
//! - **Strict FFA Isolation**: View-models and snapshot structures in `model` are pure Rust
//!   data structures independent of Leptos reactive signals or WASM bindings.
//! - **Dual-Path Transport**: Seamlessly supports both direct native Server Functions (`ssr`)
//!   and headless GraphQL transports via `fetch_active_menu`.
//! - **Slot Integration**: Provides pluggable storefront navigation slots (`NavigationHeaderMenu`,
//!   `NavigationView`) without requiring direct consumer coupling to backend database entities.
//!
//! ## Entry Points
//!
//! - [`NavigationHeaderMenu`]: Primary header menu navigation bar.
//! - [`NavigationSnapshotProvider`]: Context provider injecting navigation hierarchy into the DOM tree.
//! - [`NavigationView`]: Context-aware menu renderer for specific layout locations.
//! - [`fetch_active_menu`]: Asynchronous transport fetcher for active menus by location and locale.

pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use model::{
    StorefrontMenu, StorefrontMenuItem, StorefrontMenuLocation, StorefrontNavigationSnapshot,
};
pub use transport::{NavigationTransportError, fetch_active_menu};
pub use ui::{NavigationHeaderMenu, NavigationSnapshotProvider, NavigationView};
