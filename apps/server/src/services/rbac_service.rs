use loco_rs::Error;
use loco_rs::Result;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use tracing::{debug, warn};

use rustok_core::{Permission, UserRole};
use rustok_rbac::PermissionResolver;
use rustok_telemetry::metrics;

use super::rbac_persistence::replace_user_role_via_store;
#[cfg(test)]
use super::rbac_runtime::authz_mode as runtime_authz_mode;
pub use super::rbac_runtime::RbacResolverMetricsSnapshot;
use super::rbac_runtime::{
    authorize_request as authorize_rbac_request,
    invalidate_user_permissions_cache as invalidate_permission_runtime_cache,
    invalidate_user_rbac_caches as invalidate_rbac_runtime_caches,
    metrics_snapshot as rbac_metrics_snapshot,
    observe_authorization_decision as observe_rbac_authorization_decision,
    record_claim_role_mismatch as record_rbac_claim_role_mismatch, record_permission_cache_result,
    record_permission_lookup_latency as record_rbac_permission_lookup_latency,
    record_shadow_compare_failure as record_rbac_shadow_compare_failure,
    resolver as rbac_runtime_resolver, AuthorizationCheck, ServerRuntimePermissionResolver,
};

pub struct RbacService;

impl RbacService {
    #[cfg(test)]
    pub(crate) fn should_run_casbin_shadow() -> bool {
        runtime_authz_mode().should_run_casbin_shadow()
    }

    pub async fn invalidate_user_permissions_cache(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        invalidate_permission_runtime_cache(tenant_id, user_id).await;
    }

    pub async fn invalidate_user_rbac_caches(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        invalidate_rbac_runtime_caches(tenant_id, user_id).await;
    }

    pub fn record_claim_role_mismatch() {
        record_rbac_claim_role_mismatch();
    }

    pub fn record_shadow_compare_failure() {
        record_rbac_shadow_compare_failure();
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

        if let Err(error) = observe_rbac_authorization_decision(
            db,
            tenant_id,
            user_id,
            &decision,
            AuthorizationCheck::Single(required_permission).shadow_check(),
            latency_ms,
        )
        .await
        {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                error = %error,
                "rbac shadow runtime compare failed"
            );
        }

