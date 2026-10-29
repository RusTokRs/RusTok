/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Finance Admin
//!
//! Leptos admin UI package for Operational Finance & Cash Desks in the `rustok-finance` module.
//! Provides finance listing table rows, in-memory search filtering, and admin page components.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::{filter_rows, FinanceAdminTableRow};
pub use transport::FinanceAdminTransport;
pub use ui::{FinanceAdmin, FinanceAdminPage};

#[cfg(test)]
mod contract_tests;
