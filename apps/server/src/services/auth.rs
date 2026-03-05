use async_trait::async_trait;
use loco_rs::prelude::*;
use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::{
    sea_query::OnConflict, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use rustok_core::{Action, Permission, Rbac, Resource, UserRole};
use rustok_rbac::{
    authorize_all_permissions, authorize_any_permission, authorize_permission,
    evaluate_casbin_shadow_comparison, evaluate_dual_read, invalidate_cached_permissions,
    DeniedReasonKind, DualReadOutcome, PermissionCache, PermissionResolver, RbacAuthzMode,
    RelationPermissionStore, RoleAssignmentStore, RuntimePermissionResolver, ShadowCheck,
};

use crate::models::_entities::{permissions, role_permissions, roles, user_roles, users};

pub struct AuthService;

type ServerRuntimePermissionResolver = RuntimePermissionResolver<
    SeaOrmRelationPermissionStore,
    MokaPermissionCache,
    ServerRoleAssignmentStore,
    Error,
>;

static USER_PERMISSION_CACHE: Lazy<Cache<(uuid::Uuid, uuid::Uuid), Vec<Permission>>> =
    Lazy::new(|| {
        Cache::builder()
            .max_capacity(20_000)
            .time_to_live(Duration::from_secs(60))
            .build()
    });

static USER_LEGACY_ROLE_CACHE: Lazy<Cache<(uuid::Uuid, uuid::Uuid), UserRole>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(20_000)
        .time_to_live(Duration::from_secs(60))
        .build()
});

#[derive(Debug, Clone, Copy)]
pub struct RbacResolverMetricsSnapshot {
    pub permission_cache_hits: u64,
    pub permission_cache_misses: u64,
    pub permission_checks_allowed: u64,
    pub permission_checks_denied: u64,
    pub permission_check_latency_ms_total: u64,
    pub permission_check_latency_samples: u64,
    pub permission_lookup_latency_ms_total: u64,
    pub permission_lookup_latency_samples: u64,
    pub denied_no_permissions_resolved: u64,
    pub denied_missing_permissions: u64,
    pub denied_unknown: u64,
    pub claim_role_mismatch_total: u64,
    pub decision_mismatch_total: u64,
    pub shadow_compare_failures_total: u64,
    pub engine_decisions_relation_total: u64,
    pub engine_decisions_casbin_total: u64,
    pub engine_mismatch_total: u64,
    pub engine_eval_duration_ms_total: u64,
    pub engine_eval_duration_samples: u64,
}

static RBAC_PERMISSION_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CHECKS_ALLOWED: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CHECKS_DENIED: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CHECK_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CHECK_LATENCY_SAMPLES: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_LOOKUP_LATENCY_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_LOOKUP_LATENCY_SAMPLES: AtomicU64 = AtomicU64::new(0);
static RBAC_DENIED_NO_PERMISSIONS_RESOLVED: AtomicU64 = AtomicU64::new(0);
static RBAC_DENIED_MISSING_PERMISSIONS: AtomicU64 = AtomicU64::new(0);
static RBAC_DENIED_UNKNOWN: AtomicU64 = AtomicU64::new(0);
static RBAC_CLAIM_ROLE_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_DECISION_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_SHADOW_COMPARE_FAILURES_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_DECISIONS_RELATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_DECISIONS_CASBIN_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_SAMPLES: AtomicU64 = AtomicU64::new(0);

impl AuthService {
    fn authz_mode() -> RbacAuthzMode {
        RbacAuthzMode::from_env()
    }

    fn is_dual_read_enabled() -> bool {
        Self::authz_mode().should_run_legacy_role_shadow()
    }

    fn should_run_casbin_shadow() -> bool {
        Self::authz_mode().should_run_casbin_shadow()
    }

