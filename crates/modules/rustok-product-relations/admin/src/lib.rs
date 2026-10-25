/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-product-relations-admin
//!
//! Leptos Fluid Frontend Architecture (FFA) package for Product Relations administration.
//!
//! ## Responsibilities & Guarantees
//!
//! - **Strict FFA Isolation**: Framework-agnostic domain logic, input validation, and view-model
//!   computations live in `core`, with zero Leptos/UI dependencies.
//! - **Dual-Path Transport**: Supports both direct native SSR server functions and GraphQL transport
//!   paths with zero silent fallbacks.
//! - **Relation Management**: Admin UI for viewing, filtering, creating, removing, and reordering
//!   cross-sells, up-sells, accessories, alternatives, and custom product relations.
//!
//! ## Entry Points
//!
//! - [`ProductRelationsAdmin`]: Main routable admin view component.
//! - [`ProductRelationsPanel`]: Embeddable product relations management panel for product detail pages.
//! - [`load_product_relations`]: Transport loader for product relations.
//! - [`execute_product_relations_command`]: Command executor for relation mutations.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::{
    ProductRelationsTransportProfile, build_product_relations_panel_copy, filter_relations,
    is_valid_relation_type, matches_relation_filter, relation_grid_columns,
    selected_transport_profile, validate_target_product_id,
};
pub use model::{
    CreateProductRelationDraft, ProductRelationItem, ProductRelationsAdminCommand,
    ProductRelationsAdminCommandResult, ProductRelationsAdminFilters, ProductRelationsPanelCopy,
};
pub use transport::{
    PRODUCT_RELATIONS_FALLBACK_POLICY, ProductRelationsTransportContext,
    execute_product_relations_command, load_product_relations,
};
pub use ui::{ProductRelationsAdmin, ProductRelationsPanel};
