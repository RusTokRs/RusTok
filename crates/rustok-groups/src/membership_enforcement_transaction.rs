use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Statement,
};
use uuid::Uuid;

use crate::dto::GroupMembershipEffectiveState;
use crate::entities::group;
use crate::error::{GroupsError, GroupsResult};
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::membership_enforcement_entities::{membership_enforcement, membership_state};

/// One membership subject locked under the canonical Groups owner aggregate order.
///
/// The initial unlocked membership lookup used by `lock_membership_enforcement_target_by_id_for_update`
/// is only an aggregate locator. Callers must make every authorization, revision, provenance, and
/// mutation decision from these locked rows.
pub(crate) struct LockedMembershipEnforcementTarget {
    pub(crate) group: group::Model,
    pub(crate) membership: membership_state::Model,
    pub(crate) enforcement: Option<membership_enforcement::Model>,
}

impl LockedMembershipEnforcementTarget {
    pub(crate) fn group(&self) -> &group::Model {
        &self.group
    }

    pub(crate) fn membership(&self) -> &membership_state::Model {
        &self.membership
    }

    pub(crate) fn enforcement(&self) -> Option<&membership_enforcement::Model> {
        self.enforcement.as_ref()
    }
}

/// Acquire the Groups owner aggregate writer reservation before reading mutable group state.
///
/// PostgreSQL/MySQL retain an exclusive row lock. SQLite obtains the writer reservation through a
/// no-op update so subsequent group-version and membership/enforcement reads cannot race another
/// owner writer. Existence remains the caller's domain concern; do not use rows-affected here as an
/// existence signal because SQLite may report zero for a no-op assignment.
pub(crate) async fn reserve_group_write_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<()> {
    match transaction.get_database_backend() {
        DbBackend::Sqlite => {
            transaction
                .execute_raw(Statement::from_sql_and_values(
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
        _ => unreachable!("unsupported SeaORM database backend"),
}
    Ok(())
}

/// Lock one revisioned membership subject addressed by `group_memberships.id`.
///
/// A membership UUID does not itself carry the owning `group_id`, so the function first performs a
/// tenant-scoped locator read. That row is never trusted as mutable state. It then acquires the
/// canonical `Group -> GroupMembership -> GroupMembershipEnforcement` owner locks, re-reads the
/// membership under lock, and verifies that its immutable aggregate identity still matches the
/// locator. This makes the primitive suitable for receipt-first producer adapters that receive a
/// membership subject UUID rather than a `(group_id, user_id)` pair.
///
/// Missing subjects return `Ok(None)`. Corrupt identity movement fails closed instead of silently
/// retargeting the caller to another group or user.
pub(crate) async fn lock_membership_enforcement_target_by_id_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    membership_id: Uuid,
) -> GroupsResult<Option<LockedMembershipEnforcementTarget>> {
    let Some(locator) = membership_state::Entity::find_by_id(membership_id)
        .filter(membership_state::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await?
    else {
        return Ok(None);
    };

    reserve_group_write_for_update(transaction, tenant_id, locator.group_id).await?;

    let group_model = group::Entity::find()
        .filter(group::Column::TenantId.eq(tenant_id))
        .filter(group::Column::Id.eq(locator.group_id))
        .one(transaction)
        .await?
        .ok_or_else(|| {
            GroupsError::Invariant(
                "membership subject points to a missing group after owner reservation".to_string(),
            )
        })?;

    let locked_membership = match transaction.get_database_backend() {
        DbBackend::Sqlite => {
            membership_state::Entity::find_by_id(membership_id)
                .filter(membership_state::Column::TenantId.eq(tenant_id))
                .filter(membership_state::Column::GroupId.eq(locator.group_id))
                .one(transaction)
                .await?
        }
        DbBackend::Postgres | DbBackend::MySql => {
            membership_state::Entity::find_by_id(membership_id)
                .filter(membership_state::Column::TenantId.eq(tenant_id))
                .filter(membership_state::Column::GroupId.eq(locator.group_id))
                .lock_exclusive()
                .one(transaction)
                .await?
        }
        _ => unreachable!("unsupported SeaORM database backend"),
};
    let Some(locked_membership) = locked_membership else {
        return Ok(None);
    };
    if locked_membership.group_id != locator.group_id
        || locked_membership.user_id != locator.user_id
    {
        return Err(GroupsError::Invariant(
            "membership subject aggregate identity changed while owner locks were acquired"
                .to_string(),
        ));
    }

    let enforcement = match transaction.get_database_backend() {
        DbBackend::Sqlite => {
            membership_enforcement::Entity::find_by_id(locked_membership.id)
                .filter(membership_enforcement::Column::TenantId.eq(tenant_id))
                .one(transaction)
                .await?
        }
        DbBackend::Postgres | DbBackend::MySql => {
            membership_enforcement::Entity::find_by_id(locked_membership.id)
                .filter(membership_enforcement::Column::TenantId.eq(tenant_id))
                .lock_exclusive()
                .one(transaction)
                .await?
        }
        _ => unreachable!("unsupported SeaORM database backend"),
};

    Ok(Some(LockedMembershipEnforcementTarget {
        group: group_model,
        membership: locked_membership,
        enforcement,
    }))
}

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
    reserve_group_write_for_update(transaction, tenant_id, group_id).await?;

    let membership = match transaction.get_database_backend() {
        DbBackend::Sqlite => {
            membership_state::Entity::find()
                .filter(membership_state::Column::TenantId.eq(tenant_id))
                .filter(membership_state::Column::GroupId.eq(group_id))
                .filter(membership_state::Column::UserId.eq(user_id))
                .one(transaction)
                .await?
        }
        DbBackend::Postgres | DbBackend::MySql => {
            membership_state::Entity::find()
                .filter(membership_state::Column::TenantId.eq(tenant_id))
                .filter(membership_state::Column::GroupId.eq(group_id))
                .filter(membership_state::Column::UserId.eq(user_id))
                .lock_exclusive()
                .one(transaction)
                .await?
        }
        _ => unreachable!("unsupported SeaORM database backend"),
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
            _ => unreachable!("unsupported SeaORM database backend"),
}
    }

    resolve_group_membership_enforcement(transaction, tenant_id, group_id, user_id, evaluated_at)
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
