//! Navigation module for localized menus and channel slot bindings.

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

pub use dto::*;
pub use entities::{Menu, MenuBinding, MenuItem};
pub use error::{NavigationError, NavigationResult};
pub use graphql::{NavigationMutation, NavigationQuery};
pub use module::NavigationModule;
pub use services::{
    MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE, MenuBindingService,
    MenuService,
};
pub use translation_target::NavigationMenuTranslationTargetProvider;
