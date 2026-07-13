use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect, TransactionTrait,
};

use crate::error::{Error, Result};
use crate::models::{
    _entities::{roles, user_roles},
    users,
};

use super::rbac_persistence::replace_user_role_via_store;
use super::rbac_service::RbacService;

impl RbacService {
    /// Replace a role outside an enclosing transaction and invalidate the
    /// process-local authorization snapshot only after commit.
    ///
    /// The tenant's built-in super-admin role row is locked before continuity
    /// is checked. Concurrent demotions therefore serialize instead of both
    /// observing the same pre-change administrator set.
    pub async fn replace_user_role_committed(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: rustok_core::UserRole,
    ) -> Result<()> {
        Self::record_committed_mutation_entrypoint();
        let tx = db.begin().await?;
        ensure_active_super_admin_continuity(&tx, user_id, tenant_id, &role).await?;
        replace_user_role_via_store(&tx, user_id, tenant_id, role).await?;
        tx.commit().await?;
        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;
        Ok(())
    }

    fn record_committed_mutation_entrypoint() {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "rbac",
            "replace_user_role_committed",
            "library",
        );
    }
}

async fn ensure_active_super_admin_continuity<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    resulting_role: &rustok_core::UserRole,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if resulting_role == &rustok_core::UserRole::SuperAdmin {
        return Ok(());
    }

    let target = users::Entity::find_by_id(*user_id)
        .filter(users::Column::TenantId.eq(*tenant_id))
        .one(db)
        .await?
        .ok_or(Error::NotFound)?;
    if target.status != rustok_core::UserStatus::Active {
        return Ok(());
    }

    let Some(super_admin_role) = find_super_admin_role_for_update(db, tenant_id).await? else {
        return Ok(());
    };

    let target_is_super_admin = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(*user_id))
        .filter(user_roles::Column::RoleId.eq(super_admin_role.id))
        .count(db)
        .await?
        > 0;
    if !target_is_super_admin {
        return Ok(());
    }

    let super_admin_user_ids = user_roles::Entity::find()
        .select_only()
        .column(user_roles::Column::UserId)
        .filter(user_roles::Column::RoleId.eq(super_admin_role.id))
        .into_tuple::<uuid::Uuid>()
        .all(db)
        .await?;
    let remaining_active = users::Entity::find()
        .filter(users::Column::TenantId.eq(*tenant_id))
        .filter(users::Column::Id.is_in(super_admin_user_ids))
        .filter(users::Column::Id.ne(*user_id))
        .filter(users::Column::Status.eq(rustok_core::UserStatus::Active))
        .count(db)
        .await?;

    if remaining_active == 0 {
        return Err(Error::BadRequest(
            "cannot demote the last active super administrator".to_string(),
        ));
    }

    Ok(())
}

async fn find_super_admin_role_for_update<C>(
    db: &C,
    tenant_id: &uuid::Uuid,
) -> Result<Option<roles::Model>>
where
    C: ConnectionTrait,
{
    let query = || {
        roles::Entity::find()
            .filter(roles::Column::TenantId.eq(*tenant_id))
            .filter(roles::Column::Slug.eq(rustok_core::UserRole::SuperAdmin.to_string()))
    };

    match db.get_database_backend() {
        DbBackend::Sqlite => query().one(db).await.map_err(Into::into),
        DbBackend::Postgres | DbBackend::MySql => {
            query().lock_exclusive().one(db).await.map_err(Into::into)
        }
    }
}
