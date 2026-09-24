use std::{collections::HashMap, sync::Arc, time::Instant};

use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::SecurityContext;
use rustok_media::{
    MediaAssetReferenceListPage, MediaAssetReferenceListRequest, MediaAssetReadPort,
};
use sea_orm::{
    AccessMode, ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, IsolationLevel,
    QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use super::rbac::enforce_scope;
use crate::{
    entities::forum_attachment_relation,
    error::{ForumError, ForumResult},
};

pub const DEFAULT_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT: u64 = 100;
pub const MAX_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT: u64 = 500;
const FORUM_MEDIA_OWNER_MODULE: &str = "forum";
const FORUM_ATTACHMENT_HOLD_RECONCILIATION_OPERATION: &str =
    "forum.attachment_hold_reconciliation_report";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumAttachmentHoldDriftKind {
    OrphanMediaHold,
    MediaReferenceMismatch,
}

impl ForumAttachmentHoldDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrphanMediaHold => "orphan_media_hold",
            Self::MediaReferenceMismatch => "media_reference_mismatch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ForumAttachmentHoldDrift {
    pub kind: ForumAttachmentHoldDriftKind,
    pub reference_id: Uuid,
    pub media_id: Uuid,
    pub relation_media_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ForumAttachmentHoldReconciliationReport {
    pub requested_limit: Option<u64>,
    pub effective_limit: u64,
    pub inspected_media_holds: u64,
    pub has_more_media_holds: bool,
    pub media_cursor: Option<Uuid>,
    pub drifts: Vec<ForumAttachmentHoldDrift>,
}

impl ForumAttachmentHoldReconciliationReport {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only FORUM-33 reconciliation of Media holds owned by Forum.
///
/// Media remains authoritative for the hold rows. Forum obtains a bounded owner-scoped page from
/// the public Media read port and checks every returned reference against Forum-owned attachment
/// relation rows. An existing reference ID with another Media asset is reported as a mismatch;
/// a Media reference with no Forum relation is reported as an orphan hold.
///
/// The two owner databases cannot provide a distributed snapshot in remote deployments. The Media
/// page and Forum lookup are therefore separate owner snapshots, and the result is explicitly
/// diagnostic rather than a serializable repair fence. No path here releases or retains a Media
/// reference.
pub struct ForumAttachmentHoldReconciliationService {
    db: DatabaseConnection,
    media: Arc<dyn MediaAssetReadPort>,
}

impl ForumAttachmentHoldReconciliationService {
    pub fn new(db: DatabaseConnection, media: Arc<dyn MediaAssetReadPort>) -> Self {
        Self { db, media }
    }

    pub async fn report_page(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        media_context: PortContext,
        requested_limit: Option<u64>,
        media_reference_after: Option<Uuid>,
    ) -> ForumResult<ForumAttachmentHoldReconciliationReport> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "attachment_hold_reconciliation_report",
            "library",
        );
        let started_at = Instant::now();
        let result = match enforce_operations_scope(security) {
            Ok(()) => {
                self.report_inner(
                    tenant_id,
                    media_context,
                    requested_limit,
                    media_reference_after,
                )
                .await
            }
            Err(error) => Err(error),
        };
        rustok_telemetry::metrics::record_span_duration(
            FORUM_ATTACHMENT_HOLD_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_ATTACHMENT_HOLD_RECONCILIATION_OPERATION,
                "owner_report",
            );
            rustok_telemetry::metrics::record_module_error(
                "forum",
                "attachment_hold_reconciliation",
                "error",
            );
        }
        result
    }

    async fn report_inner(
        &self,
        tenant_id: Uuid,
        media_context: PortContext,
        requested_limit: Option<u64>,
        media_reference_after: Option<Uuid>,
    ) -> ForumResult<ForumAttachmentHoldReconciliationReport> {
        validate_cursor(media_reference_after)?;
        validate_media_context_tenant(&media_context, tenant_id)?;

        let effective_limit = requested_limit
            .unwrap_or(DEFAULT_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT)
            .clamp(1, MAX_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT);

        let media_page = self
            .media
            .list_asset_references(
                media_context,
                MediaAssetReferenceListRequest {
                    owner_module: FORUM_MEDIA_OWNER_MODULE.to_string(),
                    after_reference_id: media_reference_after,
                    limit: effective_limit,
                },
            )
            .await
            .map_err(media_port_error)?;

        let backend = self.db.get_database_backend();
        let transaction = match backend {
            DatabaseBackend::Postgres => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::RepeatableRead),
                        Some(AccessMode::ReadOnly),
                    )
                    .await?
            }
            DatabaseBackend::Sqlite => self.db.begin().await?,
            other => {
                return Err(ForumError::Validation(format!(
                    "Forum attachment hold reconciliation does not support database backend {other:?}"
                )));
            }
        };

        let report = self
            .report_media_page_in_transaction(
                &transaction,
                tenant_id,
                media_page,
                requested_limit,
                effective_limit,
            )
            .await;

        match report {
            Ok(report) => {
                transaction.commit().await?;
                Ok(report)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn report_media_page_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        media_page: MediaAssetReferenceListPage,
        requested_limit: Option<u64>,
        effective_limit: u64,
    ) -> ForumResult<ForumAttachmentHoldReconciliationReport> {
        let references = media_page.references;
        for reference in &references {
            validate_media_reference(reference, tenant_id)?;
        }
        let inspected_media_holds = references.len() as u64;
        let reference_ids = references
            .iter()
            .map(|reference| reference.reference_id)
            .collect::<Vec<_>>();

        let mut relation_media_by_reference = HashMap::with_capacity(reference_ids.len());
        if !reference_ids.is_empty() {
            let rows = forum_attachment_relation::Entity::find()
                .filter(forum_attachment_relation::Column::TenantId.eq(tenant_id))
                .filter(forum_attachment_relation::Column::ReferenceId.is_in(reference_ids))
                .all(transaction)
                .await?;

            relation_media_by_reference.extend(
                rows.into_iter()
                    .map(|row| (row.reference_id, row.media_id)),
            );
        }

        let mut drifts = Vec::new();
        for reference in &references {
            match relation_media_by_reference.get(&reference.reference_id) {
                None => drifts.push(ForumAttachmentHoldDrift {
                    kind: ForumAttachmentHoldDriftKind::OrphanMediaHold,
                    reference_id: reference.reference_id,
                    media_id: reference.media_id,
                    relation_media_id: None,
                }),
                Some(&relation_media_id) if relation_media_id != reference.media_id => {
                    drifts.push(ForumAttachmentHoldDrift {
                        kind: ForumAttachmentHoldDriftKind::MediaReferenceMismatch,
                        reference_id: reference.reference_id,
                        media_id: reference.media_id,
                        relation_media_id: Some(relation_media_id),
                    });
                }
                Some(_) => {}
            }
        }

        Ok(ForumAttachmentHoldReconciliationReport {
            requested_limit,
            effective_limit,
            inspected_media_holds,
            has_more_media_holds: media_page.has_more,
            media_cursor: media_page.next_reference_id,
            drifts,
        })
    }
}

