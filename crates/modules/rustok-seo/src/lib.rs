/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of SEO runtime, metadata, redirects, sitemaps, and robots in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

#[cfg(feature = "server")]
mod authorization;

#[cfg(feature = "server")]
pub mod controllers;
pub mod dto;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod graphql;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod services;

#[cfg(feature = "server")]
pub use authorization::SeoWorkerAuthorization;
pub use dto::{
    SeoAlternateLink, SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkArtifactRecord,
    SeoBulkBoolFieldPatch, SeoBulkExportInput, SeoBulkFieldPatchMode, SeoBulkImportInput,
    SeoBulkItem, SeoBulkJobOperationKind, SeoBulkJobRecord, SeoBulkJobStatus,
    SeoBulkJobStatusRecord, SeoBulkJsonFieldPatch, SeoBulkListInput, SeoBulkMetaPatchInput,
    SeoBulkPage, SeoBulkSelectionInput, SeoBulkSelectionMode, SeoBulkSelectionPreviewRecord,
    SeoBulkSource, SeoBulkStringFieldPatch, SeoCrossLinkSuggestionRecord, SeoDiagnosticCountRecord,
    SeoDiagnosticIssueRecord, SeoDiagnosticSeverity, SeoDiagnosticsSummaryRecord, SeoDocument,
    SeoDocumentEffectiveState, SeoFieldSource, SeoFieldState, SeoImageAsset, SeoIndexCursorRecord,
    SeoIndexDeliveryStatusRecord, SeoIndexRepairReplayInput, SeoIndexRepairReplayResultRecord,
    SeoIndexReplayMode, SeoLinkTag, SeoMetaInput, SeoMetaRecord, SeoMetaTag,
    SeoMetaTranslationInput, SeoMetaTranslationRecord, SeoModuleSettings, SeoOpenGraph,
    SeoPageContext, SeoPagination, SeoRedirectDecision, SeoRedirectInput, SeoRedirectMatchType,
    SeoRedirectRecord, SeoRevisionRecord, SeoRobots, SeoRobotsPreviewRecord, SeoRouteContext,
    SeoSchemaBlockKind, SeoSitemapFileRecord, SeoSitemapJobRecord, SeoSitemapStatusRecord,
    SeoStructuredDataBlock, SeoTemplateRuleSet, SeoTwitterCard, SeoVerification,
    SeoVerificationTag,
};
#[cfg(feature = "server")]
pub use error::{SeoError, SeoFailure, SeoFailureClass, SeoResult};
#[cfg(feature = "server")]
pub use graphql::{SeoMutation, SeoQuery};
#[cfg(feature = "server")]
pub use rustok_seo_targets::SeoTargetRegistry;
pub use rustok_seo_targets::{
    SeoTargetCapabilities, SeoTargetCapabilityKind, SeoTargetRegistryEntry, SeoTargetSlug,
    builtin_slug as seo_builtin_slug,
};
#[cfg(feature = "server")]
pub use services::{
    SeoApplicationServices, SeoBulkService, SeoMediaAssetReadProvider, SeoMetadataService,
    SeoOperationsService, SeoRedirectService, SeoRoutingService, SeoSettingsService,
    SeoSitemapService, SeoTranslationTargetProvider, register_seo_translation_target_provider,
};

#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub use module::SeoModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;
