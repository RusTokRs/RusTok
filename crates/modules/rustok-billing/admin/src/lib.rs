/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Billing Admin
//!
//! Administrator user interface and management tools for B2B billing and settlements.
//!
//! Provides accounting overviews, invoice auditing, settlement review,
//! and billing configuration following Fluid Frontend Architecture.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::{BillingAdminTableRow, filter_rows};
pub use transport::BillingAdminTransport;
pub use ui::{BillingAdmin, BillingAdminPage};
