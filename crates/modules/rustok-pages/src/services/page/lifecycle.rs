use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_core::{
    SecurityContext,
    error::{ErrorKind, RichError},
};
use rustok_events::DomainEvent;
use rustok_tenant::TenantService;

use crate::dto::PageResponse;
use crate::entities::{
    page, page_body, page_body_draft, page_body_revision, page_lifecycle_operation_receipt,
    page_translation,
};
use crate::error::{
    FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED, FEATURE_BUILDER_PROPERTIES_ENABLED,
    PAGE_LIFECYCLE_IDEMPOTENCY_CONFLICT, PAGE_LIFECYCLE_OPERATION_INTEGRITY, PagesError,
    PagesResult,
};
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

use super::document::document_revision_conflict;
use super::helpers::{
    apply_transition, collect_builder_sources, enforce_expected_version, is_builder_enabled,
    is_builder_preview_enabled, is_builder_properties_enabled, next_page_version, transition_event,
};
use super::route::{record_delete_route_tombstones_in_tx, record_published_route_snapshots_in_tx};
use super::{PAGE_KIND, PageService, PageTransition, WorkingBody};

pub const PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED: &str = "PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED";

type BodyRevisionSnapshot = Vec<(String, String)>;

const PAGE_LIFECYCLE_UNPUBLISH: &str = "unpublish";
const PAGE_LIFECYCLE_ARCHIVE: &str = "archive";
const PAGE_LIFECYCLE_DELETE: &str = "delete";

#[derive(Clone)]
struct PageLifecycleReceiptRequest {
    operation: &'static str,
    idempotency_key: String,
    request_hash: String,
}

fn normalize_page_lifecycle_key(raw: &str) -> PagesResult<String> {
    if raw.is_empty()
        || raw.len() > 128
        || !raw.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
    {
        return Err(PagesError::validation(
            "Page lifecycle idempotency key must be 1..128 ASCII letters, digits, '-' '_' '.' or ':'",
        ));
    }
    Ok(raw.to_string())
}

fn page_lifecycle_request_hash(
    operation: &'static str,
    tenant_id: Uuid,
    page_id: Uuid,
    actor_id: Option<Uuid>,
    expected_version: Option<i32>,
) -> PagesResult<String> {
    let bytes = serde_json::to_vec(&(
        "page-lifecycle-v1",
        operation,
        tenant_id,
        page_id,
        actor_id,
        expected_version,
    ))
    .map_err(|error| {
        PagesError::validation(format!("Cannot hash page lifecycle request: {error}"))
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn page_lifecycle_receipt_request(
    operation: &'static str,
    tenant_id: Uuid,
    page_id: Uuid,
    actor_id: Option<Uuid>,
    expected_version: Option<i32>,
    idempotency_key: String,
) -> PagesResult<PageLifecycleReceiptRequest> {
    Ok(PageLifecycleReceiptRequest {
        operation,
        idempotency_key: normalize_page_lifecycle_key(&idempotency_key)?,
        request_hash: page_lifecycle_request_hash(
            operation,
            tenant_id,
            page_id,
            actor_id,
            expected_version,
        )?,
    })
}

fn page_lifecycle_idempotency_conflict() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Conflict,
            "A page lifecycle idempotency key was reused for a different request or actor",
        )
        .with_user_message(
            "This lifecycle request key is already bound to another operation. Start a new operation.",
        )
        .with_error_code(PAGE_LIFECYCLE_IDEMPOTENCY_CONFLICT),
    ))
}

fn verify_page_lifecycle_receipt(
    receipt: &page_lifecycle_operation_receipt::Model,
    request: &PageLifecycleReceiptRequest,
    tenant_id: Uuid,
    page_id: Uuid,
    actor_id: Option<Uuid>,
) -> PagesResult<()> {
    if receipt.tenant_id != tenant_id
        || receipt.page_id != page_id
        || receipt.operation != request.operation
        || receipt.actor_id != actor_id
        || receipt.idempotency_key != request.idempotency_key
        || receipt.request_hash != request.request_hash
    {
        return Err(page_lifecycle_idempotency_conflict());
    }
    Ok(())
}

fn page_lifecycle_response_from_receipt(
    receipt: &page_lifecycle_operation_receipt::Model,
    request: &PageLifecycleReceiptRequest,
    tenant_id: Uuid,
    page_id: Uuid,
    actor_id: Option<Uuid>,
) -> PagesResult<PageResponse> {
    verify_page_lifecycle_receipt(receipt, request, tenant_id, page_id, actor_id)?;
    let response: PageResponse = serde_json::from_str(&receipt.response_json).map_err(|_| {
        PagesError::Rich(Box::new(
            RichError::new(
                ErrorKind::Internal,
                "Persisted page lifecycle receipt contains an invalid response",
            )
            .with_error_code(PAGE_LIFECYCLE_OPERATION_INTEGRITY),
        ))
    })?;
    if response.id != page_id {
        return Err(PagesError::Rich(Box::new(
            RichError::new(
                ErrorKind::Internal,
                "Persisted page lifecycle receipt does not match its page identity",
            )
            .with_error_code(PAGE_LIFECYCLE_OPERATION_INTEGRITY),
        )));
    }
    Ok(response)
}

