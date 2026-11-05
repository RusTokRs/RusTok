/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-security-admin
//!
//! Leptos admin FFA package for the `rustok-security` module.
//!
//! Provides the admin dashboard UI, security policy controls, API token management,
//! and audit log presentation.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::{SecurityAdminTableRow, filter_rows};
pub use transport::SecurityAdminTransport;
pub use ui::SecurityAdminPage;

#[cfg(test)]
mod contract_tests;
