/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-brand-admin
//!
//! Leptos Fluid Frontend Architecture (FFA) package for Brand catalog administration.
//!
//! ## Responsibilities & Guarantees
//!
//! - **Strict FFA Isolation**: Framework-agnostic validation, view-models, and presentation logic
//!   live in `core`, decoupled from Leptos signals or render trees.
//! - **Dual-Path Transport**: Seamlessly supports both direct native SSR server functions and
//!   headless GraphQL transports via `BrandAdminTransportContext`.
//! - **Brand Lifecycle Management**: Admin UI components for listing, filtering, creating,
//!   editing, and translating brands and manufacturer records.
//!
//! ## Entry Points
//!
//! - [`BrandAdmin`]: Main routable admin view component for brand catalog management.
//! - [`load_brand_directory`]: Transport loader for paginated brand lists.
//! - [`load_brand_detail`]: Transport loader for single brand records with translations.
//! - [`execute_brand_command`]: Command executor for creating, updating, and deleting brands.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::{
    BrandAdminTransportProfile, brand_grid_columns, build_brand_admin_shell,
    selected_transport_profile, validate_brand_name, validate_brand_slug,
};
pub use model::{
    BrandAdminCommand, BrandAdminCommandResult, BrandAdminCreateDraft, BrandAdminDirectory,
    BrandAdminFilters, BrandAdminListItem, BrandAdminRecord, BrandAdminShell,
    BrandAdminTranslation, BrandAdminUpdateDraft,
};
pub use transport::{
    BRAND_TRANSPORT_FALLBACK_POLICY, BrandAdminTransportContext, execute_brand_command,
    load_brand_detail, load_brand_directory,
};
pub use ui::leptos::BrandAdmin;
