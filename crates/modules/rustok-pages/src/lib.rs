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
//!         document: project_data,
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
pub mod provider_health_binding;
mod seo_targets;
pub mod services;
mod translation_evidence;
mod translation_target;
#[cfg(test)]
mod translation_target_tests;

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
    Page, PageArtifactBindingReplacementOperation, PageArtifactRebuildOperation,
    PageBuilderScenarioBaseline, PagePublishOperation, PagePublishOperationArtifact,
    PagePublishRebuildSource, PagePublishedLandingArtifact, PageRollbackOperation, PageRouteAlias,
    PageRouteHistoryImport, PageRoutePublication, PageStaticLandingArtifact,
};
pub use error::{CANNOT_DELETE_PUBLISHED_ERROR_CODE, PagesError, PagesResult};
pub use graphql::{PagesMutation, PagesQuery};
pub use provider_health_binding::{
    PAGE_BUILDER_PROVIDER_HEALTH_SOURCE_COMMIT_ENV, PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV,
    PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID_ENV, PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST_ENV,
    PagesProviderHealthAuthority, PagesProviderHealthBindingError, PagesProviderHealthLiveIdentity,
    SharedPagesProviderHealthAuthority, page_builder_provider_health_authority_from_environment,
};
pub use services::{
    AuditPageArtifactsInput, DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS,
    DEFAULT_PAGE_INLINE_EDIT_CLOCK_SKEW_MS, DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS,
    ImportPageRouteHistoryInput, IssuedPageInlineEditGrant, MAX_PAGE_ARTIFACT_AUDIT_FINDINGS,
    MAX_PAGE_ARTIFACT_AUDIT_RECORDS, MAX_PAGE_INLINE_EDIT_GRANT_TTL_MS, MAX_PAGE_INLINE_EDIT_KEYS,
    MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS, PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT,
    PAGE_ARTIFACT_INTEGRITY_INVALID, PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT, PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY,
    PAGE_ARTIFACT_REBUILD_SOURCE_INVALID, PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
    PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED, PAGE_DOCUMENT_REVISION_CONFLICT,
    PAGE_INLINE_EDIT_CONTEXT_MISMATCH, PAGE_INLINE_EDIT_DOCUMENT_UNAVAILABLE,
    PAGE_INLINE_EDIT_GRANT_EXPIRED, PAGE_INLINE_EDIT_GRANT_INVALID, PAGE_INLINE_EDIT_GRANT_VERSION,
    PAGE_PUBLISH_IDEMPOTENCY_CONFLICT, PAGE_PUBLISH_OPERATION_INTEGRITY,
    PAGE_PUBLISHED_DOCUMENT_IMMUTABLE, PAGE_ROLLBACK_IDEMPOTENCY_CONFLICT,
    PAGE_ROLLBACK_OPERATION_INTEGRITY, PAGE_ROLLBACK_REQUIRES_PUBLISHED,
    PAGE_ROLLBACK_TARGET_UNAVAILABLE, PAGE_ROUTE_HISTORY_IMPORT_CONFLICT, PAGE_ROUTE_NOT_FOUND,
    PAGE_ROUTE_RESOLUTION_CONFLICT, PAGES_INLINE_EDIT_GRANT_TTL_MS_ENV,
    PAGES_INLINE_EDIT_HMAC_KEY_ENV, PAGES_INLINE_EDIT_HMAC_KEY_ID_ENV,
    PageArtifactIntegrityAuditResult, PageArtifactIntegrityFinding, PageBuilderArtifactService,
    PageBuilderScenarioBaselineService, PageInlineEditConfigError, PageInlineEditDocument,
    PageInlineEditGrantClaims, PageInlineEditGrantContext, PageInlineEditKeyId,
    PageInlineEditKeyring, PageInlineEditSecret, PageRouteDescriptor, PageRouteDisposition,
    PageRouteHistoryImportItem, PageRouteHistoryImportResult, PageRouteHistoryImportService,
    PageRouteResolution, PageRouteService, PageService, PublishedLandingArtifact,
    SaveIfCurrentScenarioBaselineRequest, inline_edit_context_mismatch,
    page_inline_edit_keyring_from_environment,
};
pub use translation_target::PagesMetadataTranslationTargetProvider;

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
        &["content", "page_builder", "outbox"]
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
        )?;
        match page_inline_edit_keyring_from_environment().map_err(|error| {
            rustok_core::Error::Validation(format!(
                "pages inline edit signing runtime configuration failed: {error}"
            ))
        })? {
            Some(keyring) => {
                extensions.insert(keyring);
                tracing::debug!("Pages inline edit signing runtime registered");
            }
            None => {
                tracing::debug!(
                    env = PAGES_INLINE_EDIT_HMAC_KEY_ENV,
                    "Pages inline edit signing runtime is not configured"
                );
            }
        }

        match page_builder_provider_health_authority_from_environment() {
            Ok(Some(authority)) => {
                tracing::info!(
                    source_commit = %authority.source_commit(),
                    deployment_id = %authority.deployment_id(),
                    provider_health_state = authority
                        .current_snapshot()
                        .map(|snapshot| snapshot.state.as_str())
                        .unwrap_or("unobserved"),
                    "Pages Page Builder provider-health authority registered"
                );
                extensions.insert(authority);
            }
            Ok(None) => {
                tracing::debug!(
                    env = PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV,
                    "Pages Page Builder provider-health authority is not configured"
                );
            }
            Err(error) => {
                tracing::warn!(
                    error_code = error.code(),
                    "Pages Page Builder provider-health authority rejected; provider health remains unobserved"
                );
            }
        }
        Ok(())
    }
}

impl MigrationSource for PagesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
