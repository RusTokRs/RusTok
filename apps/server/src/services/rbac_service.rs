use crate::error::Result;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use tracing::{debug, warn};

use rustok_core::UserRole;

use rustok_api::{Permission, has_effective_permission};
use rustok_rbac::PermissionResolver;
use rustok_telemetry::metrics;

use super::rbac_persistence::replace_user_role_via_store;
use super::rbac_request_scope::{
    permissions_for as scoped_permissions_for, role_for as scoped_role_for,
};
pub use super::rbac_runtime::RbacResolverMetricsSnapshot;
use super::rbac_runtime::{
    AuthorizationCheck, ServerRuntimePermissionResolver,
    authorize_request as authorize_rbac_request,
    invalidate_user_permissions_cache as invalidate_permission_runtime_cache,
    invalidate_user_rbac_caches as invalidate_rbac_runtime_caches,
    metrics_snapshot as rbac_metrics_snapshot,
    observe_authorization_decision as observe_rbac_authorization_decision,
    record_claim_role_mismatch as record_rbac_claim_role_mismatch, record_permission_cache_result,
    record_permission_lookup_latency as record_rbac_permission_lookup_latency,
    resolver as rbac_runtime_resolver,
};

pub struct RbacService;

impl RbacService {
    pub async fn invalidate_user_permissions_cache(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        invalidate_permission_runtime_cache(tenant_id, user_id).await;
    }

    pub async fn invalidate_user_rbac_caches(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        invalidate_rbac_runtime_caches(tenant_id, user_id).await;
    }

    pub fn record_claim_role_mismatch() {
        record_rbac_claim_role_mismatch();
    }

    fn record_authz_entrypoint_call(entry_point: &str, path: &str) {
        metrics::record_module_entrypoint_call("rbac", entry_point, path);
    }

    pub fn metrics_snapshot() -> RbacResolverMetricsSnapshot {
        rbac_metrics_snapshot()
    }

    pub async fn has_permission(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permission: &Permission,
    ) -> Result<bool> {
        Self::record_authz_entrypoint_call("has_permission", "library");

        if let Some(permissions) = scoped_permissions_for(tenant_id, user_id) {
            let allowed = has_effective_permission(&permissions, required_permission);
            debug!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                permissions_count = permissions.len(),
                allowed,
                "rbac request-scoped decision (single permission check)"
            );
            return Ok(allowed);
        }

