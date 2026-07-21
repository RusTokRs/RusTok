/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pages module for RusToK platform.
//!
//! The module owns pages, localized bodies, menus, menu items, deterministic Page Builder
//! artifacts, and Page Builder release baselines.
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
//!         content: String::new(),
//!         format: Some("grapesjs".to_string()),
//!         content_json: Some(project_data),
//!     }),
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
pub mod openapi;
mod seo_targets;
pub mod services;

pub use dto::*;
pub use entities::{
    Menu, Page, PageBuilderScenarioBaseline, PagePublishedLandingArtifact,
    PageStaticLandingArtifact,
};
pub use error::{PagesError, PagesResult, CANNOT_DELETE_PUBLISHED_ERROR_CODE};
pub use graphql::{PagesMutation, PagesQuery};
pub use services::{
    MenuService, PageBuilderArtifactService, PageBuilderScenarioBaselineService, PageService,
    PublishedLandingArtifact, SaveIfCurrentScenarioBaselineRequest,
    PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_DOCUMENT_REVISION_CONFLICT,
    PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
};

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
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
        "Pages, visual documents, published artifacts and menus"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "page_builder"]
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

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::PagesSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "pages SEO target registration failed: {error}"
                ))
            },
        )
    }
}

impl MigrationSource for PagesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
