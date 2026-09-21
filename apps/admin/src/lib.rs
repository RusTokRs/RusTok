/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#![recursion_limit = "512"]

pub mod app;
pub mod entities;
pub mod features;
pub mod i18n;
pub mod pages;
pub mod shared;
pub mod widgets;

pub use app::providers::locale::{
    AdminLocaleContext, AdminLocaleProvider, Locale, use_admin_locale,
};