        Ok(allowed)
    }

    pub async fn has_any_permission(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool> {
        Self::record_authz_entrypoint_call("has_any_permission", "library");
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

        if let Err(error) = observe_rbac_authorization_decision(
            db,
            tenant_id,
            user_id,
            &decision,
            AuthorizationCheck::Any(required_permissions).shadow_check(),
            latency_ms,
        )
        .await
        {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac shadow runtime compare failed"
            );
        }

        Ok(allowed)
    }

    pub async fn has_all_permissions(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool> {
        Self::record_authz_entrypoint_call("has_all_permissions", "library");
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
            "rbac resolver decision (all-permissions check)"
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

        if let Err(error) = observe_rbac_authorization_decision(
            db,
            tenant_id,
            user_id,
            &decision,
            AuthorizationCheck::All(required_permissions).shadow_check(),
            latency_ms,
        )
        .await
        {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac shadow runtime compare failed"
            );
        }

        Ok(allowed)
    }

    pub async fn get_user_permissions(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>> {
        Self::record_authz_entrypoint_call("get_user_permissions", "library");
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

    pub async fn assign_role_permissions(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("assign_role_permissions", "library");
        let resolver = Self::resolver(db);
        resolver
            .assign_role_permissions(tenant_id, user_id, role)
            .await
    }

    pub async fn replace_user_role(
        db: &impl ConnectionTrait,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("replace_user_role", "library");
        replace_user_role_via_store(db, user_id, tenant_id, role).await
    }

    pub async fn remove_tenant_role_assignments(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("remove_tenant_role_assignments", "library");
        let resolver = Self::resolver(db);
        resolver
            .remove_tenant_role_assignments(tenant_id, user_id)
            .await
    }

    pub async fn remove_user_role_assignment(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::record_authz_entrypoint_call("remove_user_role_assignment", "library");
        let resolver = Self::resolver(db);
        resolver
            .remove_user_role_assignment(tenant_id, user_id, role)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::RbacService;
    use crate::models::{tenants, users};
    use crate::services::rbac_runtime::reset_metrics_for_tests as reset_rbac_metrics_for_tests;
    use chrono::Utc;
    use migration::Migrator;
    use rustok_core::{Permission, UserRole, UserStatus};
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ConnectionTrait, EntityTrait, Set};
    use serial_test::serial;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const AUTHZ_MODE_ENV: &str = "RUSTOK_RBAC_AUTHZ_MODE";

    struct EnvVarGuard {
        _lock: MutexGuard<'static, ()>,
        name: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn lock(name: &'static str) -> Self {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let lock = LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("env lock");
            let previous = std::env::var(name).ok();
            Self {
                _lock: lock,
                name,
                previous,
            }
        }

        fn set(&self, value: &str) {
            // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
            unsafe {
                std::env::set_var(self.name, value);
            }
        }

        fn remove(&self) {
            // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
            unsafe {
                std::env::remove_var(self.name);
            }
        }

        fn previous(&self) -> Option<&str> {
            self.previous.as_deref()
        }

        fn restore(&self) {
            if let Some(previous) = self.previous.as_ref() {
                // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
                unsafe {
                    std::env::set_var(self.name, previous);
                }
            } else {
                // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
                unsafe {
                    std::env::remove_var(self.name);
                }
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            self.restore();
        }
    }

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

        assert!(RbacService::has_permission(
            &db,
            &tenant_id,
            &user_id,
            &Permission::PRODUCTS_CREATE
        )
        .await
        .expect("first permission check should succeed"));
        let after_first = RbacService::metrics_snapshot();
        assert_eq!(after_first.permission_cache_misses, 1);
        assert_eq!(after_first.permission_cache_hits, 0);

        assert!(RbacService::has_permission(
            &db,
            &tenant_id,
            &user_id,
            &Permission::PRODUCTS_CREATE
        )
        .await
        .expect("second permission check should succeed"));
        let after_second = RbacService::metrics_snapshot();
        assert_eq!(after_second.permission_cache_misses, 1);
        assert_eq!(after_second.permission_cache_hits, 1);
    }

    #[test]
    #[serial]
    fn claim_role_mismatch_counter_increments() {
        let before = RbacService::metrics_snapshot().claim_role_mismatch_total;
        RbacService::record_claim_role_mismatch();
        let after = RbacService::metrics_snapshot().claim_role_mismatch_total;

        assert_eq!(after, before + 1);
    }

    #[test]
    #[serial]
    fn authz_mode_defaults_to_relation_only_when_env_missing() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.remove();

        assert!(!RbacService::should_run_casbin_shadow());
    }

    #[test]
    #[serial]
    fn authz_mode_enables_casbin_shadow_from_env() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("casbin_shadow");

        assert!(RbacService::should_run_casbin_shadow());
    }

    #[test]
    #[serial]
    fn authz_mode_enables_casbin_only_engine_from_env() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("casbin_only");

        assert!(super::runtime_authz_mode().is_casbin_only());
    }

    #[test]
    #[serial]
    fn authz_mode_guard_restores_previous_env_value() {
        let previous = {
            let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
            let previous = env.previous().map(ToOwned::to_owned);
            env.set("casbin_shadow");
            assert!(RbacService::should_run_casbin_shadow());
            previous
        };

        assert_eq!(std::env::var(AUTHZ_MODE_ENV).ok(), previous);
    }

    #[test]
    #[serial]
    fn shadow_compare_failure_counter_increments() {
        let before = RbacService::metrics_snapshot().shadow_compare_failures_total;
        RbacService::record_shadow_compare_failure();
        let after = RbacService::metrics_snapshot().shadow_compare_failures_total;

        assert_eq!(after, before + 1);
    }
}
