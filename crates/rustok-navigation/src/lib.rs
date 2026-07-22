//! Navigation module for localized menus and channel slot bindings.

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod http;
pub mod migrations;
pub mod openapi;
pub mod services;

pub use dto::*;
pub use entities::{Menu, MenuBinding, MenuItem};
pub use error::{NavigationError, NavigationResult};
pub use graphql::{NavigationMutation, NavigationQuery};
pub use services::{MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE, MenuBindingService, MenuService};

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub struct NavigationModule;

#[async_trait]
impl RusToKModule for NavigationModule {
    fn slug(&self) -> &'static str { "navigation" }
    fn name(&self) -> &'static str { "Navigation" }
    fn description(&self) -> &'static str { "Localized navigation menus and current-channel slot bindings" }
    fn version(&self) -> &'static str { env!("CARGO_PKG_VERSION") }
    fn dependencies(&self) -> &[&'static str] { &["channel"] }
    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Navigation, Action::Create), Permission::new(Resource::Navigation, Action::Read),
            Permission::new(Resource::Navigation, Action::Update), Permission::new(Resource::Navigation, Action::Delete),
            Permission::new(Resource::Navigation, Action::List), Permission::new(Resource::Navigation, Action::Manage),
        ]
    }
}
impl MigrationSource for NavigationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> { migrations::migrations() }
}
