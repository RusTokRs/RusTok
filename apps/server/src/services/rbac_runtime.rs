use async_trait::async_trait;
use loco_rs::Error;
use loco_rs::Result;
use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::warn;

use rustok_core::{Action, Permission, Resource, UserRole};
use rustok_rbac::{
    authorize_all_permissions_for_mode, authorize_any_permission_for_mode,
    authorize_permission_for_mode, evaluate_shadow_runtime_for_mode, invalidate_cached_permissions,
    observe_shadow_runtime, shadow_runtime_runs_casbin, AuthorizationDecision,
    CasbinShadowMismatchRecord, DeniedReasonKind, PermissionCache, RbacAuthzMode,
    RelationPermissionStore, RoleAssignmentStore, RuntimePermissionResolver, ShadowCheck,
    ShadowRuntimeContext, ShadowRuntimeInput, ShadowRuntimeObserver,
};

use crate::models::_entities::{permissions, role_permissions, roles, user_roles};

use super::rbac_persistence::{
    assign_role_permissions_via_store, remove_tenant_role_assignments_via_store,
    remove_user_role_assignment_via_store, replace_user_role_via_store,
};

pub(crate) type ServerRuntimePermissionResolver = RuntimePermissionResolver<
    SeaOrmRelationPermissionStore,
    MokaPermissionCache,
    ServerRoleAssignmentStore,
    Error,
>;

