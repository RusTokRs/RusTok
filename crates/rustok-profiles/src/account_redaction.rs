use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{ProfileResult, ProfileStatus, entities};

/// Hides a tenant-scoped profile inside the caller-owned account-deactivation transaction.
///
/// The caller must persist its durable account-deletion invalidation event before committing the
/// same transaction. A missing profile is a valid terminal state and still requires that event so
/// downstream projections can remove any stale embedded author presentation.
pub async fn redact_profile_for_account_deactivation_in_tx(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ProfileResult<bool> {
    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await?;
    let Some(profile) = profile else {
        return Ok(false);
    };

    let mut active: entities::profile::ActiveModel = profile.into();
    active.status = Set(ProfileStatus::Hidden);
    active.updated_at = Set(Utc::now().into());
    active.update(transaction).await?;
    Ok(true)
}
