use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AssetState, BlobState, MediaError, MediaService,
    entities::{
        asset::{Column as AssetCol, Entity as AssetEntity, Model as AssetModel},
        blob::{Entity as BlobEntity, Model as BlobModel},
    },
};

pub const MAX_MEDIA_REFERENCE_ADMISSION_IDS: usize = 100;

/// Media-owned answer to one cross-module reference-admission question.
///
/// The contract intentionally exposes only the durable decision another owner
/// needs before persisting a typed Media reference. Media lifecycle strings,
/// storage state, quarantine implementation details and deletion timestamps stay
/// private to Media. Any state that is not explicitly known-safe maps to
/// `referenceable = false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaReferenceAdmission {
    pub media_id: Uuid,
    pub tenant_id: Uuid,
    pub referenceable: bool,
}

/// Owner port for bounded cross-module reference admission.
///
/// Consumers must call this capability instead of inferring persistence safety
/// from `MediaAssetReadPort::get_asset` or from copied lifecycle fields. Missing
/// assets are represented as non-referenceable results so a bounded batch has a
/// deterministic one-result-per-request contract without leaking raw owner state.
#[async_trait]
pub trait MediaReferenceAdmissionPort: Send + Sync {
    async fn admit_references(
        &self,
        context: PortContext,
        media_ids: Vec<Uuid>,
    ) -> Result<Vec<MediaReferenceAdmission>, PortError>;
}

#[async_trait]
impl MediaReferenceAdmissionPort for MediaService {
    async fn admit_references(
        &self,
        context: PortContext,
        media_ids: Vec<Uuid>,
    ) -> Result<Vec<MediaReferenceAdmission>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "media.invalid_tenant_id",
                "media reference-admission context must carry a UUID tenant_id",
            )
        })?;
        let requested = normalize_reference_ids(media_ids)?;
        if requested.is_empty() {
            return Ok(Vec::new());
        }

        let rows = AssetEntity::find()
            .filter(AssetCol::TenantId.eq(tenant_id))
            .filter(AssetCol::Id.is_in(requested.clone()))
            .find_also_related(BlobEntity)
            .all(self.database())
            .await
            .map_err(|error| crate::ports::media_error_to_port_error(MediaError::Db(error)))?;
        let mut rows_by_id = rows
            .into_iter()
            .map(|(asset, blob)| (asset.id, (asset, blob)))
            .collect::<HashMap<_, _>>();

        Ok(requested
            .into_iter()
            .map(|media_id| {
                let referenceable = rows_by_id
                    .remove(&media_id)
                    .is_some_and(|(asset, blob)| {
                        is_referenceable(tenant_id, &asset, blob.as_ref())
                    });
                MediaReferenceAdmission {
                    media_id,
                    tenant_id,
                    referenceable,
                }
            })
            .collect())
    }
}

fn normalize_reference_ids(media_ids: Vec<Uuid>) -> Result<Vec<Uuid>, PortError> {
    if media_ids.len() > MAX_MEDIA_REFERENCE_ADMISSION_IDS {
        return Err(PortError::validation(
            "media.reference_admission_batch_too_large",
            format!(
                "media reference-admission batch exceeds {MAX_MEDIA_REFERENCE_ADMISSION_IDS} ids"
            ),
        ));
    }

    let mut seen = HashSet::with_capacity(media_ids.len());
    let mut normalized = Vec::with_capacity(media_ids.len());
    for media_id in media_ids {
        if media_id.is_nil() {
            return Err(PortError::validation(
                "media.reference_admission_nil_id",
                "media reference-admission ids must not be nil",
            ));
        }
        if seen.insert(media_id) {
            normalized.push(media_id);
        }
    }
    Ok(normalized)
}

fn is_referenceable(
    tenant_id: Uuid,
    asset: &AssetModel,
    active_blob: Option<&BlobModel>,
) -> bool {
    if asset.tenant_id != tenant_id
        || asset.lifecycle_state != AssetState::Active.as_str()
        || asset.delete_requested_at.is_some()
        || asset.deleted_at.is_some()
    {
        return false;
    }

    let Some(active_blob_id) = asset.active_blob_id else {
        return false;
    };
    let Some(blob) = active_blob else {
        return false;
    };

    blob.id == active_blob_id
        && blob.tenant_id == tenant_id
        && blob.asset_id == asset.id
        && blob.state == BlobState::Ready.as_str()
        && blob.ready_at.is_some()
        && blob.delete_requested_at.is_none()
        && blob.deleted_at.is_none()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn fixture(state: &str, blob_state: &str) -> (Uuid, AssetModel, BlobModel) {
        let tenant_id = Uuid::new_v4();
        let asset_id = Uuid::new_v4();
        let blob_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        let asset = AssetModel {
            id: asset_id,
            tenant_id,
            uploaded_by: None,
            upload_session_id: None,
            active_blob_id: Some(blob_id),
            original_name: "attachment.png".to_string(),
            lifecycle_state: state.to_string(),
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            delete_requested_at: None,
            deleted_at: None,
        };
        let blob = BlobModel {
            id: blob_id,
            tenant_id,
            asset_id,
            object_key: "media/attachment.png".to_string(),
            mime_type: "image/png".to_string(),
            size: 1,
            checksum_sha256: "00".repeat(32),
            width: Some(1),
            height: Some(1),
            state: blob_state.to_string(),
            created_at: now,
            ready_at: Some(now),
            delete_requested_at: None,
            deleted_at: None,
            reconcile_attempts: 0,
            last_reconciled_at: now,
            last_error: None,
        };
        (tenant_id, asset, blob)
    }

    #[test]
    fn only_active_asset_with_ready_active_blob_is_referenceable() {
        let (tenant_id, asset, blob) = fixture(AssetState::Active.as_str(), BlobState::Ready.as_str());
        assert!(is_referenceable(tenant_id, &asset, Some(&blob)));
    }

    #[test]
    fn lifecycle_and_unknown_states_fail_closed() {
        for state in [
            AssetState::DeletePending.as_str(),
            AssetState::Deleted.as_str(),
            AssetState::Failed.as_str(),
            "quarantined",
            "future-owner-state",
        ] {
            let (tenant_id, asset, blob) = fixture(state, BlobState::Ready.as_str());
            assert!(!is_referenceable(tenant_id, &asset, Some(&blob)), "accepted {state}");
        }
    }

    #[test]
    fn non_ready_missing_or_deleting_blob_fails_closed() {
        let (tenant_id, asset, mut blob) =
            fixture(AssetState::Active.as_str(), BlobState::Pending.as_str());
        assert!(!is_referenceable(tenant_id, &asset, Some(&blob)));
        assert!(!is_referenceable(tenant_id, &asset, None));

        blob.state = BlobState::Ready.as_str().to_string();
        blob.delete_requested_at = Some(Utc::now().fixed_offset());
        assert!(!is_referenceable(tenant_id, &asset, Some(&blob)));
    }

    #[test]
    fn request_normalization_is_bounded_deduplicated_and_rejects_nil_ids() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert_eq!(
            normalize_reference_ids(vec![first, second, first]).unwrap(),
            vec![first, second]
        );
        assert!(normalize_reference_ids(vec![Uuid::nil()]).is_err());
        assert!(
            normalize_reference_ids(
                (0..=MAX_MEDIA_REFERENCE_ADMISSION_IDS)
                    .map(|_| Uuid::new_v4())
                    .collect(),
            )
            .is_err()
        );
    }
}
