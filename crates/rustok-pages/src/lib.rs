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
//! The module owns pages, localized bodies, deterministic Page Builder artifacts,
//! atomic publish/rollback receipts, cache policy, and Page Builder release baselines.
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

pub mod cache_invalidation;
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod http;
pub mod migrations;
pub mod openapi;
mod seo_targets;
pub mod services;

pub use cache_invalidation::{
    MAX_PAGE_CACHE_KEY_VARIANT_BYTES, MAX_PAGE_CACHE_VALUE_BYTES, PAGE_CACHE_MUTABLE_SCOPES,
    PAGE_CACHE_SCOPES, PAGES_CACHE_ENTITY_KIND, PAGES_CACHE_EVENT_HANDLER,
    PAGES_CACHE_NAMESPACE_FORMAT, PAGES_STOREFRONT_CACHE_MAX_CAPACITY,
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PageCacheInvalidationCause, PageCacheInvalidationEventHandler, PageCacheInvalidationPort,
    PageCacheInvalidationReceipt, PageCacheInvalidationRequest, PageCacheScope,
    PagesCacheInvalidationRuntime, PagesCacheReadPort, PagesCacheReadRuntime, page_cache_key,
    page_cache_namespace, storefront_pages_cache_key,
};
pub use dto::*;
pub use entities::{
    Page, PageBuilderScenarioBaseline, PagePublishOperation, PagePublishOperationArtifact,
    PagePublishedLandingArtifact, PageRollbackOperation, PageStaticLandingArtifact,
};
pub use error::{CANNOT_DELETE_PUBLISHED_ERROR_CODE, PagesError, PagesResult};
pub use graphql::{PagesMutation, PagesQuery};
pub use services::{
    PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
    PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED, PAGE_DOCUMENT_REVISION_CONFLICT,
    PAGE_PUBLISH_IDEMPOTENCY_CONFLICT, PAGE_PUBLISH_OPERATION_INTEGRITY,
    PAGE_PUBLISHED_DOCUMENT_IMMUTABLE, PAGE_ROLLBACK_IDEMPOTENCY_CONFLICT,
    PAGE_ROLLBACK_OPERATION_INTEGRITY, PAGE_ROLLBACK_REQUIRES_PUBLISHED,
    PAGE_ROLLBACK_TARGET_UNAVAILABLE, PageBuilderArtifactService,
    PageBuilderScenarioBaselineService, PageService, PublishedLandingArtifact,
    SaveIfCurrentScenarioBaselineRequest,
};

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleRuntimeExtensions, RusToKModule,
};
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
        "Pages, visual documents and published artifacts"
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

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let Some(runtime) = ctx
            .extensions
            .get::<PagesCacheInvalidationRuntime>()
            .cloned()
        else {
            tracing::warn!(
                "Pages cache invalidation runtime is not configured; no Pages cache listener registered"
            );
            return;
        };
        registry.register(PageCacheInvalidationEventHandler::new(runtime));
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