#[derive(Clone, Copy)]
pub(crate) enum AuthorizationCheck<'a> {
    Single(&'a Permission),
    Any(&'a [Permission]),
    All(&'a [Permission]),
}

impl<'a> AuthorizationCheck<'a> {
    pub(crate) fn shadow_check(self) -> ShadowCheck<'a> {
        match self {
            Self::Single(permission) => ShadowCheck::Single(permission),
            Self::Any(permissions) => ShadowCheck::Any(permissions),
            Self::All(permissions) => ShadowCheck::All(permissions),
        }
    }
}

pub(crate) struct AuthorizationRuntimeOutcome {
    pub decision: AuthorizationDecision,
    pub latency_ms: u64,
}

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
static RBAC_SHADOW_COMPARE_FAILURES_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_DECISIONS_RELATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_DECISIONS_CASBIN_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_SAMPLES: AtomicU64 = AtomicU64::new(0);

static USER_PERMISSION_CACHE: Lazy<Cache<(uuid::Uuid, uuid::Uuid), Vec<Permission>>> =
    Lazy::new(|| {
        Cache::builder()
            .max_capacity(20_000)
            .time_to_live(Duration::from_secs(60))
            .build()
    });

pub(crate) struct ServerShadowRuntimeObserver;

impl ShadowRuntimeObserver for ServerShadowRuntimeObserver {
    fn record_engine_decision_deltas(&self, relation_delta: u64, casbin_delta: u64) {
        record_engine_decision_deltas(relation_delta, casbin_delta);
    }

    fn record_engine_mismatch_delta(&self, delta: u64) {
        record_engine_mismatch_delta(delta);
    }

    fn record_engine_eval_duration(&self, latency_ms: u64) {
        record_engine_eval_duration(latency_ms);
    }

    fn on_casbin_mismatch(
        &self,
        context: ShadowRuntimeContext<'_>,
        mismatch: &CasbinShadowMismatchRecord,
    ) {
        warn!(
            tenant_id = %context.tenant_id,
            user_id = %context.user_id,
            shadow_check = mismatch.shadow_check,
            checked_permissions_total = mismatch.checked_permissions_total,
            resource = %mismatch.permission.resource,
            action = %mismatch.permission.action,
            relation_decision = mismatch.relation_allowed,
            casbin_decision = mismatch.casbin_allowed,
            "rbac_engine_mismatch"
        );
    }
}

pub(crate) async fn invalidate_user_permissions_cache(
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) {
    let cache = MokaPermissionCache;
    invalidate_cached_permissions(&cache, tenant_id, user_id).await;
}

pub(crate) async fn invalidate_user_rbac_caches(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
    invalidate_user_permissions_cache(tenant_id, user_id).await;
}

pub(crate) fn authz_mode() -> RbacAuthzMode {
    RbacAuthzMode::from_env()
}

pub(crate) async fn compare_shadow_runtime(
    _db: &DatabaseConnection,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    resolved_permissions: &[Permission],
    shadow_check: ShadowCheck<'_>,
    relation_allowed: bool,
) -> Result<()> {
    let authz_mode = authz_mode();
    let casbin_started_at = shadow_runtime_runs_casbin(authz_mode).then(std::time::Instant::now);
    let evaluation = evaluate_shadow_runtime_for_mode(ShadowRuntimeInput {
        authz_mode,
        tenant_id,
        resolved_permissions,
        shadow_check,
        relation_allowed,
    });
    let observer = ServerShadowRuntimeObserver;
    observe_shadow_runtime(
        &observer,
        ShadowRuntimeContext { tenant_id, user_id },
        &evaluation,
        casbin_started_at.map(|started_at| started_at.elapsed().as_millis() as u64),
    );

    Ok(())
}

pub(crate) async fn authorize_request(
    db: &DatabaseConnection,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    check: AuthorizationCheck<'_>,
) -> Result<AuthorizationRuntimeOutcome> {
    let started_at = Instant::now();
    let authz_mode = authz_mode();
    let resolver = resolver(db);
    let decision = match check {
        AuthorizationCheck::Single(permission) => {
            authorize_permission_for_mode(&resolver, authz_mode, tenant_id, user_id, permission)
                .await?
        }
        AuthorizationCheck::Any(permissions) => {
            authorize_any_permission_for_mode(
                &resolver,
                authz_mode,
                tenant_id,
                user_id,
                permissions,
            )
            .await?
        }
        AuthorizationCheck::All(permissions) => {
            authorize_all_permissions_for_mode(
                &resolver,
                authz_mode,
                tenant_id,
                user_id,
                permissions,
            )
            .await?
        }
    };

    Ok(AuthorizationRuntimeOutcome {
        decision,
        latency_ms: started_at.elapsed().as_millis() as u64,
    })
}

pub(crate) async fn observe_authorization_decision(
    db: &DatabaseConnection,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    decision: &AuthorizationDecision,
    shadow_check: ShadowCheck<'_>,
    latency_ms: u64,
) -> Result<()> {
    record_permission_cache_result(decision.cache_hit);
    record_permission_check_result(decision.allowed);
    record_permission_check_latency(latency_ms);

    if let Some((denied_reason_kind, _)) = decision.denied_reason.as_ref() {
        record_denied_reason_bucket(*denied_reason_kind);
    }

    compare_shadow_runtime(
        db,
        tenant_id,
        user_id,
        &decision.resolved_permissions,
        shadow_check,
        decision.allowed,
    )
    .await
    .map_err(|error| {
        record_shadow_compare_failure();
        error
    })
}

pub(crate) fn resolver(db: &DatabaseConnection) -> ServerRuntimePermissionResolver {
    RuntimePermissionResolver::new(
        SeaOrmRelationPermissionStore { db: db.clone() },
        MokaPermissionCache,
        ServerRoleAssignmentStore { db: db.clone() },
    )
}

pub(crate) fn record_permission_cache_result(cache_hit: bool) {
    if cache_hit {
        RBAC_PERMISSION_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    } else {
        RBAC_PERMISSION_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn record_permission_check_result(allowed: bool) {
    if allowed {
        RBAC_PERMISSION_CHECKS_ALLOWED.fetch_add(1, Ordering::Relaxed);
    } else {
        RBAC_PERMISSION_CHECKS_DENIED.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn record_permission_check_latency(latency_ms: u64) {
    RBAC_PERMISSION_CHECK_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
    RBAC_PERMISSION_CHECK_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_permission_lookup_latency(latency_ms: u64) {
    RBAC_PERMISSION_LOOKUP_LATENCY_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
    RBAC_PERMISSION_LOOKUP_LATENCY_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_denied_reason_bucket(denied_reason_kind: DeniedReasonKind) {
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

pub(crate) fn record_claim_role_mismatch() {
    RBAC_CLAIM_ROLE_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_shadow_compare_failure() {
    RBAC_SHADOW_COMPARE_FAILURES_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_engine_decision_deltas(relation_delta: u64, casbin_delta: u64) {
    if relation_delta > 0 {
        RBAC_ENGINE_DECISIONS_RELATION_TOTAL.fetch_add(relation_delta, Ordering::Relaxed);
    }
    if casbin_delta > 0 {
        RBAC_ENGINE_DECISIONS_CASBIN_TOTAL.fetch_add(casbin_delta, Ordering::Relaxed);
    }
}

pub(crate) fn record_engine_mismatch_delta(delta: u64) {
    if delta > 0 {
        RBAC_ENGINE_MISMATCH_TOTAL.fetch_add(delta, Ordering::Relaxed);
    }
}

pub(crate) fn record_engine_eval_duration(latency_ms: u64) {
    RBAC_ENGINE_EVAL_DURATION_MS_TOTAL.fetch_add(latency_ms, Ordering::Relaxed);
    RBAC_ENGINE_EVAL_DURATION_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn metrics_snapshot() -> RbacResolverMetricsSnapshot {
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
        denied_no_permissions_resolved: RBAC_DENIED_NO_PERMISSIONS_RESOLVED.load(Ordering::Relaxed),
        denied_missing_permissions: RBAC_DENIED_MISSING_PERMISSIONS.load(Ordering::Relaxed),
        denied_unknown: RBAC_DENIED_UNKNOWN.load(Ordering::Relaxed),
        claim_role_mismatch_total: RBAC_CLAIM_ROLE_MISMATCH_TOTAL.load(Ordering::Relaxed),
        shadow_compare_failures_total: RBAC_SHADOW_COMPARE_FAILURES_TOTAL.load(Ordering::Relaxed),
        engine_decisions_relation_total: RBAC_ENGINE_DECISIONS_RELATION_TOTAL
            .load(Ordering::Relaxed),
        engine_decisions_casbin_total: RBAC_ENGINE_DECISIONS_CASBIN_TOTAL.load(Ordering::Relaxed),
        engine_mismatch_total: RBAC_ENGINE_MISMATCH_TOTAL.load(Ordering::Relaxed),
        engine_eval_duration_ms_total: RBAC_ENGINE_EVAL_DURATION_MS_TOTAL.load(Ordering::Relaxed),
        engine_eval_duration_samples: RBAC_ENGINE_EVAL_DURATION_SAMPLES.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
pub(crate) fn reset_metrics_for_tests() {
    RBAC_PERMISSION_CACHE_HITS.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_CACHE_MISSES.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_CHECKS_ALLOWED.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_CHECKS_DENIED.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_CHECK_LATENCY_MS_TOTAL.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_CHECK_LATENCY_SAMPLES.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_LOOKUP_LATENCY_MS_TOTAL.store(0, Ordering::Relaxed);
    RBAC_PERMISSION_LOOKUP_LATENCY_SAMPLES.store(0, Ordering::Relaxed);
    RBAC_DENIED_NO_PERMISSIONS_RESOLVED.store(0, Ordering::Relaxed);
    RBAC_DENIED_MISSING_PERMISSIONS.store(0, Ordering::Relaxed);
    RBAC_DENIED_UNKNOWN.store(0, Ordering::Relaxed);
    RBAC_CLAIM_ROLE_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    RBAC_SHADOW_COMPARE_FAILURES_TOTAL.store(0, Ordering::Relaxed);
    RBAC_ENGINE_DECISIONS_RELATION_TOTAL.store(0, Ordering::Relaxed);
    RBAC_ENGINE_DECISIONS_CASBIN_TOTAL.store(0, Ordering::Relaxed);
    RBAC_ENGINE_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    RBAC_ENGINE_EVAL_DURATION_MS_TOTAL.store(0, Ordering::Relaxed);
    RBAC_ENGINE_EVAL_DURATION_SAMPLES.store(0, Ordering::Relaxed);
}

#[derive(Clone)]
pub(crate) struct SeaOrmRelationPermissionStore {
    db: DatabaseConnection,
}

#[derive(Clone)]
pub(crate) struct MokaPermissionCache;

#[derive(Clone)]
pub(crate) struct ServerRoleAssignmentStore {
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
        assign_role_permissions_via_store(&self.db, user_id, tenant_id, role).await
    }

    async fn replace_user_role(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        replace_user_role_via_store(&self.db, user_id, tenant_id, role).await
    }

    async fn remove_tenant_role_assignments(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<()> {
        remove_tenant_role_assignments_via_store(&self.db, user_id, tenant_id).await
    }

    async fn remove_user_role_assignment(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<()> {
        remove_user_role_assignment_via_store(&self.db, user_id, tenant_id, role).await
    }
}
