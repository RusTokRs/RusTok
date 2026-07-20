use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;
pub use rustok_rbac::RbacConsistencyStats;

pub async fn load_rbac_consistency_stats(
    ctx: &ServerRuntimeContext,
) -> Result<RbacConsistencyStats> {
    rustok_rbac::load_consistency_stats(ctx.db())
        .await
        .map_err(|error| Error::Message(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{RbacConsistencyStats, load_rbac_consistency_stats};
    use crate::common::settings::RustokSettings;
    use crate::models::_entities::{permissions, role_permissions, roles, user_roles};
    use crate::models::{tenants, users};
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use chrono::Utc;
    use rustok_core::UserStatus;
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{EntityTrait, Set};

    #[test]
    fn stats_default_is_zeroed() {
        let stats = RbacConsistencyStats::default();
        assert_eq!(stats.users_without_roles_total, 0);
        assert_eq!(stats.orphan_user_roles_total, 0);
        assert_eq!(stats.orphan_role_permissions_total, 0);
        assert_eq!(stats.cross_tenant_user_roles_total, 0);
        assert_eq!(stats.cross_tenant_role_permissions_total, 0);
        assert_eq!(stats.reserved_role_slug_collisions_total, 0);
        assert_eq!(stats.system_roles_with_permission_drift_total, 0);
        assert_eq!(stats.missing_system_role_permissions_total, 0);
        assert_eq!(stats.extra_system_role_permissions_total, 0);
    }

    #[tokio::test]
    async fn database_rejects_cross_tenant_relations_and_reports_remaining_role_corruption() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = rustok_core::generate_id();
        let tenant_b = rustok_core::generate_id();
        for (id, slug) in [(tenant_a, "consistency-a"), (tenant_b, "consistency-b")] {
            tenants::Entity::insert(tenants::ActiveModel {
                id: Set(id),
                name: Set(slug.to_string()),
                slug: Set(slug.to_string()),
                domain: Set(None),
                settings: Set(serde_json::json!({})),
                default_locale: Set("en".to_string()),
                is_active: Set(true),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
            })
            .exec(&db)
            .await
            .expect("insert tenant");
        }

        let user_id = rustok_core::generate_id();
        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_a),
            email: Set("rbac-consistency@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            status: Set(UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("insert user");

        let foreign_role_id = rustok_core::generate_id();
        roles::Entity::insert(roles::ActiveModel {
            id: Set(foreign_role_id),
            tenant_id: Set(tenant_b),
            name: Set("Manager".to_string()),
            slug: Set("manager".to_string()),
            description: Set(None),
            is_system: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("insert foreign role");
        assert!(
            user_roles::Entity::insert(user_roles::ActiveModel {
                id: Set(rustok_core::generate_id()),
                user_id: Set(user_id),
                role_id: Set(foreign_role_id),
            })
            .exec(&db)
            .await
            .is_err()
        );

        let permission_id = rustok_core::generate_id();
        permissions::Entity::insert(permissions::ActiveModel {
            id: Set(permission_id),
            tenant_id: Set(tenant_a),
            resource: Set("settings".to_string()),
            action: Set("manage".to_string()),
            description: Set(None),
            created_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("insert permission");
        assert!(
            role_permissions::Entity::insert(role_permissions::ActiveModel {
                id: Set(rustok_core::generate_id()),
                role_id: Set(foreign_role_id),
                permission_id: Set(permission_id),
            })
            .exec(&db)
            .await
            .is_err()
        );

        roles::Entity::insert(roles::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_a),
            name: Set("Custom Admin".to_string()),
            slug: Set("admin".to_string()),
            description: Set(None),
            is_system: Set(false),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("insert reserved slug collision");

        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let stats = load_rbac_consistency_stats(&ctx)
            .await
            .expect("load consistency stats");

        assert_eq!(stats.cross_tenant_user_roles_total, 0);
        assert_eq!(stats.cross_tenant_role_permissions_total, 0);
        assert_eq!(stats.reserved_role_slug_collisions_total, 1);
        assert_eq!(stats.system_roles_with_permission_drift_total, 1);
        assert!(stats.missing_system_role_permissions_total > 0);
        assert_eq!(stats.extra_system_role_permissions_total, 0);
        assert_eq!(stats.orphan_user_roles_total, 0);
        assert_eq!(stats.orphan_role_permissions_total, 0);
    }
}
