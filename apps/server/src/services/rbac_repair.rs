use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};

use crate::error::{Error, Result};
use crate::models::users;

use super::rbac_cache_invalidation::publish_all_rbac_invalidation;
use super::rbac_invalidation_generation::reserve_rbac_invalidation_generation;
use super::rbac_service::RbacService;

impl RbacService {
    /// Build a read-only repair plan for canonical built-in role definitions.
    pub async fn plan_system_role_repair(
        db: &DatabaseConnection,
        tenant_id: Option<uuid::Uuid>,
    ) -> Result<rustok_rbac::RbacSystemRoleRepairReport> {
        Self::record_system_role_repair_entrypoint("plan_system_role_repair");
        rustok_rbac::plan_system_role_repair(db, tenant_id)
            .await
            .map_err(|error| Error::Message(error.to_string()))
    }

    /// Repair canonical built-in role definitions and invalidate every affected
    /// permission snapshot after commit, locally and across replicas.
    pub async fn repair_system_roles_committed(
        db: &DatabaseConnection,
        tenant_id: Option<uuid::Uuid>,
    ) -> Result<rustok_rbac::RbacSystemRoleRepairReport> {
        Self::record_system_role_repair_entrypoint("repair_system_roles_committed");
        let tx = db.begin().await?;
        let mut report = rustok_rbac::apply_system_role_repair_in_transaction(&tx, tenant_id)
            .await
            .map_err(|error| Error::Message(error.to_string()))?;

        let mut effective_affected_users = Vec::with_capacity(report.affected_users.len());
        for affected in report.affected_users.drain(..) {
            let belongs_to_role_tenant = users::Entity::find_by_id(affected.user_id)
                .filter(users::Column::TenantId.eq(affected.tenant_id))
                .one(&tx)
                .await?
                .is_some();
            if belongs_to_role_tenant {
                effective_affected_users.push(affected);
            }
        }

        let durable_generation = if effective_affected_users.is_empty() {
            None
        } else {
            Some(reserve_rbac_invalidation_generation(&tx).await?)
        };
        tx.commit().await?;
        report.applied = true;

        for affected in &effective_affected_users {
            Self::invalidate_user_rbac_caches(&affected.tenant_id, &affected.user_id).await;
        }
        if let Some(durable_generation) = durable_generation {
            if let Err(error) = publish_all_rbac_invalidation(durable_generation).await {
                tracing::warn!(
                    %error,
                    durable_generation,
                    "System-role repair fast RBAC invalidation fan-out failed; durable generation reconciliation will recover"
                );
                rustok_telemetry::metrics::record_event_error(
                    "rbac.permissions.durable_generation.v1",
                    "post_commit_fanout",
                );
            }
        }
        report.affected_users = effective_affected_users;
        report.runtime_restart_required = false;
        Ok(report)
    }

    fn record_system_role_repair_entrypoint(entry_point: &str) {
        rustok_telemetry::metrics::record_module_entrypoint_call("rbac", entry_point, "library");
    }
}

#[cfg(test)]
mod tests {
    use super::RbacService;
    use crate::models::{
        _entities::{permissions, role_permissions, roles, user_roles},
        tenants, users,
    };
    use crate::services::rbac_invalidation_generation::read_rbac_invalidation_generation;
    use chrono::Utc;
    use rustok_api::Permission;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

    async fn insert_tenant(db: &impl ConnectionTrait, tenant_slug: &str) -> uuid::Uuid {
        let tenant_id = rustok_core::generate_id();
        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Repair test tenant".to_string()),
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
        tenant_id
    }

    async fn insert_user(
        db: &impl ConnectionTrait,
        tenant_id: uuid::Uuid,
        email: &str,
    ) -> uuid::Uuid {
        let user_id = rustok_core::generate_id();
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
        user_id
    }

    #[tokio::test]
    async fn committed_repair_invalidates_all_valid_role_holders() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = insert_tenant(&db, "system-role-repair-cache").await;
        let other_tenant_id = insert_tenant(&db, "system-role-repair-cache-other").await;
        let first_user = insert_user(&db, tenant_id, "repair-first@example.com").await;
        let second_user = insert_user(&db, tenant_id, "repair-second@example.com").await;
        let cross_tenant_user =
            insert_user(&db, other_tenant_id, "repair-cross-tenant@example.com").await;

        for user_id in [first_user, second_user] {
            RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
                .await
                .expect("manager assignment should succeed");
        }

        let manager_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to load manager role")
            .expect("manager role should exist");
        assert!(
            user_roles::Entity::insert(user_roles::ActiveModel {
                id: Set(rustok_core::generate_id()),
                user_id: Set(cross_tenant_user),
                role_id: Set(manager_role.id),
            })
            .exec(&db)
            .await
            .is_err()
        );

        let stale_permission_id = rustok_core::generate_id();
        permissions::Entity::insert(permissions::ActiveModel {
            id: Set(stale_permission_id),
            tenant_id: Set(tenant_id),
            resource: Set(Permission::SETTINGS_MANAGE.resource.to_string()),
            action: Set(Permission::SETTINGS_MANAGE.action.to_string()),
            description: Set(None),
            created_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert stale permission");
        role_permissions::Entity::insert(role_permissions::ActiveModel {
            id: Set(rustok_core::generate_id()),
            role_id: Set(manager_role.id),
            permission_id: Set(stale_permission_id),
        })
        .exec(&db)
        .await
        .expect("failed to insert stale role permission");

        for user_id in [first_user, second_user] {
            RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
            assert!(
                RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE,
                )
                .await
                .expect("primed permission lookup should succeed")
            );
        }
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let report = RbacService::repair_system_roles_committed(&db, Some(tenant_id))
            .await
            .expect("committed repair should succeed");

        assert!(report.applied);
        assert!(report.role_permission_links_removed >= 1);
        assert!(!report.runtime_restart_required);
        assert_eq!(report.affected_users.len(), 2);
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 1);
        assert!(
            report
                .affected_users
                .iter()
                .all(|affected| affected.tenant_id == tenant_id)
        );
        assert!(
            !report
                .affected_users
                .iter()
                .any(|affected| affected.user_id == cross_tenant_user)
        );

        for user_id in [first_user, second_user] {
            assert!(
                !RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE,
                )
                .await
                .expect("post-repair permission lookup should succeed")
            );
        }
    }
}
