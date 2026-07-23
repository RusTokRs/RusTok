use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Statement,
};
use uuid::Uuid;

use crate::dto::GroupMembershipEffectiveState;
use crate::entities::group;
use crate::error::GroupsResult;
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::membership_enforcement_entities::{membership_enforcement, membership_state};

/// Resolve effective membership under the Groups owner write-lock protocol.
///
/// The lock order is always `Group -> GroupMembership -> GroupMembershipEnforcement`.
/// PostgreSQL/MySQL use row locks. SQLite acquires the database writer reservation through a
/// no-op group update before reading membership/enforcement, preventing another owner transaction
/// from committing enforcement or membership changes between authorization and mutation.
pub(crate) async fn resolve_group_membership_enforcement_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    evaluated_at: DateTime<Utc>,
) -> GroupsResult<GroupMembershipEffectiveState> {
    match transaction.get_database_backend() {
        DbBackend::Sqlite => {
            // The command has already resolved the group row before entering the effective guard.
            // Do not use rows_affected as an existence test: SQLite may report zero for a no-op
            // assignment depending on connection settings even though the writer lock was acquired.
            transaction
                .execute(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "UPDATE groups SET version = version WHERE tenant_id = ? AND id = ?",
                    [tenant_id.into(), group_id.into()],
                ))
                .await?;
        }
        DbBackend::Postgres | DbBackend::MySql => {
            group::Entity::find()
                .filter(group::Column::TenantId.eq(tenant_id))
                .filter(group::Column::Id.eq(group_id))
                .lock_exclusive()
                .one(transaction)
                .await?;
        }
    }

    let membership = match transaction.get_database_backend() {
        DbBackend::Sqlite => membership_state::Entity::find()
            .filter(membership_state::Column::TenantId.eq(tenant_id))
            .filter(membership_state::Column::GroupId.eq(group_id))
            .filter(membership_state::Column::UserId.eq(user_id))
            .one(transaction)
            .await?,
        DbBackend::Postgres | DbBackend::MySql => membership_state::Entity::find()
            .filter(membership_state::Column::TenantId.eq(tenant_id))
            .filter(membership_state::Column::GroupId.eq(group_id))
            .filter(membership_state::Column::UserId.eq(user_id))
            .lock_exclusive()
            .one(transaction)
            .await?,
    };

    if let Some(membership) = membership {
        match transaction.get_database_backend() {
            DbBackend::Sqlite => {
                membership_enforcement::Entity::find_by_id(membership.id)
                    .filter(membership_enforcement::Column::TenantId.eq(tenant_id))
                    .one(transaction)
                    .await?;
            }
            DbBackend::Postgres | DbBackend::MySql => {
                membership_enforcement::Entity::find_by_id(membership.id)
                    .filter(membership_enforcement::Column::TenantId.eq(tenant_id))
                    .lock_exclusive()
                    .one(transaction)
                    .await?;
            }
        }
    }

    resolve_group_membership_enforcement(
        transaction,
        tenant_id,
        group_id,
        user_id,
        evaluated_at,
    )
    .await
}

pub(crate) async fn resolve_group_membership_enforcement_now_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<GroupMembershipEffectiveState> {
    resolve_group_membership_enforcement_for_update(
        transaction,
        tenant_id,
        group_id,
        user_id,
        Utc::now(),
    )
    .await
}
