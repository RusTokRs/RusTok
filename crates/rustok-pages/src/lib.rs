//! Pages module for RusToK platform
//!
//! This module provides pages, blocks and menus functionality built on top of the content module.
//!
//! # Architecture
//!
//! The pages module is a "wrapper" module that:
//! - Uses `rustok-content` tables for storage (no own database schema)
//! - Adds pages-specific business logic and validation
//! - Provides PageService, BlockService, and MenuService
//! - Publishes domain events through TransactionalEventBus
//!
//! # Example
//!
//! ```rust,ignore
//! use rustok_pages::{PageService, CreatePageInput, PageResponse};
//!
//! // Create a page service
//! let service = PageService::new(db, event_bus);
//!
//! // Create a page
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
//!     publish: false,
//! };
//!
//! let page = service.create_page(tenant_id, security, input).await?;
//! ```

pub mod dto;
pub mod entities;
pub mod error;
pub mod services;

pub use dto::*;
pub use entities::{Block, Menu, Page};
pub use error::{PagesError, PagesResult};
pub use services::{BlockService, MenuService, PageService};

use async_trait::async_trait;
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Pages module instance
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

    fn permissions(&self) -> Vec<Permission> {
        vec![
            // Pages
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::List),
            Permission::new(Resource::Pages, Action::Publish),
            // Blocks (stored as nodes)
            Permission::new(Resource::Nodes, Action::Create),
            Permission::new(Resource::Nodes, Action::Read),
            Permission::new(Resource::Nodes, Action::Update),
            Permission::new(Resource::Nodes, Action::Delete),
            Permission::new(Resource::Nodes, Action::List),
        ]
    }
}

impl MigrationSource for PagesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        // Pages module uses content module tables, no own migrations
        Vec::new()
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

        // Check pages permissions
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Pages && p.action == Action::Create }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Pages && p.action == Action::Publish }));

        // Check blocks (nodes) permissions
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Nodes && p.action == Action::Create }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Nodes && p.action == Action::Delete }));
    }

    #[test]
    fn module_migrations_empty() {
        let module = PagesModule;
        assert!(module.migrations().is_empty());
    }
}

#[cfg(test)]
mod contract_tests;
