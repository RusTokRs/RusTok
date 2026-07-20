use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::models::{
    _entities::{permissions, role_permissions, roles, user_roles},
    users,
};
use rustok_api::{Action, Permission, Resource};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::rbac_service::RbacService;

impl RbacService {
    /// Resolve the canonical database-backed permission snapshot without using
    /// the process-local authorization cache.
    ///
    /// Authentication must observe role revocation and demotion immediately.
    /// Authorization entry points use `get_user_permissions`, which honors the
    /// immutable request scope and may use the runtime cache outside a request.
    /// Accepting any `ConnectionTrait` keeps hierarchy and delegation checks on
    /// the same transaction that owns their serialization lock.
    pub async fn get_user_permissions_authoritative<C>(
        db: &C,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>>
    where
        C: ConnectionTrait,
    {
        let user_belongs_to_tenant = users::Entity::find_by_id(*user_id)
            .filter(users::Column::TenantId.eq(*tenant_id))
            .one(db)
            .await?
            .is_some();
        if !user_belongs_to_tenant {
            return Ok(Vec::new());
        }

        let assigned_roles = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .all(db)
            .await?;
        if assigned_roles.is_empty() {
            return Ok(Vec::new());
        }

        let assigned_role_ids = assigned_roles
            .into_iter()
            .map(|assignment| assignment.role_id)
            .collect::<Vec<_>>();
        let tenant_roles = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(*tenant_id))
            .filter(roles::Column::Id.is_in(assigned_role_ids))
            .all(db)
            .await?;
        if tenant_roles.is_empty() {
            return Ok(Vec::new());
        }

        let tenant_role_ids = tenant_roles
            .into_iter()
            .map(|role| role.id)
            .collect::<Vec<_>>();
        let links = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.is_in(tenant_role_ids))
            .all(db)
            .await?;
        if links.is_empty() {
            return Ok(Vec::new());
        }

        let permission_ids = links
            .into_iter()
            .map(|link| link.permission_id)
            .collect::<Vec<_>>();
        let rows = permissions::Entity::find()
            .filter(permissions::Column::TenantId.eq(*tenant_id))
            .filter(permissions::Column::Id.is_in(permission_ids))
            .all(db)
            .await?;

        let mut seen = HashSet::new();
        let mut resolved = Vec::with_capacity(rows.len());
        for row in rows {
            let resource = row
                .resource
                .parse::<Resource>()
                .map_err(Error::BadRequest)?;
            let action = row.action.parse::<Action>().map_err(Error::BadRequest)?;
            let permission = Permission::new(resource, action);
            if seen.insert(permission) {
                resolved.push(permission);
            }
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::RbacService;
    use crate::models::{
        _entities::{roles, user_roles},
        tenants, users,
    };
    use chrono::Utc;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

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
    async fn authoritative_permissions_require_user_tenant_membership() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (_tenant_a, user_a) =
            insert_tenant_and_user(&db, "authoritative-tenant-a", "authoritative-a@example.com")
                .await;
        let (tenant_b, user_b) =
            insert_tenant_and_user(&db, "authoritative-tenant-b", "authoritative-b@example.com")
                .await;

        RbacService::assign_role_permissions(&db, &user_b, &tenant_b, UserRole::Admin)
            .await
            .expect("assign foreign tenant role");
        let foreign_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_b))
            .filter(roles::Column::Slug.eq(UserRole::Admin.to_string()))
            .one(&db)
            .await
            .expect("load foreign role")
            .expect("foreign role should exist");

        assert!(
            user_roles::Entity::insert(user_roles::ActiveModel {
                id: Set(rustok_core::generate_id()),
                user_id: Set(user_a),
                role_id: Set(foreign_role.id),
            })
            .exec(&db)
            .await
            .is_err()
        );

        let permissions = RbacService::get_user_permissions_authoritative(&db, &tenant_b, &user_a)
            .await
            .expect("resolve authoritative permissions");

        assert!(permissions.is_empty());
    }

    #[tokio::test]
    async fn authoritative_permissions_resolve_for_matching_user_tenant() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "authoritative-matching-tenant",
            "authoritative-match@example.com",
        )
        .await;

        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("assign matching tenant role");

        let permissions =
            RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id)
                .await
                .expect("resolve authoritative permissions");

        assert!(!permissions.is_empty());
    }
}
