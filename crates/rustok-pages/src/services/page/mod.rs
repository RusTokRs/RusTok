mod artifact_set;
mod create;
mod document;
mod helpers;
mod inline_edit;
mod lifecycle;
mod metadata;
mod persistence;
pub(crate) mod publish_manifest;
mod read;
mod reviewed_publish;
mod rollback;
mod route;
mod route_history_import;

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
pub use lifecycle::PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED;
pub use route::{
    PAGE_ROUTE_NOT_FOUND, PAGE_ROUTE_RESOLUTION_CONFLICT, PageRouteDescriptor,
    PageRouteDisposition, PageRouteResolution, PageRouteService,
};
pub use route_history_import::{
    ImportPageRouteHistoryInput, MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS,
    PAGE_ROUTE_HISTORY_IMPORT_CONFLICT, PageRouteHistoryImportItem,
    PageRouteHistoryImportResult, PageRouteHistoryImportService,
};

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
