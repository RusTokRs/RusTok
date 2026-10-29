/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Calls Admin
//!
//! Administrator user interface and session management package for `rustok-calls`.
//!
//! Provides WebRTC call administration, room lifecycle management, active session review,
//! and conference settings following Fluid Frontend Architecture.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::{CallsAdminTableRow, filter_rows};
pub use transport::CallsAdminTransport;
pub use ui::{CallsAdmin, CallsAdminPage};
