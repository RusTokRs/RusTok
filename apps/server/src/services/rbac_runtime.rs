use crate::error::Error;
use crate::error::Result;
use async_trait::async_trait;
use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustok_core::UserRole;

use rustok_api::{Action, Permission, Resource};
use rustok_rbac::{
    AuthorizationDecision, DeniedReasonKind, PermissionCache, PermissionCacheLookup,
    RelationPermissionStore, RoleAssignmentStore, RuntimePermissionResolver,
    authorize_all_permissions, authorize_any_permission, authorize_permission,
    invalidate_cached_permissions,
};

use crate::models::_entities::{permissions, role_permissions, roles, user_roles, users};

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
    pub engine_decisions_policy_total: u64,
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
static RBAC_ENGINE_DECISIONS_POLICY_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBAC_ENGINE_EVAL_DURATION_SAMPLES: AtomicU64 = AtomicU64::new(0);
static RBAC_PERMISSION_CACHE_GLOBAL_EPOCH: AtomicU32 = AtomicU32::new(1);
static RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED: AtomicBool = AtomicBool::new(false);

const RBAC_PERMISSION_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const RBAC_PERMISSION_CACHE_LOOKUP_ATTEMPTS: usize = 4;
const RBAC_PERMISSION_CACHE_EPOCH_STRIPES: usize = 64;

type PermissionCacheKey = (uuid::Uuid, uuid::Uuid);

static RBAC_PERMISSION_CACHE_KEY_EPOCHS: Lazy<[AtomicU32; RBAC_PERMISSION_CACHE_EPOCH_STRIPES]> =
    Lazy::new(|| std::array::from_fn(|_| AtomicU32::new(1)));

#[derive(Clone)]
struct CachedPermissionSnapshot {
    token: u64,
    permissions: Vec<Permission>,
}

static USER_PERMISSION_CACHE: Lazy<Cache<PermissionCacheKey, CachedPermissionSnapshot>> =
    Lazy::new(|| {
        Cache::builder()
            .weigher(permission_cache_entry_weight)
            .max_capacity(RBAC_PERMISSION_CACHE_MAX_WEIGHT_BYTES)
            .time_to_live(Duration::from_secs(60))
            .build()
    });

fn permission_cache_entry_weight(
    _key: &PermissionCacheKey,
    snapshot: &CachedPermissionSnapshot,
) -> u32 {
    let weight = std::mem::size_of::<PermissionCacheKey>()
        .saturating_add(std::mem::size_of::<CachedPermissionSnapshot>())
        .saturating_add(
            snapshot
                .permissions
                .len()
                .saturating_mul(std::mem::size_of::<Permission>()),
        );
    weight.clamp(1, u32::MAX as usize) as u32
}

fn permission_cache_epoch_stripe(key: &PermissionCacheKey) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % RBAC_PERMISSION_CACHE_EPOCH_STRIPES
}

fn permission_cache_token(global_epoch: u32, key_epoch: u32) -> u64 {
    (u64::from(global_epoch) << 32) | u64::from(key_epoch)
}

fn current_permission_cache_token(key: &PermissionCacheKey) -> Option<u64> {
    if RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED.load(Ordering::Acquire) {
        return None;
    }

    let global_epoch = RBAC_PERMISSION_CACHE_GLOBAL_EPOCH.load(Ordering::Acquire);
    let key_epoch = RBAC_PERMISSION_CACHE_KEY_EPOCHS[permission_cache_epoch_stripe(key)]
        .load(Ordering::Acquire);
    if RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED.load(Ordering::Acquire) {
        return None;
    }
    Some(permission_cache_token(global_epoch, key_epoch))
}