fn page_delete_response_from_receipt(
    receipt: &page_lifecycle_operation_receipt::Model,
    request: &PageLifecycleReceiptRequest,
    tenant_id: Uuid,
    page_id: Uuid,
    actor_id: Option<Uuid>,
) -> PagesResult<()> {
    verify_page_lifecycle_receipt(receipt, request, tenant_id, page_id, actor_id)?;
    let deleted: bool = serde_json::from_str(&receipt.response_json).map_err(|_| {
        PagesError::Rich(Box::new(
            RichError::new(
                ErrorKind::Internal,
                "Persisted page delete receipt contains an invalid response",
            )
            .with_error_code(PAGE_LIFECYCLE_OPERATION_INTEGRITY),
        ))
    })?;
    if !deleted {
        return Err(PagesError::Rich(Box::new(
            RichError::new(
                ErrorKind::Internal,
                "Persisted page delete receipt does not acknowledge deletion",
            )
            .with_error_code(PAGE_LIFECYCLE_OPERATION_INTEGRITY),
        )));
    }
    Ok(())
}

async fn find_page_lifecycle_receipt<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    page_id: Uuid,
    request: &PageLifecycleReceiptRequest,
) -> PagesResult<Option<page_lifecycle_operation_receipt::Model>> {
    Ok(page_lifecycle_operation_receipt::Entity::find()
        .filter(page_lifecycle_operation_receipt::Column::TenantId.eq(tenant_id))
        .filter(page_lifecycle_operation_receipt::Column::PageId.eq(page_id))
        .filter(page_lifecycle_operation_receipt::Column::Operation.eq(request.operation))
        .filter(
            page_lifecycle_operation_receipt::Column::IdempotencyKey
                .eq(&request.idempotency_key),
        )
        .one(conn)
        .await?)
}

async fn insert_page_lifecycle_receipt(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    owner_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    request: &PageLifecycleReceiptRequest,
    response_json: String,
) -> PagesResult<()> {
    page_lifecycle_operation_receipt::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        owner_id: Set(owner_id),
        operation: Set(request.operation.to_string()),
        actor_id: Set(actor_id),
        idempotency_key: Set(request.idempotency_key.clone()),
        request_hash: Set(request.request_hash.clone()),
        response_json: Set(response_json),
        created_at: Set(Utc::now().into()),
    }
    .insert(txn)
    .await?;
    Ok(())
}

