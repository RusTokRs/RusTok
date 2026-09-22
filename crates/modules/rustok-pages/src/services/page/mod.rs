mod artifact_binding_replacement;
mod artifact_integrity_audit;
mod artifact_rebuild;
mod artifact_set;
mod create;
mod document;
mod helpers;
mod inline_edit;
mod inline_edit_feature;
mod inline_edit_runtime;
mod lifecycle;
mod metadata;
mod persistence;
pub(crate) mod publish_manifest;
mod read;
mod reviewed_publish;
mod rollback;
mod route;
mod route_history_import;
mod translation_apply;

use rustok_content::entities::node::ContentStatus;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::entities::page_translation;

pub use crate::error::{
    PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
    PAGE_PUBLISH_IDEMPOTENCY_CONFLICT, PAGE_PUBLISH_OPERATION_INTEGRITY,
    PAGE_ROLLBACK_IDEMPOTENCY_CONFLICT, PAGE_ROLLBACK_OPERATION_INTEGRITY,
    PAGE_ROLLBACK_REQUIRES_PUBLISHED, PAGE_ROLLBACK_TARGET_UNAVAILABLE,
};
pub use artifact_binding_replacement::{
    PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID,
};
pub use artifact_integrity_audit::{
    AuditPageArtifactsInput, DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS, MAX_PAGE_ARTIFACT_AUDIT_FINDINGS,
    MAX_PAGE_ARTIFACT_AUDIT_RECORDS, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT,
    PAGE_ARTIFACT_INTEGRITY_INVALID, PageArtifactIntegrityAuditResult,
    PageArtifactIntegrityFinding,
};
pub use artifact_rebuild::{
    PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT, PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT,
    PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY, PAGE_ARTIFACT_REBUILD_SOURCE_INVALID,
};
pub use document::{PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE};
pub(crate) use helpers::is_page_visible_for_channel;
pub use inline_edit::{
    DEFAULT_PAGE_INLINE_EDIT_CLOCK_SKEW_MS, DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS,
    IssuedPageInlineEditGrant, MAX_PAGE_INLINE_EDIT_GRANT_TTL_MS, MAX_PAGE_INLINE_EDIT_KEYS,
    PAGE_INLINE_EDIT_CONTEXT_MISMATCH, PAGE_INLINE_EDIT_DOCUMENT_UNAVAILABLE,
    PAGE_INLINE_EDIT_GRANT_EXPIRED, PAGE_INLINE_EDIT_GRANT_INVALID, PAGE_INLINE_EDIT_GRANT_VERSION,
    PageInlineEditConfigError, PageInlineEditDocument, PageInlineEditGrantClaims,
    PageInlineEditGrantContext, PageInlineEditKeyId, PageInlineEditKeyring, PageInlineEditSecret,
    inline_edit_context_mismatch,
};
pub use inline_edit_feature::FEATURE_BUILDER_INLINE_EDIT_ENABLED;
pub use inline_edit_runtime::{
    PAGES_INLINE_EDIT_GRANT_TTL_MS_ENV, PAGES_INLINE_EDIT_HMAC_KEY_ENV,
    PAGES_INLINE_EDIT_HMAC_KEY_ID_ENV, page_inline_edit_keyring_from_environment,
};
pub use lifecycle::PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED;
pub use route::{
    PAGE_ROUTE_NOT_FOUND, PAGE_ROUTE_RESOLUTION_CONFLICT, PageRouteDescriptor,
    PageRouteDisposition, PageRouteResolution, PageRouteService,
};
pub use route_history_import::{
    ImportPageRouteHistoryInput, MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS,
    PAGE_ROUTE_HISTORY_IMPORT_CONFLICT, PageRouteHistoryImportItem, PageRouteHistoryImportResult,
    PageRouteHistoryImportService,
};
pub(crate) use translation_apply::ApplyExactPageMetadataTranslationInput;

pub(super) const PAGE_KIND: &str = "page";

#[derive(Clone)]
pub struct PageService {
    pub(super) db: DatabaseConnection,
    pub(super) event_bus: TransactionalEventBus,
}

impl PageService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}

pub(super) struct PageResponseParts {
    pub(super) channel_slugs: Vec<String>,
    pub(super) locale: String,
    pub(super) fallback_locale: Option<String>,
}

pub(super) struct PreparedPageBody {
    pub(super) locale: String,
    pub(super) content: String,
    pub(super) format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageTransition {
    Publish,
    Unpublish,
    Archive,
}

impl PageTransition {
    pub(super) fn status(self) -> ContentStatus {
        match self {
            Self::Publish => ContentStatus::Published,
            Self::Unpublish => ContentStatus::Draft,
            Self::Archive => ContentStatus::Archived,
        }
    }
}

pub(super) struct ResolvedTranslationRecord<'a> {
    pub(super) translation: Option<&'a page_translation::Model>,
    pub(super) effective_locale: String,
}