fn advance_permission_cache_epoch(epoch: &AtomicU32) {
    let mut current = epoch.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(1) else {
            RBAC_PERMISSION_CACHE_EPOCH_EXHAUSTED.store(true, Ordering::Release);
            return;
        };
        match epoch.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn advance_permission_cache_key_epoch(key: &PermissionCacheKey) {
    advance_permission_cache_epoch(
        &RBAC_PERMISSION_CACHE_KEY_EPOCHS[permission_cache_epoch_stripe(key)],
    );
}

fn advance_permission_cache_global_epoch() {
    advance_permission_cache_epoch(&RBAC_PERMISSION_CACHE_GLOBAL_EPOCH);
}

pub(crate) async fn invalidate_user_permissions_cache(
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) {
    let cache = MokaPermissionCache;
    invalidate_cached_permissions(&cache, tenant_id, user_id).await;
}

pub(crate) async fn invalidate_all_user_permissions_cache() {
    advance_permission_cache_global_epoch();
    USER_PERMISSION_CACHE.invalidate_all();
    USER_PERMISSION_CACHE.run_pending_tasks().await;
}

pub(crate) async fn invalidate_user_rbac_caches(tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
    invalidate_user_permissions_cache(tenant_id, user_id).await;
}

pub(crate) async fn authorize_request(
    db: &DatabaseConnection,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    check: AuthorizationCheck<'_>,
) -> Result<AuthorizationRuntimeOutcome> {
    let started_at = Instant::now();
    let resolver = resolver(db);
    let decision = match check {
        AuthorizationCheck::Single(permission) => {
            authorize_permission(&resolver, tenant_id, user_id, permission).await?
        }
        AuthorizationCheck::Any(permissions) => {
            authorize_any_permission(&resolver, tenant_id, user_id, permissions).await?
        }
        AuthorizationCheck::All(permissions) => {
            authorize_all_permissions(&resolver, tenant_id, user_id, permissions).await?
        }
    };

    Ok(AuthorizationRuntimeOutcome {
        decision,
        latency_ms: started_at.elapsed().as_millis() as u64,
    })
}

pub(crate) fn observe_authorization_decision(decision: &AuthorizationDecision, latency_ms: u64) {
    record_permission_cache_result(decision.cache_hit);
    record_permission_check_result(decision.allowed);
    record_permission_check_latency(latency_ms);
    record_engine_decision();
    record_engine_eval_duration(latency_ms);

    if let Some((denied_reason_kind, _)) = decision.denied_reason.as_ref() {
        record_denied_reason_bucket(*denied_reason_kind);
    }
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

pub(crate) fn record_engine_decision() {
    RBAC_ENGINE_DECISIONS_POLICY_TOTAL.fetch_add(1, Ordering::Relaxed);
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
        engine_decisions_policy_total: RBAC_ENGINE_DECISIONS_POLICY_TOTAL.load(Ordering::Relaxed),
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
    RBAC_ENGINE_DECISIONS_POLICY_TOTAL.store(0, Ordering::Relaxed);
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
        self.lookup(tenant_id, user_id).await.into_parts().0
    }

    async fn insert(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: Vec<Permission>,
    ) {
        let key = (*tenant_id, *user_id);
        let Some(token) = current_permission_cache_token(&key) else {
            return;
        };
        let _ = self
            .insert_if_current(tenant_id, user_id, token, permissions)
            .await;
    }

    async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
        super::rbac_request_scope::invalidate_current_rbac_request_scope(tenant_id, user_id);
        let key = (*tenant_id, *user_id);
        advance_permission_cache_key_epoch(&key);
        USER_PERMISSION_CACHE.invalidate(&key).await;
    }

    async fn lookup(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) -> PermissionCacheLookup {
        let key = (*tenant_id, *user_id);
        for _ in 0..RBAC_PERMISSION_CACHE_LOOKUP_ATTEMPTS {
            let Some(token) = current_permission_cache_token(&key) else {
                return PermissionCacheLookup::new(None, 0);
            };
            let snapshot = USER_PERMISSION_CACHE.get(&key).await;
            if current_permission_cache_token(&key) != Some(token) {
                continue;
            }

            match snapshot {
                Some(snapshot) if snapshot.token == token => {
                    return PermissionCacheLookup::new(Some(snapshot.permissions), token);
                }
                Some(_) => {
                    USER_PERMISSION_CACHE.invalidate(&key).await;
                }
                None => {}
            }
            return PermissionCacheLookup::new(None, token);
        }

        PermissionCacheLookup::new(None, current_permission_cache_token(&key).unwrap_or(0))
    }

    async fn insert_if_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        token: u64,
        permissions: Vec<Permission>,
    ) -> bool {
        let key = (*tenant_id, *user_id);
        if current_permission_cache_token(&key) != Some(token) {
            return false;
        }

        USER_PERMISSION_CACHE
            .insert(key, CachedPermissionSnapshot { token, permissions })
            .await;
        if current_permission_cache_token(&key) != Some(token) {
            USER_PERMISSION_CACHE.invalidate(&key).await;
            return false;
        }
        true
    }
}

#[async_trait]
impl RelationPermissionStore for SeaOrmRelationPermissionStore {
    type Error = Error;

    async fn load_user_role_ids(&self, user_id: &uuid::Uuid) -> Result<Vec<uuid::Uuid>> {
        let Some(user) = users::Entity::find_by_id(*user_id).one(&self.db).await? else {
            return Ok(Vec::new());
        };

        let assigned_role_ids = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|user_role| user_role.role_id)
            .collect::<Vec<_>>();
        if assigned_role_ids.is_empty() {
            return Ok(Vec::new());
        }

