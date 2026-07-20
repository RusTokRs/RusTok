use crate::error::Error;
use crate::error::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use rustok_core::UserRole;
use rustok_telemetry::metrics;

use crate::models::_entities::{roles, user_roles};

pub(crate) async fn assign_role_permissions_via_store<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()>
where
    C: ConnectionTrait,
{
    record_authz_entrypoint_call("assign_role_permissions_via_store", "core_runtime");
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(db, *tenant_id, *user_id, role)
        .await
        .map_err(|error| Error::Message(error.to_string()))
}

pub(crate) async fn replace_user_role_via_store<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()>
where
    C: ConnectionTrait,
{
    record_authz_entrypoint_call("replace_user_role_via_store", "core_runtime");
    remove_tenant_role_assignments_via_store(db, user_id, tenant_id).await?;
    assign_role_permissions_via_store(db, user_id, tenant_id, role).await
}

pub(crate) async fn remove_tenant_role_assignments_via_store<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
) -> Result<()>
where
    C: ConnectionTrait,
{
    record_authz_entrypoint_call("remove_tenant_role_assignments_via_store", "core_runtime");
    let tenant_role_models = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(*tenant_id))
        .all(db)
        .await?;

    let tenant_role_ids: Vec<uuid::Uuid> = tenant_role_models
        .into_iter()
        .map(|tenant_role| tenant_role.id)
        .collect();

    if !tenant_role_ids.is_empty() {
        user_roles::Entity::delete_many()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .filter(user_roles::Column::RoleId.is_in(tenant_role_ids))
            .exec(db)
            .await?;
    }

    Ok(())
}

pub(crate) async fn remove_user_role_assignment_via_store<C>(
    db: &C,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let role_slug = role.to_string();
    let tenant_role = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(*tenant_id))
        .filter(roles::Column::Slug.eq(role_slug))
        .one(db)
        .await?;

    if let Some(tenant_role) = tenant_role {
        user_roles::Entity::delete_many()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .filter(user_roles::Column::RoleId.eq(tenant_role.id))
            .exec(db)
            .await?;
    }

    Ok(())
}

fn record_authz_entrypoint_call(entry_point: &str, path: &str) {
    metrics::record_module_entrypoint_call("rbac", entry_point, path);
}

