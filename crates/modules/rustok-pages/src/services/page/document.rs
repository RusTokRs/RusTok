use chrono::Utc;
use sha2::{Digest, Sha256};
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
use crate::entities::{page, page_body_revision, page_document_save_receipt, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::{can_read_page_source, enforce_owned_scope};

use super::helpers::{next_body_revision_timestamp, normalize_locale, normalize_page_body_input};
use super::{PAGE_KIND, PageService, PreparedPageBody, WorkingBody};

pub const PAGE_DOCUMENT_REVISION_CONFLICT: &str = "PAGE_DOCUMENT_REVISION_CONFLICT";
pub const PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT: &str = "PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT";
pub const PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED: &str = "PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED";
const MAX_REPLAYABLE_SAVE_RECEIPTS_PER_PAGE: u64 = 64;
const MAX_SAVE_RECEIPT_KEY_BYTES: usize = 128;
pub const PAGE_BODY_REVISION_NOT_FOUND: &str = "PAGE_BODY_REVISION_NOT_FOUND";

/// Newest body revisions returned by one history read.
pub const MAX_PAGE_BODY_REVISIONS_RETURNED: u64 = 200;

impl PageService {
    /// Saves one localized builder document into the page's working copy.
    ///
    /// While the page is published the working copy is the page body draft and the public body
    /// stays untouched; every other lifecycle state updates the current body directly. Both
    /// paths append one body-revision journal row.
    #[instrument(skip(self, input, security))]
    pub async fn save_document(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: SavePageDocumentInput,
    ) -> PagesResult<PageResponse> {
        self.save_document_with_receipt(tenant_id, security, page_id, input, None).await
    }

    /// A durable, caller-identified save. Repeating the same key and normalized
    /// payload returns the original acknowledgement, even after another editor
    /// changed the page. Reusing a key with different content fails closed.
    ///
    /// Callers must generate one stable key for each *attempt* and retain it
    /// across network retries; fresh edits must receive a new key.
    #[instrument(skip(self, input, security))]
    pub async fn save_document_with_receipt(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: SavePageDocumentInput,
        idempotency_key: Option<String>,
    ) -> PagesResult<PageResponse> {
        let idempotency_key = idempotency_key
            .as_deref()
            .map(normalize_document_save_key)
            .transpose()?;
        let mut body = normalize_page_body_input(Some(input.body))?
            .ok_or_else(|| PagesError::validation("Page document body is required"))?;
        if body.format != PAGE_BUILDER_DOCUMENT_FORMAT {
            return Err(PagesError::validation(
                "Page document save accepts only the current Fly/GrapesJS body format",
            ));
        }
        body.content = super::layout::strip_layout_nodes(&body.content)?;
        let request_hash = idempotency_key
            .as_ref()
            .map(|_| document_save_request_hash(
                tenant_id,
                page_id,
                &input.expected_revision,
                &body,
            ))
            .transpose()?;
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
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            locked_page.author_id,
        )?;

        // Replays are resolved *before* the current revision check, otherwise a
        // write that committed but lost its HTTP reply would always conflict.
        if let (Some(key), Some(hash)) = (idempotency_key.as_deref(), request_hash.as_deref()) {
            if let Some(receipt) = find_document_save_receipt_in_tx(
                &txn, tenant_id, page_id, key,
            ).await? {
                verify_document_save_receipt(&receipt, security.user_id, hash)?;
                let result = page_response_from_receipt(&receipt)?;
                txn.commit().await?;
                return Ok(result);
            }
        }

        // Replaying a committed receipt is read-only and must remain available
        // even if the tenant disables new Page Builder writes after the commit.
        Self::ensure_builder_enabled_in_tx(&txn, tenant_id).await?;

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
            // The row lock protects the CAS; a repeated or regressed wall clock
            // must not make an already consumed revision token valid again.
            let now = next_body_revision_timestamp(
                Utc::now(),
                working.as_ref().map(|body| &body.updated_at),
            )?;

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
            // The row lock protects the CAS; a repeated or regressed wall clock
            // must not make an already consumed revision token valid again.
            let now = next_body_revision_timestamp(
                Utc::now(),
                working.as_ref().map(|body| &body.updated_at),
            )?;

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
        // Construct the acknowledgement before a competing writer can commit.
        let response = self
            .get_with_locale_fallback_in_tx(
                &txn,
                tenant_id,
                &security,
                page_id,
                &response_locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        if let (Some(key), Some(hash)) = (idempotency_key, request_hash) {
            let result_revision = response.body.as_ref().ok_or_else(|| {
                PagesError::validation("Accepted document save has no authoring body")
            })?.updated_at.clone();
            let response_json = serde_json::to_string(&response).map_err(|error| {
                PagesError::validation(format!("Cannot serialize page document save receipt: {error}"))
            })?;
            page_document_save_receipt::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                page_id: Set(page_id),
                actor_id: Set(security.user_id),
                idempotency_key: Set(key),
                request_hash: Set(hash),
                result_revision: Set(result_revision),
                response_json: Set(Some(response_json)),
                created_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;
            // Retain permanent tiny key/hash tombstones while bounding the
            // number of *full document snapshots* retained per page.
            expire_old_document_save_receipts_in_tx(&txn, tenant_id, page_id).await?;
        }
        txn.commit().await?;
        Ok(response)
    }


    /// Lookup an already committed save by its stable ID without replaying the mutation.
    /// A missing receipt is not proof that an in-flight request failed.
    #[instrument(skip(self, security))]
    pub async fn document_save_receipt(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        idempotency_key: &str,
    ) -> PagesResult<Option<PageResponse>> {
        let key = normalize_document_save_key(idempotency_key)?;
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            page.author_id,
        )?;
        let receipt = page_document_save_receipt::Entity::find()
            .filter(page_document_save_receipt::Column::TenantId.eq(tenant_id))
            .filter(page_document_save_receipt::Column::PageId.eq(page_id))
            .filter(page_document_save_receipt::Column::IdempotencyKey.eq(&key))
            .one(&self.db)
            .await?;
        match receipt {
            Some(receipt) => {
                if receipt.actor_id != security.user_id {
                    return Err(document_save_receipt_error(
                        PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT,
                        "A document save idempotency key belongs to a different actor",
                    ));
                }
                Ok(Some(page_response_from_receipt(&receipt)?))
            }
            None => Ok(None),
        }
    }

    /// Restores one journaled body revision into the locale's working copy.
    #[instrument(skip(self, input, security))]
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
            // The row lock protects the CAS; a repeated or regressed wall clock
            // must not make an already consumed revision token valid again.
            let now = next_body_revision_timestamp(
                Utc::now(),
                working.as_ref().map(|body| &body.updated_at),
            )?;

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
            // The row lock protects the CAS; a repeated or regressed wall clock
            // must not make an already consumed revision token valid again.
            let now = next_body_revision_timestamp(
                Utc::now(),
                working.as_ref().map(|body| &body.updated_at),
            )?;

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
        // Construct the acknowledgement before a competing writer can commit.
        let response = self
            .get_with_locale_fallback_in_tx(
                &txn,
                tenant_id,
                &security,
                page_id,
                &revision.locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        txn.commit().await?;
        Ok(response)
    }

    /// Returns the newest body-revision journal metadata for one locale, newest first.
    ///
    /// The journal describes the document, so it is readable by whoever may read the document
    /// ([`can_read_page_source`]): a principal that may save or publish it. Restoring a
    /// revision is a save and keeps requiring `pages:update`.
    #[instrument(skip(self, security))]
    pub async fn body_revision_history(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<Vec<PageBodyRevisionResponse>> {
        let page = self.find_page(tenant_id, page_id).await?;
        if !can_read_page_source(&security, page.author_id) {
            return Err(PagesError::forbidden("Permission denied"));
        }
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


fn normalize_document_save_key(raw: &str) -> PagesResult<String> {
    if raw.is_empty()
        || raw.len() > MAX_SAVE_RECEIPT_KEY_BYTES
        || !raw.bytes().all(|byte| byte.is_ascii_alphanumeric() ||
            matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(PagesError::validation(
            "Document save idempotency key must be 1..128 ASCII letters, digits, '-' '_' '.' or ':'",
        ));
    }
    Ok(raw.to_string())
}

fn document_save_request_hash(
    tenant_id: Uuid,
    page_id: Uuid,
    expected_revision: &str,
    body: &PreparedPageBody,
) -> PagesResult<String> {
    // Hash the normalized, layout-stripped persisted command, not raw JSON
    // supplied by one client: semantically equivalent encodings replay.
    let bytes = serde_json::to_vec(&(
        "page-document-save-v1",
        tenant_id,
        page_id,
        expected_revision,
        &body.locale,
        &body.format,
        &body.content,
    ))
    .map_err(|error| PagesError::validation(format!("Cannot hash document save: {error}")))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

async fn find_document_save_receipt_in_tx(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    key: &str,
) -> PagesResult<Option<page_document_save_receipt::Model>> {
    Ok(page_document_save_receipt::Entity::find()
        .filter(page_document_save_receipt::Column::TenantId.eq(tenant_id))
        .filter(page_document_save_receipt::Column::PageId.eq(page_id))
        .filter(page_document_save_receipt::Column::IdempotencyKey.eq(key))
        .one(txn)
        .await?)
}

fn verify_document_save_receipt(
    receipt: &page_document_save_receipt::Model,
    actor_id: Option<Uuid>,
    expected_hash: &str,
) -> PagesResult<()> {
    if receipt.request_hash != expected_hash || receipt.actor_id != actor_id {
        return Err(document_save_receipt_error(
            PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT,
            "A document save idempotency key was reused for a different request or actor",
        ));
    }
    Ok(())
}

fn page_response_from_receipt(
    receipt: &page_document_save_receipt::Model,
) -> PagesResult<PageResponse> {
    let encoded = receipt.response_json.as_deref().ok_or_else(|| {
        document_save_receipt_error(
            PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED,
            "The saved operation is known but its full response has expired; reload the current document",
        )
    })?;
    let response: PageResponse = serde_json::from_str(encoded).map_err(|_| {
        PagesError::validation("Persisted page document save receipt is invalid")
    })?;
    if response.id != receipt.page_id ||
        response.body.as_ref().map(|body| body.updated_at.as_str())
            != Some(receipt.result_revision.as_str())
    {
        return Err(PagesError::validation(
            "Persisted page document save receipt is inconsistent with its result revision",
        ));
    }
    Ok(response)
}

fn document_save_receipt_error(code: &'static str, detail: &str) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Conflict, format!("{code}: {detail}"))
            .with_user_message(detail)
            .with_field("error_code", code)
            .with_error_code(code),
    ))
}

async fn expire_old_document_save_receipts_in_tx(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<()> {
    let to_expire = page_document_save_receipt::Entity::find()
        .filter(page_document_save_receipt::Column::TenantId.eq(tenant_id))
        .filter(page_document_save_receipt::Column::PageId.eq(page_id))
        .filter(page_document_save_receipt::Column::ResponseJson.is_not_null())
        .order_by_desc(page_document_save_receipt::Column::CreatedAt)
        .order_by_desc(page_document_save_receipt::Column::Id)
        .offset(MAX_REPLAYABLE_SAVE_RECEIPTS_PER_PAGE)
        .all(txn)
        .await?;
    for old in to_expire {
        let mut model: page_document_save_receipt::ActiveModel = old.into();
        model.response_json = Set(None);
        model.update(txn).await?;
    }
    Ok(())
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
                "Page document changed concurrently ({PAGE_DOCUMENT_REVISION_CONFLICT}): expected revision `{expected}`, found `{actual}`"
            ),
        )
        .with_user_message(
            "The visual document changed while you were editing it. Reload the latest document and retry.",
        )
        .with_field("error_code", PAGE_DOCUMENT_REVISION_CONFLICT)
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