        let outcome = authorize_rbac_request(
            db,
            tenant_id,
            user_id,
            AuthorizationCheck::Single(required_permission),
        )
        .await?;
        let decision = outcome.decision;
        let allowed = decision.allowed;
        let denied = decision.denied_reason.clone();
        let permissions_count = decision.permissions_count;
        let cache_hit = decision.cache_hit;
        let latency_ms = outcome.latency_ms;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permission = %required_permission,
            engine = ?decision.engine,
            permissions_count,
            cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            allowed,
            latency_ms,
            "rbac resolver decision (single permission check)"
        );

        if let Some((_, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                denied_reason = %denied_reason,
                "rbac deny: missing required permission"
            );
        }

        observe_rbac_authorization_decision(&decision, latency_ms);

        Ok(allowed)
    }

    pub async fn has_any_permission(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool> {
        Self::record_authz_entrypoint_call("has_any_permission", "library");

        if let Some(permissions) = scoped_permissions_for(tenant_id, user_id) {
            let allowed = required_permissions
                .iter()
                .any(|required| has_effective_permission(&permissions, required));
            debug!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                permissions_count = permissions.len(),
                allowed,
                "rbac request-scoped decision (any-permission check)"
            );
            return Ok(allowed);
        }

        let outcome = authorize_rbac_request(
            db,
            tenant_id,
            user_id,
            AuthorizationCheck::Any(required_permissions),
        )
        .await?;
        let decision = outcome.decision;
        let allowed = decision.allowed;
        let denied = decision.denied_reason.clone();
        let permissions_count = decision.permissions_count;
        let cache_hit = decision.cache_hit;
        let latency_ms = outcome.latency_ms;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permissions = ?required_permissions,
            engine = ?decision.engine,
            permissions_count,
            cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            allowed,
            latency_ms,
            "rbac resolver decision (any-permission check)"
        );

        if let Some((_, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                denied_reason = %denied_reason,
                "rbac deny: none of required permissions granted"
            );
        }

        observe_rbac_authorization_decision(&decision, latency_ms);

        Ok(allowed)
    }

    pub async fn has_all_permissions(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool> {
        Self::record_authz_entrypoint_call("has_all_permissions", "library");

        if let Some(permissions) = scoped_permissions_for(tenant_id, user_id) {
            let allowed = required_permissions
                .iter()
                .all(|required| has_effective_permission(&permissions, required));
            debug!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                permissions_count = permissions.len(),
                allowed,
                "rbac request-scoped decision (all-permissions check)"
            );
            return Ok(allowed);
        }

        let outcome = authorize_rbac_request(
            db,
            tenant_id,
            user_id,
            AuthorizationCheck::All(required_permissions),
        )
        .await?;
        let decision = outcome.decision;
        let allowed = decision.allowed;
        let missing_permissions = decision.missing_permissions.clone();
        let denied = decision.denied_reason.clone();
        let permissions_count = decision.permissions_count;
        let cache_hit = decision.cache_hit;
        let latency_ms = outcome.latency_ms;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permissions = ?required_permissions,
            engine = ?decision.engine,
            permissions_count,
            cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            missing_permissions = ?missing_permissions,
            allowed,
            latency_ms,
            "rbac resolver decision (all-permission check)"
        );

        if let Some((_, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                denied_reason = %denied_reason,
                missing_permissions = ?missing_permissions,
                "rbac deny: not all required permissions granted"
            );
        }

        observe_rbac_authorization_decision(&decision, latency_ms);

        Ok(allowed)
    }

    pub async fn get_user_permissions(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>> {
        Self::record_authz_entrypoint_call("get_user_permissions", "library");

        if let Some(permissions) = scoped_permissions_for(tenant_id, user_id) {
            debug!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                permissions_count = permissions.len(),
                "rbac request-scoped permission lookup"
            );
            return Ok(permissions);
        }

        let resolver = Self::resolver(db);
        let started_at = std::time::Instant::now();
        let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;

        record_permission_cache_result(resolved.cache_hit);

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            cache_hit = resolved.cache_hit,
            permissions_count = resolved.permissions.len(),
            latency_ms = started_at.elapsed().as_millis(),
            "rbac resolver permission lookup"
        );

        record_rbac_permission_lookup_latency(started_at.elapsed().as_millis() as u64);

        Ok(resolved.permissions)
    }

    pub async fn get_user_role(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<UserRole> {
        Self::record_authz_entrypoint_call("get_user_role", "library");

        if let Some(role) = scoped_role_for(tenant_id, user_id) {
            return Ok(role);
        }

        let permissions = Self::get_user_permissions(db, tenant_id, user_id).await?;
        Ok(crate::context::infer_user_role_from_permissions(
            &permissions,
        ))
    }

    pub async fn get_user_ids_for_role(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<Vec<uuid::Uuid>> {
        use crate::models::_entities::{roles, user_roles};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        Self::record_authz_entrypoint_call("get_user_ids_for_role", "library");

        let role_ids = roles::Entity::find()
            .select_only()
            .column(roles::Column::Id)
            .filter(roles::Column::TenantId.eq(*tenant_id))
            .filter(roles::Column::Slug.eq(role.to_string()))
            .into_tuple::<uuid::Uuid>()
            .all(db)
            .await?;

        if role_ids.is_empty() {
            return Ok(Vec::new());
        }

        user_roles::Entity::find()
            .select_only()
            .column(user_roles::Column::UserId)
            .filter(user_roles::Column::RoleId.is_in(role_ids))
            .into_tuple::<uuid::Uuid>()
            .all(db)
            .await
            .map_err(Into::into)
    }

    fn resolver(db: &DatabaseConnection) -> ServerRuntimePermissionResolver {
        rbac_runtime_resolver(db)
    }

    #[cfg(test)]
    pub(crate) async fn assign_role_permissions(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("assign_role_permissions", "internal");
        let resolver = Self::resolver(db);
        resolver
            .assign_role_permissions(tenant_id, user_id, role)
            .await
    }

    /// Transaction-only compatibility alias for legacy auth lifecycle composition.
    /// New code must use `replace_user_role_in_transaction` or `replace_user_role_committed`.
    pub(crate) async fn replace_user_role(
        db: &impl ConnectionTrait,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("replace_user_role_legacy_transaction", "internal");
        replace_user_role_via_store(db, user_id, tenant_id, role).await
    }
}

#[cfg(test)]
mod tests {
    use super::RbacService;
    use crate::models::{tenants, users};
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use crate::services::rbac_runtime::reset_metrics_for_tests as reset_rbac_metrics_for_tests;
    use chrono::Utc;
    use rustok_api::Permission;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ConnectionTrait, EntityTrait, Set};
    use serial_test::serial;

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
    #[serial]
    async fn has_permission_records_cache_miss_then_hit() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) = insert_tenant_and_user(
            &db,
            "test-tenant-cache-metrics",
            "cache-metrics@example.com",
        )
        .await;

        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("role assignment should succeed");
        RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
        reset_rbac_metrics_for_tests();

        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::PRODUCTS_CREATE)
                .await
                .expect("first permission check should succeed")
        );
        let after_first = RbacService::metrics_snapshot();
        assert_eq!(after_first.permission_cache_misses, 1);
        assert_eq!(after_first.permission_cache_hits, 0);

        assert!(
            RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::PRODUCTS_CREATE)
                .await
                .expect("second permission check should succeed")
        );
        let after_second = RbacService::metrics_snapshot();
        assert_eq!(after_second.permission_cache_misses, 1);
        assert_eq!(after_second.permission_cache_hits, 1);
    }

    #[tokio::test]
    async fn request_scope_prevents_database_permission_restoration() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_id, user_id) =
            insert_tenant_and_user(&db, "test-request-scope", "request-scope@example.com").await;
        RbacService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Admin)
            .await
            .expect("admin role assignment should succeed");

        let scope = RbacRequestScope::new(
            tenant_id,
            user_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );
        with_rbac_request_scope(Some(scope), async {
            assert!(
                RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::USERS_READ)
                    .await
                    .expect("scoped read check")
            );
            assert!(
                !RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE
                )
                .await
                .expect("scoped manage check")
            );
            assert_eq!(
                RbacService::get_user_permissions(&db, &tenant_id, &user_id)
                    .await
                    .expect("scoped permission lookup"),
                vec![Permission::USERS_READ]
            );
            assert_eq!(
                RbacService::get_user_role(&db, &tenant_id, &user_id)
                    .await
                    .expect("scoped role lookup"),
                UserRole::Admin
            );
        })
        .await;
    }

    #[test]
    #[serial]
    fn claim_role_mismatch_counter_increments() {
        let before = RbacService::metrics_snapshot().claim_role_mismatch_total;
        RbacService::record_claim_role_mismatch();
        let after = RbacService::metrics_snapshot().claim_role_mismatch_total;

        assert_eq!(after, before + 1);
    }
}
