/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Calendar Admin
//!
//! Administrator user interface and appointment management package for `rustok-calendar`.
//!
//! Provides appointment calendars, booking overviews, time slot configuration,
//! and schedule management following Fluid Frontend Architecture.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::{CalendarAdminTableRow, filter_rows};
pub use transport::CalendarAdminTransport;
pub use ui::{CalendarAdmin, CalendarAdminPage};
