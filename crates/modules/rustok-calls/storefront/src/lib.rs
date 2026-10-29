/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Calls Storefront
//!
//! Storefront user interface and peer conferencing package for `rustok-calls`.
//!
//! Provides public WebRTC call views, media controls, peer connection state displays,
//! and room participation interfaces following Fluid Frontend Architecture.

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use core::CallsViewModel;
pub use transport::CallsStorefrontTransport;
pub use ui::CallsView;