fn enforce_operations_scope(security: &SecurityContext) -> ForumResult<()> {
    enforce_scope(security, Resource::ForumCategories, Action::Manage)?;
    enforce_scope(security, Resource::ForumTopics, Action::Manage)
}

fn validate_media_reference(
    reference: &rustok_media::MediaAssetReference,
    tenant_id: Uuid,
) -> ForumResult<()> {
    if reference.tenant_id != tenant_id {
        return Err(ForumError::CapabilityFailure {
            capability: "media.asset_reference_reconciliation",
            source_code: "MEDIA_REFERENCE_TENANT_MISMATCH".to_string(),
            message: "Media returned an owner reference outside the trusted Forum tenant".to_string(),
            retryable: false,
        });
    }
    if reference.owner_module != FORUM_MEDIA_OWNER_MODULE {
        return Err(ForumError::CapabilityFailure {
            capability: "media.asset_reference_reconciliation",
            source_code: "MEDIA_REFERENCE_OWNER_MISMATCH".to_string(),
            message: "Media returned a reference outside the Forum owner scope".to_string(),
            retryable: false,
        });
    }
    if reference.reference_id.is_nil() || reference.media_id.is_nil() {
        return Err(ForumError::CapabilityFailure {
            capability: "media.asset_reference_reconciliation",
            source_code: "MEDIA_REFERENCE_IDENTITY_INVALID".to_string(),
            message: "Media returned an invalid durable reference identity".to_string(),
            retryable: false,
        });
    }
    Ok(())
}

fn validate_cursor(cursor: Option<Uuid>) -> ForumResult<()> {
    if cursor.is_some_and(|value| value.is_nil()) {
        return Err(ForumError::Validation(
            "Forum attachment hold reconciliation cursor must be a non-nil UUID".to_string(),
        ));
    }
    Ok(())
}

fn validate_media_context_tenant(context: &PortContext, tenant_id: Uuid) -> ForumResult<()> {
    let context_tenant = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(
            "Forum attachment hold reconciliation Media context requires a UUID tenant identity"
                .to_string(),
        )
    })?;
    if context_tenant != tenant_id {
        return Err(ForumError::Validation(
            "Forum attachment hold reconciliation Media context tenant does not match the trusted Forum tenant"
                .to_string(),
        ));
    }
    Ok(())
}

fn media_port_error(error: PortError) -> ForumError {
    ForumError::capability_failure(
        "forum.media.asset_reference_reconciliation",
        error.code,
        error.message,
        error.retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{PortActor, PortContext};

    #[test]
    fn cursor_rejects_nil_uuid() {
        assert!(validate_cursor(Some(Uuid::nil())).is_err());
        assert!(validate_cursor(Some(Uuid::new_v4())).is_ok());
        assert!(validate_cursor(None).is_ok());
    }

    #[test]
    fn media_context_must_match_forum_tenant() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let context = PortContext::new(
            other.to_string(),
            PortActor::service("forum-reconciliation"),
            "en",
            "corr-1",
        );
        assert!(validate_media_context_tenant(&context, tenant_id).is_err());
    }

    #[test]
    fn media_reference_boundary_rejects_foreign_tenant_and_owner() {
        let tenant_id = Uuid::new_v4();
        let foreign = rustok_media::MediaAssetReference {
            media_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            owner_module: "forum".to_string(),
            reference_id: Uuid::new_v4(),
        };
        assert!(validate_media_reference(&foreign, tenant_id).is_err());

        let wrong_owner = rustok_media::MediaAssetReference {
            media_id: Uuid::new_v4(),
            tenant_id,
            owner_module: "blog".to_string(),
            reference_id: Uuid::new_v4(),
        };
        assert!(validate_media_reference(&wrong_owner, tenant_id).is_err());
    }

    #[test]
    fn drift_kind_wire_values_are_stable() {
        assert_eq!(
            ForumAttachmentHoldDriftKind::OrphanMediaHold.as_str(),
            "orphan_media_hold"
        );
        assert_eq!(
            ForumAttachmentHoldDriftKind::MediaReferenceMismatch.as_str(),
            "media_reference_mismatch"
        );
    }
}
