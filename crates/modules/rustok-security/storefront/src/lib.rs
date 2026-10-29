/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-security-storefront
//!
//! Leptos storefront FFA package for the `rustok-security` module.
//!
//! Provides account protection views, verification dialogs, and security
//! setting presentations for storefront users.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::SecurityStorefrontViewModel;
pub use transport::SecurityStorefrontTransport;
pub use ui::SecurityStorefrontView;

#[cfg(test)]
mod contract_tests;
