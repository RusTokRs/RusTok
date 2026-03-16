use loco_rs::Error;
use loco_rs::Result;
use sea_orm::{
    sea_query::OnConflict, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};

use rustok_core::{Permission, Rbac, UserRole};
use rustok_telemetry::metrics;

use crate::models::_entities::{permissions, role_permissions, roles, user_roles};

use super::rbac_runtime::invalidate_user_rbac_caches;

pub(crate) async fn assign_role_permissions_via_store(
    db: &impl ConnectionTrait,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()> {
    record_authz_entrypoint_call("assign_role_permissions_via_store", "core_runtime");
    let role_model = get_or_create_role(db, tenant_id, &role).await?;

    match user_roles::Entity::insert(user_roles::ActiveModel {
        id: ActiveValue::Set(rustok_core::generate_id()),
        user_id: ActiveValue::Set(*user_id),
        role_id: ActiveValue::Set(role_model.id),
    })
    .on_conflict(
        OnConflict::columns([user_roles::Column::UserId, user_roles::Column::RoleId])
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await
    {
        Ok(_) | Err(sea_orm::DbErr::RecordNotInserted) => {}
        Err(err) => return Err(err.into()),
    }

    for permission in Rbac::permissions_for_role(&role).iter() {
        let permission_model = get_or_create_permission(db, tenant_id, permission).await?;

        match role_permissions::Entity::insert(role_permissions::ActiveModel {
            id: ActiveValue::Set(rustok_core::generate_id()),
            role_id: ActiveValue::Set(role_model.id),
            permission_id: ActiveValue::Set(permission_model.id),
        })
        .on_conflict(
            OnConflict::columns([
                role_permissions::Column::RoleId,
                role_permissions::Column::PermissionId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(db)
        .await
        {
            Ok(_) | Err(sea_orm::DbErr::RecordNotInserted) => {}
            Err(err) => return Err(err.into()),
        }
    }

    invalidate_user_rbac_caches(tenant_id, user_id).await;

    Ok(())
}

pub(crate) async fn replace_user_role_via_store(
    db: &impl ConnectionTrait,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()> {
    record_authz_entrypoint_call("replace_user_role_via_store", "core_runtime");
    remove_tenant_role_assignments_via_store(db, user_id, tenant_id).await?;

    assign_role_permissions_via_store(db, user_id, tenant_id, role).await
}

pub(crate) async fn remove_tenant_role_assignments_via_store(
    db: &impl ConnectionTrait,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
) -> Result<()> {
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

    invalidate_user_rbac_caches(tenant_id, user_id).await;

    Ok(())
}

pub(crate) async fn remove_user_role_assignment_via_store(
    db: &impl ConnectionTrait,
    user_id: &uuid::Uuid,
    tenant_id: &uuid::Uuid,
    role: UserRole,
) -> Result<()> {
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

    invalidate_user_rbac_caches(tenant_id, user_id).await;

    Ok(())
}

async fn get_or_create_role(
    db: &impl ConnectionTrait,
    tenant_id: &uuid::Uuid,
    role: &UserRole,
) -> Result<roles::Model> {
    let role_slug = role.to_string();

    if let Some(existing) = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(*tenant_id))
        .filter(roles::Column::Slug.eq(&role_slug))
        .one(db)
        .await?
    {
        return Ok(existing);
    }

    roles::Entity::insert(roles::ActiveModel {
        id: ActiveValue::Set(rustok_core::generate_id()),
        tenant_id: ActiveValue::Set(*tenant_id),
        name: ActiveValue::Set(role_slug.clone()),
        slug: ActiveValue::Set(role_slug),
        description: ActiveValue::Set(None),
        is_system: ActiveValue::Set(true),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    })
    .on_conflict(
        OnConflict::columns([roles::Column::TenantId, roles::Column::Slug])
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;

    roles::Entity::find()
        .filter(roles::Column::TenantId.eq(*tenant_id))
        .filter(roles::Column::Slug.eq(role.to_string()))
        .one(db)
        .await?
        .ok_or(Error::InternalServerError)
}

async fn get_or_create_permission(
    db: &impl ConnectionTrait,
    tenant_id: &uuid::Uuid,
    permission: &Permission,
) -> Result<permissions::Model> {
    let resource_str = permission.resource.to_string();
    let action_str = permission.action.to_string();

    if let Some(existing) = permissions::Entity::find()
        .filter(permissions::Column::TenantId.eq(*tenant_id))
        .filter(permissions::Column::Resource.eq(&resource_str))
        .filter(permissions::Column::Action.eq(&action_str))
        .one(db)
        .await?
    {
        return Ok(existing);
    }

    permissions::Entity::insert(permissions::ActiveModel {
        id: ActiveValue::Set(rustok_core::generate_id()),
        tenant_id: ActiveValue::Set(*tenant_id),
        resource: ActiveValue::Set(resource_str.clone()),
        action: ActiveValue::Set(action_str.clone()),
        description: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
    })
    .on_conflict(
        OnConflict::columns([
            permissions::Column::TenantId,
            permissions::Column::Resource,
            permissions::Column::Action,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec(db)
    .await?;

    permissions::Entity::find()
        .filter(permissions::Column::TenantId.eq(*tenant_id))
        .filter(permissions::Column::Resource.eq(resource_str))
        .filter(permissions::Column::Action.eq(action_str))
        .one(db)
        .await?
        .ok_or(Error::InternalServerError)
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
    use crate::models::_entities::{roles, user_roles};
    use crate::models::{tenants, users};
    use chrono::Utc;
    use migration::Migrator;
    use rustok_core::{UserRole, UserStatus};
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
