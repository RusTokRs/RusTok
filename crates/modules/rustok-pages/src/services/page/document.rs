use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_core::{SecurityContext, error::ErrorKind, error::RichError};
use rustok_events::DomainEvent;
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;

use crate::dto::{
    PageBodyRevisionResponse, PageBodyRevisionSource, PageResponse, RestorePageBodyRevisionInput,
    SavePageDocumentInput,
};
use crate::entities::{page, page_body_revision, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

use super::helpers::{body_revision_timestamp, normalize_locale, normalize_page_body_input};
use super::{PAGE_KIND, PageService, PreparedPageBody, WorkingBody};

pub const PAGE_DOCUMENT_REVISION_CONFLICT: &str = "PAGE_DOCUMENT_REVISION_CONFLICT";
pub const PAGE_BODY_REVISION_NOT_FOUND: &str = "PAGE_BODY_REVISION_NOT_FOUND";

/// Newest body revisions returned by one history read.
pub const MAX_PAGE_BODY_REVISIONS_RETURNED: u64 = 200;

impl PageService {
    /// Saves one localized builder document into the page's working copy.
    ///
    /// While the page is published the working copy is the page body draft and the public body
    /// stays untouched; every other lifecycle state updates the current body directly. Both
    /// paths append one body-revision journal row.
    #[instrument(skip(self, input))]
    pub async fn save_document(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: SavePageDocumentInput,
    ) -> PagesResult<PageResponse> {
        let body = normalize_page_body_input(Some(input.body))?
            .ok_or_else(|| PagesError::validation("Page document body is required"))?;
        if body.format != PAGE_BUILDER_DOCUMENT_FORMAT {
            return Err(PagesError::validation(
                "Page document save accepts only the current Fly/GrapesJS body format",
            ));
        }
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;

        let response_locale = body.locale.clone();
        let txn = self.db.begin().await?;
        let locked_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        Self::ensure_builder_enabled_in_tx(&txn, tenant_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            locked_page.author_id,
        )?;

        let translation_exists = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(&body.locale))
            .one(&txn)
            .await?
            .is_some();
        if !translation_exists {
            return Err(PagesError::validation(format!(
                "Page document locale `{}` requires a matching page translation",
                body.locale
            )));
        }

        let now = body_revision_timestamp(Utc::now());
        if locked_page.status == "published" {
            let draft = self
                .find_draft_in_tx(&txn, tenant_id, page_id, &body.locale)
                .await?;
            let current = self
                .find_body_in_tx(&txn, tenant_id, page_id, &body.locale)
                .await?;
            let working = draft
                .as_ref()
                .map(WorkingBody::from)
                .or_else(|| current.as_ref().map(WorkingBody::from));
            let actual_revision = page_document_revision(page_id, working.as_ref());
            if input.expected_revision != actual_revision {
                return Err(document_revision_conflict(
                    input.expected_revision,
                    actual_revision,
                ));
            }

            let draft = self
                .upsert_draft_in_tx(&txn, tenant_id, page_id, &body, now, security.user_id)
                .await?;
            let body_revision = draft.updated_at.to_string();
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                page_id,
                &draft.locale,
                &draft.content,
                &draft.format,
                PageBodyRevisionSource::DraftSave,
                &body_revision,
                security.user_id,
            )
            .await?;
        } else {
            let existing = self
                .find_body_in_tx(&txn, tenant_id, page_id, &body.locale)
                .await?;
            let working = existing.as_ref().map(WorkingBody::from);
            let actual_revision = page_document_revision(page_id, working.as_ref());
            if input.expected_revision != actual_revision {
                return Err(document_revision_conflict(
                    input.expected_revision,
                    actual_revision,
                ));
            }

            self.upsert_body_in_tx(&txn, tenant_id, page_id, Some(body.clone()), now)
                .await?;
            let body_revision = now.to_string();
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                page_id,
                &body.locale,
                &body.content,
                &body.format,
                PageBodyRevisionSource::Save,
                &body_revision,
                security.user_id,
            )
            .await?;

            let mut page_active: page::ActiveModel = locked_page.into();
            page_active.updated_at = Set(now);
            page_active.update(&txn).await?;

            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::NodeUpdated {
                        node_id: page_id,
                        kind: PAGE_KIND.to_string(),
                    },
                )
                .await?;
        }
        // Site symbol catalog travels with every body save: the document's
        // `flySymbols` block is the editing surface and replaces the stored
        // catalog for this (tenant, locale) as a full set.
        super::symbols::sync_site_symbols_in_tx(
            &txn,
            tenant_id,
            page_id,
            &body.locale,
            &body.content,
        )
        .await?;
        txn.commit().await?;

        self.get_with_locale_fallback(
            tenant_id,
            security,
            page_id,
            &response_locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await
    }

    /// Restores one journaled body revision into the locale's working copy.
    #[instrument(skip(self, input))]
    pub async fn restore_body_revision(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: RestorePageBodyRevisionInput,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;

        let txn = self.db.begin().await?;
        let locked_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        Self::ensure_builder_enabled_in_tx(&txn, tenant_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            locked_page.author_id,
        )?;

        let revision = page_body_revision::Entity::find_by_id(input.revision_id)
            .filter(page_body_revision::Column::TenantId.eq(tenant_id))
            .filter(page_body_revision::Column::PageId.eq(page_id))
            .one(&txn)
            .await?
            .ok_or_else(|| body_revision_not_found(input.revision_id))?;

        let translation_exists = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(&revision.locale))
            .one(&txn)
            .await?
            .is_some();
        if !translation_exists {
            return Err(PagesError::validation(format!(
                "Page body revision locale `{}` requires a matching page translation",
                revision.locale
            )));
        }

        let now = body_revision_timestamp(Utc::now());
        let restored = PreparedPageBody {
            locale: revision.locale.clone(),
            content: revision.content.clone(),
            format: revision.format.clone(),
        };
        if locked_page.status == "published" {
            let draft = self
                .find_draft_in_tx(&txn, tenant_id, page_id, &revision.locale)
                .await?;
            let current = self
                .find_body_in_tx(&txn, tenant_id, page_id, &revision.locale)
                .await?;
            let working = draft
                .as_ref()
                .map(WorkingBody::from)
                .or_else(|| current.as_ref().map(WorkingBody::from));
            let actual_revision = page_document_revision(page_id, working.as_ref());
            if input.expected_revision != actual_revision {
                return Err(document_revision_conflict(
                    input.expected_revision,
                    actual_revision,
                ));
            }

            let draft = self
                .upsert_draft_in_tx(&txn, tenant_id, page_id, &restored, now, security.user_id)
                .await?;
            let body_revision = draft.updated_at.to_string();
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                page_id,
                &draft.locale,
                &draft.content,
                &draft.format,
                PageBodyRevisionSource::Restore,
                &body_revision,
                security.user_id,
            )
            .await?;
        } else {
            let existing = self
                .find_body_in_tx(&txn, tenant_id, page_id, &revision.locale)
                .await?;
            let working = existing.as_ref().map(WorkingBody::from);
            let actual_revision = page_document_revision(page_id, working.as_ref());
            if input.expected_revision != actual_revision {
                return Err(document_revision_conflict(
                    input.expected_revision,
                    actual_revision,
                ));
            }

            self.upsert_body_in_tx(&txn, tenant_id, page_id, Some(restored.clone()), now)
                .await?;
            let body_revision = now.to_string();
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                page_id,
                &revision.locale,
                &revision.content,
                &revision.format,
                PageBodyRevisionSource::Restore,
                &body_revision,
                security.user_id,
            )
            .await?;

            let mut page_active: page::ActiveModel = locked_page.into();
            page_active.updated_at = Set(now);
            page_active.update(&txn).await?;

            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::NodeUpdated {
                        node_id: page_id,
                        kind: PAGE_KIND.to_string(),
                    },
                )
                .await?;
        }
        // Restoring a page body must not roll back site-wide definitions from
        // an older journal entry. The next editor read overlays the live store.
        txn.commit().await?;

        self.get_with_locale_fallback(
            tenant_id,
            security,
            page_id,
            &revision.locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await
    }

    /// Returns the newest body-revision journal metadata for one locale, newest first.
    #[instrument(skip(self))]
    pub async fn body_revision_history(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<Vec<PageBodyRevisionResponse>> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, page.author_id)?;
        let locale = normalize_locale(locale)?;
        let revisions = page_body_revision::Entity::find()
            .filter(page_body_revision::Column::TenantId.eq(tenant_id))
            .filter(page_body_revision::Column::PageId.eq(page_id))
            .filter(page_body_revision::Column::Locale.eq(&locale))
            .order_by_desc(page_body_revision::Column::CreatedAt)
            .order_by_desc(page_body_revision::Column::Id)
            .limit(MAX_PAGE_BODY_REVISIONS_RETURNED)
            .all(&self.db)
            .await?;
        Ok(revisions
            .into_iter()
            .map(|revision| PageBodyRevisionResponse {
                id: revision.id,
                locale: revision.locale,
                source: PageBodyRevisionSource::parse(&revision.source)
                    .unwrap_or(PageBodyRevisionSource::Save),
                body_revision: revision.body_revision,
                created_at: revision.created_at.to_string(),
                created_by: revision.created_by,
            })
            .collect())
    }
}

/// Working-copy revision token: the copy's `updated_at`, or `page:<page_id>:initial`
/// while the locale has no working copy yet.
pub(crate) fn page_document_revision(page_id: Uuid, working: Option<&WorkingBody>) -> String {
    working
        .map(|working| working.updated_at.to_string())
        .unwrap_or_else(|| format!("page:{page_id}:initial"))
}

pub(super) fn document_revision_conflict(expected: String, actual: String) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Conflict,
            format!(
                "Page document changed concurrently: expected revision `{expected}`, found `{actual}`"
            ),
        )
        .with_user_message(
            "The visual document changed while you were editing it. Reload the latest document and retry.",
        )
        .with_field("expected_revision", expected)
        .with_field("actual_revision", actual)
        .with_error_code(PAGE_DOCUMENT_REVISION_CONFLICT),
    ))
}

pub(super) fn body_revision_not_found(revision_id: Uuid) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::NotFound,
            format!("Page body revision `{revision_id}` does not exist for this page"),
        )
        .with_user_message("The requested body revision no longer exists.")
        .with_field("revision_id", revision_id.to_string())
        .with_error_code(PAGE_BODY_REVISION_NOT_FOUND),
    ))
}
