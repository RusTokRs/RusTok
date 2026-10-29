/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-tasks-admin
//!
//! Leptos admin FFA package for the `rustok-tasks` module.
//!
//! Provides the admin dashboard UI, DataGrid task view-models, search filtering,
//! and issue management for team tasks.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::{TasksAdminTableRow, filter_rows};
pub use transport::TasksAdminTransport;
pub use ui::{TasksAdmin, TasksAdminPage};

#[cfg(test)]
mod contract_tests;
