/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-party-admin
//!
//! Leptos admin FFA package for Counterparty & Legal Entities (`rustok-party`).
//!
//! Provides the admin dashboard UI, DataGrid view models, search filtering,
//! and transport facade for managing parties and legal entities.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::{filter_rows, PartyAdminTableRow};
pub use transport::PartyAdminTransport;
pub use ui::{PartyAdmin, PartyAdminPage};

#[cfg(test)]
mod contract_tests;
