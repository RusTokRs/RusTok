/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-navigation
//!
//! Canonical platform owner of localized navigation menus and channel slot bindings in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod http;
pub mod migrations;
pub mod module;
pub mod openapi;
pub mod services;

mod translation_evidence;
mod translation_target;

#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod translation_target_tests;

pub use dto::{
    ActiveMenuBindingResponse, BindActiveMenuInput, CreateMenuInput, MenuItemInput,
    MenuItemResponse, MenuItemTranslationInput, MenuLocation, MenuResponse, MenuTranslationInput,
};

pub use entities::{Menu, MenuBinding, MenuItem};
pub use error::{NavigationError, NavigationResult};
pub use graphql::{NavigationMutation, NavigationQuery};
pub use module::NavigationModule;
pub use services::{
    MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE, MenuBindingService,
    MenuService,
};
pub use translation_target::NavigationMenuTranslationTargetProvider;
