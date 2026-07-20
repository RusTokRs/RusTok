use std::time::Duration;

use chrono::Utc;
use rustok_api::Permission;
use rustok_cache::{CacheService, VersionedCacheInvalidation};
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::_entities::{permissions, role_permissions, roles, user_roles};
use rustok_server::models::{tenants, users};
use rustok_server::services::rbac_cache_invalidation::{
    RBAC_PERMISSION_INVALIDATION_CHANNEL, start_rbac_cache_invalidation_listener,
};
use rustok_server::services::rbac_service::RbacService;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use serial_test::serial;
use uuid::Uuid;

async fn insert_tenant(db: &sea_orm::DatabaseConnection) -> Uuid {
    let tenant_id = rustok_core::generate_id();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("RBAC invalidation tenant".to_string()),
        slug: Set("rbac-distributed-invalidation".to_string()),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await
    .expect("insert tenant");
    tenant_id
}

async fn insert_user(db: &sea_orm::DatabaseConnection, tenant_id: Uuid, email: &str) -> Uuid {
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
    .expect("insert user");
    user_id
}

async fn publish_generation(cache: &CacheService, tenant_id: Uuid, user_id: Uuid, generation: u64) {
    let message = VersionedCacheInvalidation::new(
        RBAC_PERMISSION_INVALIDATION_CHANNEL,
        format!("{tenant_id}:{user_id}"),
        generation,
        1_000,
    )
    .expect("build RBAC invalidation")
    .to_message()
    .expect("encode RBAC invalidation");
    let outcome = cache.publish_invalidation(message).await;
    assert!(outcome.local_subscribers > 0);
}

async fn wait_for_permission(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    expected: bool,
) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let actual =
                RbacService::has_permission(db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE)
                    .await
                    .expect("permission lookup");
            if actual == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("RBAC invalidation was not applied before timeout");
}

#[tokio::test]
#[serial]
async fn local_generation_delivery_invalidates_target_and_gap_clears_all_snapshots() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = insert_tenant(&db).await;
    let first_user = insert_user(&db, tenant_id, "rbac-first@example.com").await;
    let second_user = insert_user(&db, tenant_id, "rbac-second@example.com").await;

    let context = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache = CacheService::from_url(None);
    start_rbac_cache_invalidation_listener(&context, cache.clone())
        .await
        .expect("start RBAC invalidation listener");

    let writer = RbacRoleAssignmentDbWriter::new(db.clone());
    for user_id in [first_user, second_user] {
        writer
            .assign_role_permissions(tenant_id, user_id, UserRole::Admin)
            .await
            .expect("assign admin role");
        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE,)
                .await
                .expect("prime admin permission snapshot")
        );
    }

    let admin_role = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(tenant_id))
        .filter(roles::Column::Slug.eq(UserRole::Admin.to_string()))
        .one(&db)
        .await
        .expect("load admin role")
        .expect("admin role exists");
    user_roles::Entity::delete_many()
        .filter(user_roles::Column::UserId.eq(first_user))
        .filter(user_roles::Column::RoleId.eq(admin_role.id))
        .exec(&db)
        .await
        .expect("remove first user's admin relation without cache invalidation");

    publish_generation(&cache, tenant_id, first_user, 1).await;
    wait_for_permission(&db, tenant_id, first_user, false).await;
    assert!(
        RbacService::has_permission(&db, &tenant_id, &second_user, &Permission::SETTINGS_MANAGE,)
            .await
            .expect("unrelated cached permission remains valid after targeted invalidation")
    );

    let settings_manage = permissions::Entity::find()
        .filter(permissions::Column::TenantId.eq(tenant_id))
        .filter(permissions::Column::Resource.eq(Permission::SETTINGS_MANAGE.resource.to_string()))
        .filter(permissions::Column::Action.eq(Permission::SETTINGS_MANAGE.action.to_string()))
        .one(&db)
        .await
        .expect("load settings manage permission")
        .expect("settings manage permission exists");
    role_permissions::Entity::delete_many()
        .filter(role_permissions::Column::RoleId.eq(admin_role.id))
        .filter(role_permissions::Column::PermissionId.eq(settings_manage.id))
        .exec(&db)
        .await
        .expect("remove admin permission link without cache invalidation");

    publish_generation(&cache, tenant_id, first_user, 2).await;
    publish_generation(&cache, tenant_id, second_user, 3).await;
    wait_for_permission(&db, tenant_id, second_user, false).await;
}