        let user_tenant_roles = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(user.tenant_id))
            .filter(roles::Column::Id.is_in(assigned_role_ids))
            .all(&self.db)
            .await?;

        Ok(user_tenant_roles.into_iter().map(|role| role.id).collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{tenants, users as user_models};
    use chrono::Utc;
    use rustok_core::UserStatus;
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ConnectionTrait, Set};
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

        user_models::Entity::insert(user_models::ActiveModel {
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

    #[test]
    fn permission_cache_weight_grows_with_permission_count() {
        let key = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let one = permission_cache_entry_weight(
            &key,
            &CachedPermissionSnapshot {
                token: 1,
                permissions: vec![Permission::new(Resource::Users, Action::Read)],
            },
        );
        let many = permission_cache_entry_weight(
            &key,
            &CachedPermissionSnapshot {
                token: 1,
                permissions: vec![Permission::new(Resource::Users, Action::Read); 32],
            },
        );

        assert!(many > one);
        assert!(one as usize >= std::mem::size_of::<PermissionCacheKey>());
    }

    #[tokio::test]
    #[serial]
    async fn full_permission_cache_invalidation_removes_unknown_user_entries() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let cache = MokaPermissionCache;
        cache
            .insert(
                &tenant_id,
                &user_id,
                vec![Permission::new(Resource::Users, Action::Read)],
            )
            .await;
        assert!(cache.get(&tenant_id, &user_id).await.is_some());

        invalidate_all_user_permissions_cache().await;

        assert!(cache.get(&tenant_id, &user_id).await.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn targeted_invalidation_preserves_an_unrelated_epoch_stripe() {
        invalidate_all_user_permissions_cache().await;
        let tenant_id = uuid::Uuid::new_v4();
        let first_user_id = uuid::Uuid::new_v4();
        let first_key = (tenant_id, first_user_id);
        let first_stripe = permission_cache_epoch_stripe(&first_key);
        let second_user_id = loop {
            let candidate = uuid::Uuid::new_v4();
            if permission_cache_epoch_stripe(&(tenant_id, candidate)) != first_stripe {
                break candidate;
            }
        };
        let cache = MokaPermissionCache;
        cache
            .insert(&tenant_id, &first_user_id, vec![Permission::USERS_READ])
            .await;
        cache
            .insert(&tenant_id, &second_user_id, vec![Permission::USERS_LIST])
            .await;

        invalidate_user_permissions_cache(&tenant_id, &first_user_id).await;

        assert!(cache.get(&tenant_id, &first_user_id).await.is_none());
        assert_eq!(
            cache.get(&tenant_id, &second_user_id).await,
            Some(vec![Permission::USERS_LIST])
        );
    }

    #[tokio::test]
    #[serial]
    async fn stale_permission_fill_is_rejected_after_invalidation() {
        invalidate_all_user_permissions_cache().await;
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let cache = MokaPermissionCache;
        let (_, stale_token) = cache.lookup(&tenant_id, &user_id).await.into_parts();

        invalidate_user_permissions_cache(&tenant_id, &user_id).await;
        let published = cache
            .insert_if_current(
                &tenant_id,
                &user_id,
                stale_token,
                vec![Permission::SETTINGS_MANAGE],
            )
            .await;

        assert!(!published);
        assert!(cache.get(&tenant_id, &user_id).await.is_none());
    }

    #[tokio::test]
    async fn database_rejects_cross_tenant_role_links_and_loader_keeps_local_role() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let (tenant_a, user_a) = insert_tenant_and_user(
            &db,
            "runtime-role-filter-a",
            "runtime-role-filter-a@example.com",
        )
        .await;
        let (tenant_b, user_b) = insert_tenant_and_user(
            &db,
            "runtime-role-filter-b",
            "runtime-role-filter-b@example.com",
        )
        .await;

        assign_role_permissions_via_store(&db, &user_a, &tenant_a, UserRole::Customer)
            .await
            .expect("assign local role");
        assign_role_permissions_via_store(&db, &user_b, &tenant_b, UserRole::Admin)
            .await
            .expect("assign foreign role");

        let local_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_a))
            .filter(roles::Column::Slug.eq(UserRole::Customer.to_string()))
            .one(&db)
            .await
            .expect("load local role")
            .expect("local role exists");
        let foreign_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_b))
            .filter(roles::Column::Slug.eq(UserRole::Admin.to_string()))
            .one(&db)
            .await
            .expect("load foreign role")
            .expect("foreign role exists");

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

        let store = SeaOrmRelationPermissionStore { db };
        let role_ids = store
            .load_user_role_ids(&user_a)
            .await
            .expect("load user roles");

        assert_eq!(role_ids, vec![local_role.id]);
    }
}
