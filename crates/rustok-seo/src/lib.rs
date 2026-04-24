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
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::{MigrationSource, Permission, RusToKModule};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;

pub use dto::{
    SeoAlternateLink, SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkArtifactRecord,
    SeoBulkBoolFieldPatch, SeoBulkExportInput, SeoBulkFieldPatchMode, SeoBulkImportInput,
    SeoBulkItem, SeoBulkJobOperationKind, SeoBulkJobRecord, SeoBulkJobStatus,
    SeoBulkJsonFieldPatch, SeoBulkListInput, SeoBulkMetaPatchInput, SeoBulkPage,
    SeoBulkSelectionInput, SeoBulkSelectionMode, SeoBulkSelectionPreviewRecord, SeoBulkSource,
    SeoBulkStringFieldPatch, SeoDiagnosticCountRecord, SeoDiagnosticIssueRecord,
    SeoDiagnosticSeverity, SeoDiagnosticsSummaryRecord, SeoDocument, SeoDocumentEffectiveState,
    SeoFieldSource, SeoFieldState, SeoImageAsset, SeoLinkTag, SeoMetaInput, SeoMetaRecord,
    SeoMetaTag, SeoMetaTranslationInput, SeoMetaTranslationRecord, SeoModuleSettings, SeoOpenGraph,
    SeoPageContext, SeoPagination, SeoRedirectDecision, SeoRedirectInput, SeoRedirectMatchType,
    SeoRedirectRecord, SeoRevisionRecord, SeoRobots, SeoRobotsPreviewRecord, SeoRouteContext,
    SeoSitemapFileRecord, SeoSitemapStatusRecord, SeoStructuredDataBlock, SeoTemplateRuleSet,
    SeoTwitterCard, SeoVerification, SeoVerificationTag,
};
#[cfg(feature = "server")]
pub use error::{SeoError, SeoResult};
#[cfg(feature = "server")]
pub use graphql::{SeoMutation, SeoQuery};
pub use rustok_seo_targets::{
    builtin_slug as seo_builtin_slug, SeoTargetCapabilities, SeoTargetCapabilityKind,
    SeoTargetRegistryEntry, SeoTargetSlug,
};
#[cfg(feature = "server")]
pub use rustok_seo_targets::SeoTargetRegistry;
#[cfg(feature = "server")]
pub use services::SeoService;

#[cfg(feature = "server")]
pub struct SeoModule;

#[cfg(feature = "server")]
#[async_trait]
impl RusToKModule for SeoModule {
    fn slug(&self) -> &'static str {
        "seo"
    }

    fn name(&self) -> &'static str {
        "SEO"
    }

    fn description(&self) -> &'static str {
        "SEO metadata, routing resolution, redirects, sitemaps, and robots runtime"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::SEO_READ,
            Permission::SEO_UPDATE,
            Permission::SEO_PUBLISH,
            Permission::SEO_GENERATE,
            Permission::SEO_MANAGE,
        ]
    }
}

#[cfg(feature = "server")]
impl MigrationSource for SeoModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