    async fn load_legacy_role(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Option<UserRole>> {
        let cache_key = Self::cache_key(tenant_id, user_id);
        if let Some(cached_role) = USER_LEGACY_ROLE_CACHE.get(&cache_key).await {
            return Ok(Some(cached_role));
        }

        let user = users::Entity::find_by_id(*user_id)
            .filter(users::Column::TenantId.eq(*tenant_id))
            .one(db)
            .await?;

        if let Some(user) = user {
            USER_LEGACY_ROLE_CACHE
                .insert(cache_key, user.role.clone())
                .await;
            return Ok(Some(user.role));
        }

        Ok(None)
    }

    async fn shadow_compare_decision(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        shadow_check: ShadowCheck<'_>,
        relation_allowed: bool,
    ) -> Result<()> {
        if !Self::is_dual_read_enabled() {
            return Ok(());
        }

        let legacy_role = Self::load_legacy_role(db, tenant_id, user_id).await?;

        match evaluate_dual_read(legacy_role.as_ref(), shadow_check, relation_allowed) {
            DualReadOutcome::Skipped => {
                debug!(
                    tenant_id = %tenant_id,
                    user_id = %user_id,
                    "rbac dual-read skipped: user not found for legacy role"
                );
            }
            DualReadOutcome::Compared(shadow_decision) => {
                if shadow_decision.mismatch() {
                    if let Some(legacy_role) = legacy_role.as_ref() {
                        Self::record_decision_mismatch();
                        Self::log_shadow_mismatch(
                            tenant_id,
                            user_id,
                            shadow_check,
                            legacy_role,
                            shadow_decision.relation_allowed,
                            shadow_decision.legacy_allowed,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn shadow_compare_casbin_decision(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        shadow_check: ShadowCheck<'_>,
        relation_allowed: bool,
    ) -> Result<()> {
        if !Self::should_run_casbin_shadow() {
            return Ok(());
        }

        let started_at = Instant::now();
        let resolver = Self::resolver(db);
        let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;
        let casbin_shadow = evaluate_casbin_shadow_comparison(
            tenant_id,
            &resolved.permissions,
            shadow_check,
            relation_allowed,
        );

        let eval_latency_ms = started_at.elapsed().as_millis() as u64;
        Self::record_engine_decision("relation");
        Self::record_engine_decision("casbin");
        Self::record_engine_eval_duration(eval_latency_ms);

        if casbin_shadow.decision.mismatch() {
            Self::record_engine_mismatch();
            let shadow_check_mode = shadow_check.as_str();
            let checked_permissions_total = casbin_shadow.checked_permissions_total();
            for permission in casbin_shadow.checked_permissions {
                warn!(
                    tenant_id = %tenant_id,
                    user_id = %user_id,
                    shadow_check = shadow_check_mode,
                    checked_permissions_total,
                    resource = %permission.resource,
                    action = %permission.action,
                    relation_decision = casbin_shadow.decision.relation_allowed,
                    casbin_decision = casbin_shadow.decision.casbin_allowed,
                    "rbac_engine_mismatch"
                );
            }
        }

        Ok(())
    }

    fn denied_reason_bucket(denied_reason_kind: DeniedReasonKind) {
        match denied_reason_kind {
            DeniedReasonKind::NoPermissionsResolved => {
                RBAC_DENIED_NO_PERMISSIONS_RESOLVED.fetch_add(1, Ordering::Relaxed);
            }
            DeniedReasonKind::MissingPermissions => {
                RBAC_DENIED_MISSING_PERMISSIONS.fetch_add(1, Ordering::Relaxed);
            }
            DeniedReasonKind::Unknown => {
                RBAC_DENIED_UNKNOWN.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn cache_key(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) -> (uuid::Uuid, uuid::Uuid) {
        (*tenant_id, *user_id)
    }

    fn log_shadow_mismatch(
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        shadow_check: ShadowCheck<'_>,
        legacy_role: &UserRole,
        relation_allowed: bool,
        legacy_allowed: bool,
    ) {
        let shadow_mode = shadow_check.as_str();
        match shadow_check {
            ShadowCheck::Single(required_permission) => {
                warn!(
                    tenant_id = %tenant_id,
                    user_id = %user_id,
                    shadow_check = shadow_mode,
                    required_permission = %required_permission,
                    legacy_role = %legacy_role,
                    relation_allowed,
                    legacy_allowed,
                    "rbac_decision_mismatch"
                );
            }
            ShadowCheck::Any(required_permissions) | ShadowCheck::All(required_permissions) => {
                warn!(
                    tenant_id = %tenant_id,
                    user_id = %user_id,
                    shadow_check = shadow_mode,
                    required_permissions = ?required_permissions,
                    legacy_role = %legacy_role,
                    relation_allowed,
                    legacy_allowed,
                    "rbac_decision_mismatch"
                );
            }
        }
    }

    pub async fn invalidate_user_permissions_cache(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        let cache = MokaPermissionCache;
        invalidate_cached_permissions(&cache, tenant_id, user_id).await;
    }

    pub async fn invalidate_user_rbac_caches(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        let cache = MokaPermissionCache;
        invalidate_cached_permissions(&cache, tenant_id, user_id).await;

        let cache_key = Self::cache_key(tenant_id, user_id);
        USER_LEGACY_ROLE_CACHE.invalidate(&cache_key).await;
    }

    fn record_permission_check_result(allowed: bool) {
        if allowed {
            RBAC_PERMISSION_CHECKS_ALLOWED.fetch_add(1, Ordering::Relaxed);
        } else {
            RBAC_PERMISSION_CHECKS_DENIED.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_permission_check_latency(latency_ms: u64) {
        RBAC_PERMISSION_CHECK_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
        RBAC_PERMISSION_CHECK_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);
    }

    fn record_permission_lookup_latency(latency_ms: u64) {
        RBAC_PERMISSION_LOOKUP_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
        RBAC_PERMISSION_LOOKUP_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_claim_role_mismatch() {
        RBAC_CLAIM_ROLE_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_decision_mismatch() {
        RBAC_DECISION_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_shadow_compare_failure() {
        RBAC_SHADOW_COMPARE_FAILURES_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    fn record_engine_decision(engine: &str) {
        match engine {
            "relation" => {
                RBAC_ENGINE_DECISIONS_RELATION_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
            "casbin" => {
                RBAC_ENGINE_DECISIONS_CASBIN_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn record_engine_mismatch() {
        RBAC_ENGINE_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    fn record_engine_eval_duration(latency_ms: u64) {
        RBAC_ENGINE_EVAL_DURATION_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
        RBAC_ENGINE_EVAL_DURATION_SAMPLES.fetch_add(1, Ordering::Relaxed);
    }

    pub fn metrics_snapshot() -> RbacResolverMetricsSnapshot {
        RbacResolverMetricsSnapshot {
            permission_cache_hits: RBAC_PERMISSION_CACHE_HITS.load(Ordering::Relaxed),
            permission_cache_misses: RBAC_PERMISSION_CACHE_MISSES.load(Ordering::Relaxed),
            permission_checks_allowed: RBAC_PERMISSION_CHECKS_ALLOWED.load(Ordering::Relaxed),
            permission_checks_denied: RBAC_PERMISSION_CHECKS_DENIED.load(Ordering::Relaxed),
            permission_check_latency_ms_total: RBAC_PERMISSION_CHECK_LATENCY_MS_TOTAL
                .load(Ordering::Relaxed),
            permission_check_latency_samples: RBAC_PERMISSION_CHECK_LATENCY_SAMPLES
                .load(Ordering::Relaxed),
            permission_lookup_latency_ms_total: RBAC_PERMISSION_LOOKUP_LATENCY_MS_TOTAL
                .load(Ordering::Relaxed),
            permission_lookup_latency_samples: RBAC_PERMISSION_LOOKUP_LATENCY_SAMPLES
                .load(Ordering::Relaxed),
            denied_no_permissions_resolved: RBAC_DENIED_NO_PERMISSIONS_RESOLVED
                .load(Ordering::Relaxed),
            denied_missing_permissions: RBAC_DENIED_MISSING_PERMISSIONS.load(Ordering::Relaxed),
            denied_unknown: RBAC_DENIED_UNKNOWN.load(Ordering::Relaxed),
            claim_role_mismatch_total: RBAC_CLAIM_ROLE_MISMATCH_TOTAL.load(Ordering::Relaxed),
            decision_mismatch_total: RBAC_DECISION_MISMATCH_TOTAL.load(Ordering::Relaxed),
            shadow_compare_failures_total: RBAC_SHADOW_COMPARE_FAILURES_TOTAL
                .load(Ordering::Relaxed),
            engine_decisions_relation_total: RBAC_ENGINE_DECISIONS_RELATION_TOTAL
                .load(Ordering::Relaxed),
            engine_decisions_casbin_total: RBAC_ENGINE_DECISIONS_CASBIN_TOTAL
                .load(Ordering::Relaxed),
            engine_mismatch_total: RBAC_ENGINE_MISMATCH_TOTAL.load(Ordering::Relaxed),
            engine_eval_duration_ms_total: RBAC_ENGINE_EVAL_DURATION_MS_TOTAL
                .load(Ordering::Relaxed),
            engine_eval_duration_samples: RBAC_ENGINE_EVAL_DURATION_SAMPLES.load(Ordering::Relaxed),
        }
    }

    pub async fn has_permission(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permission: &Permission,
    ) -> Result<bool> {
        let started_at = Instant::now();
        let resolver = Self::resolver(db);
        let decision =
            authorize_permission(&resolver, tenant_id, user_id, required_permission).await?;
        let allowed = decision.allowed;
        let denied = decision.denied_reason;
        let latency_ms = started_at.elapsed().as_millis() as u64;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permission = %required_permission,
            permissions_count = decision.permissions_count,
            cache_hit = decision.cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            allowed,
            latency_ms,
            "rbac resolver decision (single permission check)"
        );

        if let Some((denied_reason_kind, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                denied_reason = %denied_reason,
                "rbac deny: missing required permission"
            );
            Self::denied_reason_bucket(denied_reason_kind);
        }

        Self::record_permission_check_result(allowed);
        Self::record_permission_check_latency(latency_ms);
        if let Err(error) = Self::shadow_compare_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::Single(required_permission),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                error = %error,
                "rbac dual-read shadow compare failed"
            );
        }

        if let Err(error) = Self::shadow_compare_casbin_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::Single(required_permission),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permission = %required_permission,
                error = %error,
                "rbac casbin-shadow compare failed"
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
        let started_at = Instant::now();

        let resolver = Self::resolver(db);
        let decision =
            authorize_any_permission(&resolver, tenant_id, user_id, required_permissions).await?;
        let allowed = decision.allowed;
        let denied = decision.denied_reason;
        let latency_ms = started_at.elapsed().as_millis() as u64;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permissions = ?required_permissions,
            permissions_count = decision.permissions_count,
            cache_hit = decision.cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            allowed,
            latency_ms,
            "rbac resolver decision (any-permission check)"
        );

        if let Some((denied_reason_kind, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                denied_reason = %denied_reason,
                "rbac deny: none of required permissions granted"
            );
            Self::denied_reason_bucket(denied_reason_kind);
        }

        Self::record_permission_check_result(allowed);
        Self::record_permission_check_latency(latency_ms);
        if let Err(error) = Self::shadow_compare_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::Any(required_permissions),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac dual-read shadow compare failed"
            );
        }

        if let Err(error) = Self::shadow_compare_casbin_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::Any(required_permissions),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac casbin-shadow compare failed"
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
        let started_at = Instant::now();

        let resolver = Self::resolver(db);
        let decision =
            authorize_all_permissions(&resolver, tenant_id, user_id, required_permissions).await?;
        let allowed = decision.allowed;
        let denied = decision.denied_reason;
        let missing_permissions = decision.missing_permissions;
        let latency_ms = started_at.elapsed().as_millis() as u64;

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            required_permissions = ?required_permissions,
            permissions_count = decision.permissions_count,
            cache_hit = decision.cache_hit,
            denied_reason = denied.as_ref().map(|(_, reason)| reason.as_str()),
            missing_permissions = ?missing_permissions,
            allowed,
            latency_ms,
            "rbac resolver decision (all-permissions check)"
        );

        if let Some((denied_reason_kind, denied_reason)) = denied {
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                denied_reason = %denied_reason,
                missing_permissions = ?missing_permissions,
                "rbac deny: not all required permissions granted"
            );
            Self::denied_reason_bucket(denied_reason_kind);
        }

        Self::record_permission_check_result(allowed);
        Self::record_permission_check_latency(latency_ms);
        if let Err(error) = Self::shadow_compare_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::All(required_permissions),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac dual-read shadow compare failed"
            );
        }

        if let Err(error) = Self::shadow_compare_casbin_decision(
            db,
            tenant_id,
            user_id,
            ShadowCheck::All(required_permissions),
            allowed,
        )
        .await
        {
            Self::record_shadow_compare_failure();
            warn!(
                tenant_id = %tenant_id,
                user_id = %user_id,
                required_permissions = ?required_permissions,
                error = %error,
                "rbac casbin-shadow compare failed"
            );
        }

        Ok(allowed)
    }

    pub async fn get_user_permissions(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>> {
        let resolver = Self::resolver(db);
        let started_at = Instant::now();
        let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;

        if resolved.cache_hit {
            RBAC_PERMISSION_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        } else {
            RBAC_PERMISSION_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        }

        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            cache_hit = resolved.cache_hit,
            permissions_count = resolved.permissions.len(),
            latency_ms = started_at.elapsed().as_millis(),
            "rbac resolver permission lookup"
        );

        Self::record_permission_lookup_latency(started_at.elapsed().as_millis() as u64);

        Ok(resolved.permissions)
    }

    fn resolver(db: &DatabaseConnection) -> ServerRuntimePermissionResolver {
        RuntimePermissionResolver::new(
            SeaOrmRelationPermissionStore { db: db.clone() },
            MokaPermissionCache,
            ServerRoleAssignmentStore { db: db.clone() },
        )
    }

    pub async fn assign_role_permissions(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
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
        Self::replace_user_role_via_store(db, user_id, tenant_id, role).await
    }

    pub async fn remove_tenant_role_assignments(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
    ) -> Result<()> {
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
        let resolver = Self::resolver(db);
        resolver
            .remove_user_role_assignment(tenant_id, user_id, role)
            .await
    }

    async fn assign_role_permissions_via_store(
        db: &impl ConnectionTrait,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        let role_model = Self::get_or_create_role(db, tenant_id, &role).await?;

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
            let permission_model =
                Self::get_or_create_permission(db, tenant_id, permission).await?;

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

        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;

        Ok(())
    }

    async fn replace_user_role_via_store(
        db: &impl ConnectionTrait,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        Self::remove_tenant_role_assignments_via_store(db, user_id, tenant_id).await?;

        Self::assign_role_permissions_via_store(db, user_id, tenant_id, role).await
    }

    async fn remove_tenant_role_assignments_via_store(
        db: &impl ConnectionTrait,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
    ) -> Result<()> {
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

        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;

        Ok(())
    }

    async fn remove_user_role_assignment_via_store(
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

        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;

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
}

#[derive(Clone)]
struct SeaOrmRelationPermissionStore {
    db: DatabaseConnection,
}

#[derive(Clone)]
struct MokaPermissionCache;

#[derive(Clone)]
struct ServerRoleAssignmentStore {
    db: DatabaseConnection,
}

#[async_trait]
impl PermissionCache for MokaPermissionCache {
    async fn get(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) -> Option<Vec<Permission>> {
        USER_PERMISSION_CACHE.get(&(*tenant_id, *user_id)).await
    }

    async fn insert(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: Vec<Permission>,
    ) {
        USER_PERMISSION_CACHE
            .insert((*tenant_id, *user_id), permissions)
            .await;
    }

    async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        USER_PERMISSION_CACHE
            .invalidate(&(*tenant_id, *user_id))
            .await;
    }
}

#[async_trait]
impl RelationPermissionStore for SeaOrmRelationPermissionStore {
    type Error = Error;

    async fn load_user_role_ids(&self, user_id: &uuid::Uuid) -> Result<Vec<uuid::Uuid>> {
        let user_role_models = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .all(&self.db)
            .await?;

        Ok(user_role_models
            .into_iter()
            .map(|user_role| user_role.role_id)
            .collect())
    }

    async fn load_tenant_role_ids(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<uuid::Uuid>> {
        let tenant_role_models = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(*tenant_id))
            .filter(roles::Column::Id.is_in(role_ids.iter().copied()))
            .all(&self.db)
            .await?;

        Ok(tenant_role_models.into_iter().map(|role| role.id).collect())
    }

    async fn load_permissions_for_roles(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<Permission>> {
        let role_permission_models = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.is_in(role_ids.iter().copied()))
            .all(&self.db)
            .await?;

        if role_permission_models.is_empty() {
            return Ok(vec![]);
        }

        let permission_ids: Vec<uuid::Uuid> = role_permission_models
            .into_iter()
            .map(|role_permission| role_permission.permission_id)
            .collect();

        let permission_models = permissions::Entity::find()
            .filter(permissions::Column::TenantId.eq(*tenant_id))
            .filter(permissions::Column::Id.is_in(permission_ids))
            .all(&self.db)
            .await?;

        let mut result = Vec::with_capacity(permission_models.len());
        for permission in permission_models {
            let resource = permission
                .resource
                .parse::<Resource>()
                .map_err(Error::BadRequest)?;
            let action = permission
                .action
                .parse::<Action>()
                .map_err(Error::BadRequest)?;
            result.push(Permission::new(resource, action));
        }

        Ok(result)
    }
}

#[async_trait]
impl RoleAssignmentStore for ServerRoleAssignmentStore {
    type Error = Error;

    async fn assign_role_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        AuthService::assign_role_permissions_via_store(&self.db, user_id, tenant_id, role).await
    }

    async fn replace_user_role(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        AuthService::replace_user_role_via_store(&self.db, user_id, tenant_id, role).await
    }

    async fn remove_tenant_role_assignments(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<()> {
        AuthService::remove_tenant_role_assignments_via_store(&self.db, user_id, tenant_id).await
    }

    async fn remove_user_role_assignment(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        AuthService::remove_user_role_assignment_via_store(&self.db, user_id, tenant_id, role).await
    }
}

#[cfg(test)]
mod tests {
    use super::AuthService;
    use crate::models::_entities::{roles, user_roles};
    use crate::models::{tenants, users};
    use chrono::Utc;
    use migration::Migrator;
    use rustok_core::UserRole;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const AUTHZ_MODE_ENV: &str = "RUSTOK_RBAC_AUTHZ_MODE";
    const LEGACY_ROLE_FALLBACK_FLAG_ENV: &str = "RUSTOK_RBAC_LEGACY_ROLE_FALLBACK_ENABLED";

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

    struct ScopedEnvOverride {
        name: &'static str,
        previous: Option<String>,
    }

    impl ScopedEnvOverride {
        fn set(name: &'static str, value: &str) -> Self {
            let previous = std::env::var(name).ok();
            // SAFETY: tests serialize environment mutations via `EnvVarGuard` lock.
            unsafe {
                std::env::set_var(name, value);
            }
            Self { name, previous }
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

    impl Drop for ScopedEnvOverride {
        fn drop(&mut self) {
            self.restore();
        }
    }

    #[tokio::test]
    async fn assign_role_permissions_creates_user_roles_link() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = rustok_core::generate_id();
        let user_id = rustok_core::generate_id();

        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Test tenant".to_string()),
            slug: Set("test-tenant-assign-role".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            is_active: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert tenant");

        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_id),
            email: Set("assign-role@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            role: Set(UserRole::Customer),
            status: Set(rustok_core::UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert user");

        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
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
        let tenant_id = rustok_core::generate_id();
        let user_id = rustok_core::generate_id();

        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Test tenant".to_string()),
            slug: Set("test-tenant-replace-role".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            is_active: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert tenant");

        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_id),
            email: Set("replace-role@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            role: Set(UserRole::Customer),
            status: Set(rustok_core::UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert user");

        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .expect("initial role assignment should succeed");

        AuthService::replace_user_role(&db, &user_id, &tenant_id, UserRole::Admin)
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
        let tenant_id = rustok_core::generate_id();
        let user_id = rustok_core::generate_id();

        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Test tenant".to_string()),
            slug: Set("test-tenant-idempotent-role".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            is_active: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert tenant");

        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_id),
            email: Set("idempotent-role@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            role: Set(UserRole::Customer),
            status: Set(rustok_core::UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert user");

        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("first role assignment should succeed");
        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
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
        let tenant_id = rustok_core::generate_id();
        let user_id = rustok_core::generate_id();

        tenants::Entity::insert(tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Test tenant".to_string()),
            slug: Set("test-tenant-remove-all-roles".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            is_active: Set(true),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert tenant");

        users::Entity::insert(users::ActiveModel {
            id: Set(user_id),
            tenant_id: Set(tenant_id),
            email: Set("remove-all-roles@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            name: Set(None),
            role: Set(UserRole::Customer),
            status: Set(rustok_core::UserStatus::Active),
            email_verified_at: Set(None),
            last_login_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        })
        .exec(&db)
        .await
        .expect("failed to insert user");

        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Customer)
            .await
            .expect("customer role assignment should succeed");
        AuthService::assign_role_permissions(&db, &user_id, &tenant_id, UserRole::Manager)
            .await
            .expect("manager role assignment should succeed");

        AuthService::remove_tenant_role_assignments(&db, &user_id, &tenant_id)
            .await
            .expect("remove tenant role assignments should succeed");

        let remaining_links = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&db)
            .await
            .expect("failed to query remaining user_roles links");

        assert!(remaining_links.is_empty());
    }

    #[test]
    fn claim_role_mismatch_counter_increments() {
        let before = AuthService::metrics_snapshot().claim_role_mismatch_total;
        AuthService::record_claim_role_mismatch();
        let after = AuthService::metrics_snapshot().claim_role_mismatch_total;

        assert_eq!(after, before + 1);
    }

    #[test]
    fn authz_mode_defaults_to_relation_only_when_env_missing() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.remove();

        assert!(!AuthService::is_dual_read_enabled());
    }

    #[test]
    fn authz_mode_enables_dual_read_from_env() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("dual_read");

        assert!(AuthService::is_dual_read_enabled());
    }

    #[test]
    fn authz_mode_handles_trimmed_and_uppercase_env_value() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("  DUAL_READ  ");

        assert!(AuthService::is_dual_read_enabled());
    }

    #[test]
    fn authz_mode_enables_dual_read_from_legacy_fallback_alias_when_mode_missing() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.remove();

        let _fallback_alias = ScopedEnvOverride::set(LEGACY_ROLE_FALLBACK_FLAG_ENV, "true");

        assert!(AuthService::is_dual_read_enabled());
    }

    #[test]
    fn authz_mode_explicit_relation_only_overrides_legacy_fallback_alias() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("relation_only");

        let _fallback_alias = ScopedEnvOverride::set(LEGACY_ROLE_FALLBACK_FLAG_ENV, "true");

        assert!(!AuthService::is_dual_read_enabled());
    }

    #[test]
    fn authz_mode_enables_casbin_shadow_from_env() {
        let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
        env.set("casbin_shadow");

        assert!(AuthService::should_run_casbin_shadow());
    }

    #[test]
    fn authz_mode_guard_restores_previous_env_value() {
        let previous = {
            let env = EnvVarGuard::lock(AUTHZ_MODE_ENV);
            let previous = env.previous().map(ToOwned::to_owned);
            env.set("dual_read");
            assert!(AuthService::is_dual_read_enabled());
            previous
        };

        assert_eq!(std::env::var(AUTHZ_MODE_ENV).ok(), previous);
    }

    #[test]
    fn shadow_compare_failure_counter_increments() {
        let before = AuthService::metrics_snapshot().shadow_compare_failures_total;
        AuthService::record_shadow_compare_failure();
        let after = AuthService::metrics_snapshot().shadow_compare_failures_total;

        assert_eq!(after, before + 1);
    }
}