#[cfg(test)]
mod tests {
    use super::{
        assign_role_permissions_via_store, remove_tenant_role_assignments_via_store,
        replace_user_role_via_store,
    };
    use crate::models::_entities::{permissions, role_permissions, roles, user_roles};
    use crate::models::{tenants, users};
    use chrono::Utc;
    use rustok_api::Permission;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};

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
    async fn assign_role_permissions_creates_user_roles_link() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) =
            insert_tenant_and_user(&db, "test-tenant-assign-role", "assign-role@example.com").await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("assign role permissions should succeed");

        let tenant_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to load tenant role")
            .expect("tenant role should exist");

        let relation_exists = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .filter(user_roles::Column::RoleId.eq(tenant_role.id))
            .one(&db)
            .await
            .expect("failed to query user_roles")
            .is_some();

        assert!(relation_exists);
    }

    #[tokio::test]
    async fn cross_tenant_role_assignment_is_rejected_without_side_effects() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (user_tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-user-tenant-role-boundary",
            "role-boundary-user@example.com",
        )
        .await;
        let (foreign_tenant_id, _) = insert_tenant_and_user(
            &db,
            "test-foreign-tenant-role-boundary",
            "role-boundary-foreign@example.com",
        )
        .await;

        let error =
            assign_role_permissions_via_store(&db, &user_id, &foreign_tenant_id, UserRole::Manager)
                .await
                .expect_err("cross-tenant role assignment must fail");
        let message = error.to_string();
        assert!(message.contains(&user_id.to_string()));
        assert!(message.contains(&user_tenant_id.to_string()));
        assert!(message.contains(&foreign_tenant_id.to_string()));

        let foreign_manager_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(foreign_tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to query foreign tenant role");
        assert!(foreign_manager_role.is_none());

        let assignments = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&db)
            .await
            .expect("failed to query rejected user role assignments");
        assert!(assignments.is_empty());
    }

    #[tokio::test]
    async fn explicit_reconciliation_links_expected_permission_and_removes_stale_link() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-tenant-role-permission-sync",
            "role-permission-sync@example.com",
        )
        .await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("manager role assignment should succeed");

        let manager_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to load manager role")
            .expect("manager role should exist");
        let expected = Permission::PRODUCTS_CREATE;
        let expected_permission = permissions::Entity::find()
            .filter(permissions::Column::TenantId.eq(tenant_id))
            .filter(permissions::Column::Resource.eq(expected.resource.to_string()))
            .filter(permissions::Column::Action.eq(expected.action.to_string()))
            .one(&db)
            .await
            .expect("failed to load expected permission")
            .expect("expected permission should exist");

        let expected_link_exists = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.eq(manager_role.id))
            .filter(role_permissions::Column::PermissionId.eq(expected_permission.id))
            .one(&db)
            .await
            .expect("failed to query expected role permission")
            .is_some();
        assert!(expected_link_exists);

        let stale = Permission::SETTINGS_MANAGE;
        let stale_permission_id = rustok_core::generate_id();
        permissions::Entity::insert(permissions::ActiveModel {
            id: Set(stale_permission_id),
            tenant_id: Set(tenant_id),
            resource: Set(stale.resource.to_string()),
            action: Set(stale.action.to_string()),
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

        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_permissions_on(
            &db,
            tenant_id,
            user_id,
            UserRole::Manager,
        )
        .await
        .expect("explicit manager role reconciliation should succeed");

        let stale_link_exists = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.eq(manager_role.id))
            .filter(role_permissions::Column::PermissionId.eq(stale_permission_id))
            .one(&db)
            .await
            .expect("failed to query stale role permission")
            .is_some();
        assert!(!stale_link_exists);
    }

    #[tokio::test]
    async fn routine_assignment_preserves_existing_role_definition() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-tenant-runtime-role-definition",
            "runtime-role-definition@example.com",
        )
        .await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("manager role assignment should succeed");
        let manager_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to load manager role")
            .expect("manager role should exist");
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
        .expect("insert noncanonical permission");
        role_permissions::Entity::insert(role_permissions::ActiveModel {
            id: Set(rustok_core::generate_id()),
            role_id: Set(manager_role.id),
            permission_id: Set(stale_permission_id),
        })
        .exec(&db)
        .await
        .expect("insert noncanonical role permission");

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("routine assignment should succeed");

        assert!(
            role_permissions::Entity::find()
                .filter(role_permissions::Column::RoleId.eq(manager_role.id))
                .filter(role_permissions::Column::PermissionId.eq(stale_permission_id))
                .one(&db)
                .await
                .expect("query role definition")
                .is_some()
        );
    }

    #[tokio::test]
    async fn replace_user_role_replaces_tenant_role_assignment() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) =
            insert_tenant_and_user(&db, "test-tenant-replace-role", "replace-role@example.com")
                .await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .expect("initial role assignment should succeed");

        replace_user_role_via_store(&db, &user_id, &tenant_id, UserRole::Admin)
            .await
            .expect("role replacement should succeed");

        let admin_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Admin.to_string()))
            .one(&db)
            .await
            .expect("failed to load admin role")
            .expect("admin role should exist");

        let customer_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Customer.to_string()))
            .one(&db)
            .await
            .expect("failed to load customer role")
            .expect("customer role should exist");

        let has_admin = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .filter(user_roles::Column::RoleId.eq(admin_role.id))
            .one(&db)
            .await
            .expect("failed to query admin assignment")
            .is_some();

        let has_customer = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .filter(user_roles::Column::RoleId.eq(customer_role.id))
            .one(&db)
            .await
            .expect("failed to query customer assignment")
            .is_some();

        assert!(has_admin);
        assert!(!has_customer);
    }

    #[tokio::test]
    async fn assign_role_permissions_is_idempotent_for_user_role_link() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-tenant-idempotent-role",
            "idempotent-role@example.com",
        )
        .await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("first role assignment should succeed");
        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("second role assignment should succeed");

        let manager_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
            .one(&db)
            .await
            .expect("failed to load manager role")
            .expect("manager role should exist");

        let assignment_count = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .filter(user_roles::Column::RoleId.eq(manager_role.id))
            .count(&db)
            .await
            .expect("failed to count user_roles links");

        assert_eq!(assignment_count, 1);
    }

    #[tokio::test]
    async fn remove_tenant_role_assignments_clears_user_links_for_tenant_roles() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-tenant-remove-all-roles",
            "remove-all-roles@example.com",
        )
        .await;

        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .expect("customer role assignment should succeed");
        assign_role_permissions_via_store(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("manager role assignment should succeed");

        remove_tenant_role_assignments_via_store(&db, &user_id, &tenant_id)
            .await
            .expect("remove tenant role assignments should succeed");

        let remaining_links = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&db)
            .await
            .expect("failed to query remaining user_roles links");

        assert!(remaining_links.is_empty());
    }
}
