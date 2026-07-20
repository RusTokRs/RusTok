use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect, TransactionTrait, sea_query::Expr,
};

use crate::error::{Error, Result};
use crate::models::{
    _entities::{roles, user_roles},
    users,
};

use super::rbac_cache_invalidation::publish_user_rbac_invalidation;
use super::rbac_invalidation_generation::reserve_rbac_invalidation_generation;
use super::rbac_persistence::replace_user_role_via_store;
use super::rbac_service::RbacService;

impl RbacService {
    /// Replace a role inside a transaction owned by the caller.
    ///
    /// This operation neither commits nor invalidates process-local
    /// authorization caches. The transaction owner must invalidate the user's
    /// RBAC caches only after a successful commit.
    pub(crate) async fn replace_user_role_in_transaction<C>(
        db: &C,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: rustok_core::UserRole,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        replace_user_role_via_store(db, user_id, tenant_id, role).await
    }

    /// Replace a role outside an enclosing transaction and invalidate the
    /// authorization snapshot only after commit, locally and across replicas.
    ///
    /// The target user and the tenant's built-in super-admin role row are
    /// locked before continuity is checked. Concurrent role changes for one
    /// user therefore serialize, and an exact single-role no-op does not
    /// advance the global invalidation generation.
    pub async fn replace_user_role_committed(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: rustok_core::UserRole,
    ) -> Result<()> {
        Self::record_committed_mutation_entrypoint();
        let tx = db.begin().await?;
        let target = lock_target_user_for_role_mutation(&tx, user_id, tenant_id).await?;
        if has_exact_tenant_role_assignment(&tx, user_id, tenant_id, &role).await? {
            tx.rollback().await?;
            return Ok(());
        }
        ensure_active_super_admin_continuity(&tx, &target, tenant_id, &role).await?;
        Self::replace_user_role_in_transaction(&tx, user_id, tenant_id, role).await?;
        let durable_generation = reserve_rbac_invalidation_generation(&tx).await?;
        tx.commit().await?;
        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;
        if let Err(error) =
            publish_user_rbac_invalidation(tenant_id, user_id, durable_generation).await
        {
            tracing::warn!(
                %error,
                durable_generation,
                %tenant_id,
                %user_id,
                "RBAC fast invalidation fan-out failed after committed role replacement; durable generation reconciliation will recover"
            );
            rustok_telemetry::metrics::record_event_error(
                "rbac.permissions.durable_generation.v1",
                "post_commit_fanout",
            );
        }
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

async fn lock_target_user_for_role_mutation<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
) -> Result<users::Model>
where
    C: ConnectionTrait,
{
    let query =
        || users::Entity::find_by_id(*user_id).filter(users::Column::TenantId.eq(*tenant_id));
    let target = match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(db).await?,
        DbBackend::Sqlite => {
            let target = query().one(db).await?;
            if target.is_some() {
                users::Entity::update_many()
                    .col_expr(
                        users::Column::UpdatedAt,
                        Expr::col(users::Column::UpdatedAt).into(),
                    )
                    .filter(users::Column::Id.eq(*user_id))
                    .filter(users::Column::TenantId.eq(*tenant_id))
                    .exec(db)
                    .await?;
            }
            target
        }
    };
    target.ok_or(Error::NotFound)
}

async fn has_exact_tenant_role_assignment<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    resulting_role: &rustok_core::UserRole,
) -> Result<bool>
where
    C: ConnectionTrait,
{
    let tenant_roles = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(*tenant_id))
        .all(db)
        .await?;
    let target_slug = resulting_role.to_string();
    let target_role_id = tenant_roles
        .iter()
        .find(|role| role.slug == target_slug && role.is_system)
        .map(|role| role.id);
    let tenant_role_ids = tenant_roles
        .into_iter()
        .map(|role| role.id)
        .collect::<Vec<_>>();
    if target_role_id.is_none() || tenant_role_ids.is_empty() {
        return Ok(false);
    }

    let assignments = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(*user_id))
        .filter(user_roles::Column::RoleId.is_in(tenant_role_ids))
        .all(db)
        .await?;
    Ok(assignments.len() == 1 && assignments[0].role_id == target_role_id.unwrap())
}

async fn ensure_active_super_admin_continuity<C>(
    db: &C,
    target: &users::Model,
    tenant_id: &uuid::Uuid,
    resulting_role: &rustok_core::UserRole,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if resulting_role == &rustok_core::UserRole::SuperAdmin
        || target.status != rustok_core::UserStatus::Active
    {
        return Ok(());
    }

    let Some(super_admin_role) = find_super_admin_role_for_update(db, tenant_id).await? else {
        return Ok(());
    };

    let target_is_super_admin = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(target.id))
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
        .filter(users::Column::Id.ne(target.id))
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

#[cfg(test)]
mod tests {
    use super::RbacService;
    use crate::error::Error;
    use crate::models::{tenants, users};
    use crate::services::rbac_invalidation_generation::read_rbac_invalidation_generation;
    use chrono::Utc;
    use rustok_api::Permission;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ConnectionTrait, EntityTrait, Set};

    async fn insert_tenant_and_user(
        db: &impl ConnectionTrait,
        tenant_slug: &str,
        email: &str,
    ) -> (uuid::Uuid, uuid::Uuid) {
        let tenant_id = rustok_core::generate_id();
        let user_id = rustok_core::generate_id();

        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Test tenant".to_string()),
            slug: Set(tenant_slug.to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(db)
        .await
        .expect("failed to insert tenant");

        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_id),
            email: Set(email.to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            status: Set(UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(db)
        .await
        .expect("failed to insert user");

        (tenant_id, user_id)
    }

    #[tokio::test]
    async fn committed_role_replacement_invalidates_primed_permission_cache() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "committed-role-cache-invalidation",
            "committed-role-cache@example.com",
        )
        .await;

        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Admin)
            .await
            .expect("admin role assignment should succeed");

        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE,)
                .await
                .expect("admin permission lookup should succeed")
        );
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .expect("committed demotion should succeed");

        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 1);
        assert!(
            !RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE,)
                .await
                .expect("post-demotion permission lookup should succeed")
        );
        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::PRODUCTS_READ,)
                .await
                .expect("customer permission lookup should succeed")
        );
    }

    #[tokio::test]
    async fn exact_single_role_replacement_is_a_generation_noop() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "committed-role-exact-noop",
            "committed-role-noop@example.com",
        )
        .await;
        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .unwrap();

        RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .unwrap();

        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn matching_role_among_multiple_assignments_is_not_treated_as_noop() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "committed-role-multiple",
            "committed-role-multiple@example.com",
        )
        .await;
        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .unwrap();
        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .unwrap();

        RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .unwrap();

        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 1);
        assert!(
            !RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::PRODUCTS_CREATE,)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn rejected_last_super_admin_demotion_preserves_authority() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "last-super-admin-rollback",
            "last-super-admin@example.com",
        )
        .await;

        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::SuperAdmin)
            .await
            .expect("super-admin role assignment should succeed");
        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE,)
                .await
                .expect("super-admin permission lookup should succeed")
        );
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let error =
            RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer)
                .await
                .expect_err("last active super-admin demotion must be rejected");
        assert!(matches!(error, Error::BadRequest(_)));
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let authoritative =
            RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id)
                .await
                .expect("authoritative permissions should remain readable");
        assert!(authoritative.contains(&Permission::SETTINGS_MANAGE));
        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE,)
                .await
                .expect("cached permission lookup should remain valid")
        );
    }
}
