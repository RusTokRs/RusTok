//! Pages module for RusToK platform.
//!
//! The module owns storage for pages, page blocks, menus, and menu items.
//!
//! # Example
//!
//! ```rust,ignore
//! use rustok_pages::{CreatePageInput, PageBodyInput, PageService, PageTranslationInput};
//!
//! let service = PageService::new(db, event_bus);
//! let input = CreatePageInput {
//!     translations: vec![PageTranslationInput {
//!         locale: "en".to_string(),
//!         title: "About Us".to_string(),
//!         slug: Some("about-us".to_string()),
//!         meta_title: None,
//!         meta_description: None,
//!     }],
//!     template: Some("default".to_string()),
//!     body: Some(PageBodyInput {
//!         locale: "en".to_string(),
//!         content: "Welcome to our company!".to_string(),
//!         format: Some("markdown".to_string()),
//!         content_json: None,
//!     }),
//!     blocks: None,
//!     channel_slugs: None,
//!     publish: false,
//! };
//!
//! let page = service.create(tenant_id, security, input).await?;
//! ```

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
mod seo_targets;
pub mod services;

pub use dto::*;
pub use entities::{Block, Menu, Page};
pub use error::{PagesError, PagesResult};
pub use graphql::{PagesMutation, PagesQuery};
pub use services::{BlockService, MenuService, PageService};

use async_trait::async_trait;
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

/// Pages module instance.
pub struct PagesModule;

#[async_trait]
impl RusToKModule for PagesModule {
    fn slug(&self) -> &'static str {
        "pages"
    }

    fn name(&self) -> &'static str {
        "Pages"
    }

    fn description(&self) -> &'static str {
        "Static pages, blocks and menus"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::List),
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ]
    }

    fn register_runtime_extensions(&self, extensions: &mut ModuleRuntimeExtensions) {
        register_seo_target_provider(extensions, seo_targets::PagesSeoTargetProvider)
            .expect("pages SEO target registration should remain unique");
    }
}

impl MigrationSource for PagesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = PagesModule;
        assert_eq!(module.slug(), "pages");
        assert_eq!(module.name(), "Pages");
        assert_eq!(module.description(), "Static pages, blocks and menus");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn module_permissions() {
        let module = PagesModule;
        let permissions = module.permissions();

        assert!(permissions
            .iter()
            .any(|p| p.resource == Resource::Pages && p.action == Action::Create));
        assert!(permissions
            .iter()
            .any(|p| p.resource == Resource::Pages && p.action == Action::Publish));
        assert!(
            permissions.iter().all(|p| p.resource != Resource::Nodes),
            "pages module should no longer publish node permissions"
        );
    }

    #[test]
    fn module_has_migrations() {
        let module = PagesModule;
        assert!(!module.migrations().is_empty());
    }
}

#[cfg(test)]
mod contract_tests;