impl PageService {
    #[instrument(skip(self, security))]
    pub async fn publish_non_builder(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.publish_non_builder_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn publish_non_builder_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;

        let bodies = self.load_working_bodies(tenant_id, page_id).await?;
        if !collect_builder_sources(&bodies, None, true).is_empty() {
            return Err(builder_reviewed_publish_required());
        }
        let body_revisions = body_revision_snapshot(&bodies);

        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Publish,
            Some(observed.version),
            Some(body_revisions),
            None,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.unpublish_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let expected_version = if let Some(expected_version) = expected_version {
            Some(expected_version)
        } else {
            let observed = self.find_page(tenant_id, page_id).await?;
            enforce_owned_scope(
                &security,
                Resource::Pages,
                Action::Publish,
                observed.author_id,
            )?;
            Some(observed.version)
        };
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Unpublish,
            expected_version,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish_with_receipt(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
        idempotency_key: String,
    ) -> PagesResult<PageResponse> {
        let request = page_lifecycle_receipt_request(
            PAGE_LIFECYCLE_UNPUBLISH,
            tenant_id,
            page_id,
            security.user_id,
            expected_version,
            idempotency_key,
        )?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Unpublish,
            expected_version,
            None,
            Some(request),
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.archive_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let expected_version = if let Some(expected_version) = expected_version {
            Some(expected_version)
        } else {
            let observed = self.find_page(tenant_id, page_id).await?;
            enforce_owned_scope(
                &security,
                Resource::Pages,
                Action::Publish,
                observed.author_id,
            )?;
            Some(observed.version)
        };
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Archive,
            expected_version,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive_with_receipt(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
        idempotency_key: String,
    ) -> PagesResult<PageResponse> {
        let request = page_lifecycle_receipt_request(
            PAGE_LIFECYCLE_ARCHIVE,
            tenant_id,
            page_id,
            security.user_id,
            expected_version,
            idempotency_key,
        )?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Archive,
            expected_version,
            None,
            Some(request),
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_preview_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_preview_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PREVIEW_ENABLED,
            ));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_properties_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_properties_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PROPERTIES_ENABLED,
            ));
        }
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        self.delete_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self, security))]
    #[instrument(skip(self, security))]
    pub async fn delete_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<()> {
        self.delete_inner(tenant_id, security, page_id, expected_version, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn delete_with_receipt(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
        idempotency_key: String,
    ) -> PagesResult<()> {
        let request = page_lifecycle_receipt_request(
            PAGE_LIFECYCLE_DELETE,
            tenant_id,
            page_id,
            security.user_id,
            expected_version,
            idempotency_key,
        )?;
        self.delete_inner(
            tenant_id,
            security,
            page_id,
            expected_version,
            Some(request),
        )
        .await
    }

    async fn delete_inner(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
        receipt_request: Option<PageLifecycleReceiptRequest>,
    ) -> PagesResult<()> {
        enforce_scope(&security, Resource::Pages, Action::Delete)?;
        let txn = self.begin_page_write_transaction().await?;
        if let Some(request) = receipt_request.as_ref()
            && let Some(receipt) =
                find_page_lifecycle_receipt(&txn, tenant_id, page_id, request).await?
        {
            enforce_owned_scope(
                &security,
                Resource::Pages,
                Action::Delete,
                receipt.owner_id,
            )?;
            page_delete_response_from_receipt(
                &receipt,
                request,
                tenant_id,
                page_id,
                security.user_id,
            )?;
            txn.commit().await?;
            return Ok(());
        }

        let existing = match self.find_page_for_update(&txn, tenant_id, page_id).await {
            Ok(existing) => existing,
            Err(error) if receipt_request.is_some() => {
                // A concurrent delete may remove the row (or a stricter isolation level may
                // abort the locking read) while this request waits. Re-read the receipt in a
                // fresh transaction before surfacing the original failure.
                txn.rollback().await?;
                let request = receipt_request
                    .as_ref()
                    .expect("receipt request checked above");
                if let Some(receipt) =
                    find_page_lifecycle_receipt(&self.db, tenant_id, page_id, request).await?
                {
                    enforce_owned_scope(
                        &security,
                        Resource::Pages,
                        Action::Delete,
                        receipt.owner_id,
                    )?;
                    page_delete_response_from_receipt(
                        &receipt,
                        request,
                        tenant_id,
                        page_id,
                        security.user_id,
                    )?;
                    return Ok(());
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            existing.author_id,
        )?;
        enforce_expected_version(expected_version, existing.version)?;
        if existing.status == "published" {
            return Err(PagesError::cannot_delete_published());
        }

        let resource_revision = i64::from(next_page_version(page_id, existing.version)?);
        record_delete_route_tombstones_in_tx(&txn, tenant_id, page_id).await?;
        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id,
                resource_revision,
                operation: "delete",
                lifecycle: "deleted",
            },
        )
        .await?;

        page_body::Entity::delete_many()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_body_draft::Entity::delete_many()
            .filter(page_body_draft::Column::TenantId.eq(tenant_id))
            .filter(page_body_draft::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_body_revision::Entity::delete_many()
            .filter(page_body_revision::Column::TenantId.eq(tenant_id))
            .filter(page_body_revision::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_translation::Entity::delete_many()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page::Entity::delete_many()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .exec(&txn)
            .await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeDeleted {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;

        if let Some(request) = receipt_request.as_ref() {
            insert_page_lifecycle_receipt(
                &txn,
                tenant_id,
                page_id,
                existing.author_id,
                security.user_id,
                request,
                "true".to_string(),
            )
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn transition_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        transition: PageTransition,
        expected_version: Option<i32>,
        expected_body_revisions: Option<BodyRevisionSnapshot>,
        receipt_request: Option<PageLifecycleReceiptRequest>,
    ) -> PagesResult<PageResponse> {
        let txn = self.begin_page_write_transaction().await?;
        let existing = match self.find_page_for_update(&txn, tenant_id, page_id).await {
            Ok(existing) => existing,
            Err(error) if receipt_request.is_some() => {
                txn.rollback().await?;
                let request = receipt_request
                    .as_ref()
                    .expect("receipt request checked above");
                if let Some(receipt) =
                    find_page_lifecycle_receipt(&self.db, tenant_id, page_id, request).await?
                {
                    enforce_owned_scope(
                        &security,
                        Resource::Pages,
                        Action::Publish,
                        receipt.owner_id,
                    )?;
                    return page_lifecycle_response_from_receipt(
                        &receipt,
                        request,
                        tenant_id,
                        page_id,
                        security.user_id,
                    );
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing.author_id,
        )?;
        if let Some(request) = receipt_request.as_ref()
            && let Some(receipt) =
                find_page_lifecycle_receipt(&txn, tenant_id, page_id, request).await?
        {
            if receipt.owner_id != existing.author_id {
                return Err(page_lifecycle_idempotency_conflict());
            }
            let response = page_lifecycle_response_from_receipt(
                &receipt,
                request,
                tenant_id,
                page_id,
                security.user_id,
            )?;
            txn.commit().await?;
            return Ok(response);
        }
        // Receipt replay must precede CAS: a retry after a lost response observes
        // the incremented page version but still belongs to the original transition.
        enforce_expected_version(expected_version, existing.version)?;

        if transition == PageTransition::Publish {
            let current_bodies = self
                .load_working_bodies_in_tx(&txn, tenant_id, page_id)
                .await?;
            if !collect_builder_sources(&current_bodies, None, true).is_empty() {
                return Err(builder_reviewed_publish_required());
            }
            let current_revisions = body_revision_snapshot(&current_bodies);
            let expected_revisions = expected_body_revisions.unwrap_or_default();
            if current_revisions != expected_revisions {
                return Err(document_revision_conflict(
                    format_body_revisions(&expected_revisions),
                    format_body_revisions(&current_revisions),
                ));
            }
        } else {
            record_published_route_snapshots_in_tx(&txn, tenant_id, page_id, &existing.status)
                .await?;
        }

        // Unpublish/archive fold the unpublished working copy into the current body so the
        // page keeps exactly one editable copy after the transition.
        self.promote_drafts_in_tx(&txn, tenant_id, page_id, security.user_id)
            .await?;

        let now = Utc::now();
        let next_version = next_page_version(page_id, existing.version)?;
        let lifecycle = if transition == PageTransition::Archive {
            "archived"
        } else {
            "active"
        };
        let mut active: page::ActiveModel = existing.into();
        active.updated_at = Set(now.into());
        active.version = Set(next_version);
        apply_transition(&mut active, Some(transition), now);
        let transitioned_page = active.update(&txn).await?;

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id,
                resource_revision: i64::from(transitioned_page.version),
                operation: "lifecycle",
                lifecycle,
            },
        )
        .await?;

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
        if let Some(event) = transition_event(Some(transition), page_id) {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }

        let response = self
            .get_page_response_for_authorized_mutation_in_tx(
                &txn,
                tenant_id,
                &security,
                page_id,
                PLATFORM_FALLBACK_LOCALE,
            )
            .await?;
        if let Some(request) = receipt_request.as_ref() {
            let response_json = serde_json::to_string(&response).map_err(|error| {
                PagesError::validation(format!(
                    "Cannot serialize page lifecycle receipt: {error}"
                ))
            })?;
            insert_page_lifecycle_receipt(
                &txn,
                tenant_id,
                page_id,
                transitioned_page.author_id,
                security.user_id,
                request,
                response_json,
            )
            .await?;
        }
        txn.commit().await?;
        Ok(response)
    }

    pub(super) async fn ensure_builder_enabled_in_tx(
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module =
            TenantService::find_tenant_module_locked_in_tx(txn, tenant_id, "pages").await?;
        let enabled = module
            .as_ref()
            .map(|module| is_builder_enabled(&module.settings))
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_ENABLED));
        }
        Ok(())
    }

    pub(super) async fn ensure_builder_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module.as_ref().map(is_builder_enabled).unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_ENABLED));
        }
        Ok(())
    }

    async fn load_tenant_pages_module(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<Option<serde_json::Value>> {
        TenantService::new(self.db.clone())
            .find_tenant_module(tenant_id, "pages")
            .await
            .map(|module| module.map(|module| module.settings))
            .map_err(Into::into)
    }
}

fn builder_reviewed_publish_required() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::BusinessLogic,
            "Page Builder documents cannot use the non-builder publish lifecycle",
        )
        .with_user_message(
            "Review a promoted Page Builder runtime scenario and use the atomic publish command.",
        )
        .with_error_code(PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED),
    ))
}

fn body_revision_snapshot(bodies: &[WorkingBody]) -> BodyRevisionSnapshot {
    let mut revisions = bodies
        .iter()
        .map(|body| {
            let digest = Sha256::digest(format!("{}\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{}", body.updated_at, encode_digest(&digest)),
            )
        })
        .collect::<Vec<_>>();
    revisions.sort();
    revisions
}

fn encode_digest(digest: &[u8]) -> String {
    hex::encode(digest)
}

fn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {
    revisions
        .iter()
        .map(|(locale, revision)| format!("{locale}:{revision}"))
        .collect::<Vec<_>>()
        .join(",")
}
