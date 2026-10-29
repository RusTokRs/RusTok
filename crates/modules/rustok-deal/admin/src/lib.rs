/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Deal Admin
//!
//! Leptos admin UI package for CRM Deals & Pipelines in the `rustok-deal` module.
//! Provides deal listing table rows, in-memory search filtering, and admin page components.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::{filter_rows, DealAdminTableRow};
pub use transport::DealAdminTransport;
pub use ui::{DealAdmin, DealAdminPage};

#[cfg(test)]
mod contract_tests;
