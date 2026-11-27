use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use rustok_api::{AuthPrincipalKind, Permission};
use rustok_core::UserRole;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRequestScope {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub permissions: Vec<Permission>,
    pub role: UserRole,
    credential: Option<RequestCredentialAuthority>,
}

/// Facts admitted by the common JWT extractor, never transport input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestCredentialAuthority {
    pub(crate) principal_kind: AuthPrincipalKind,
    pub(crate) session_id: Uuid,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) recovery_revision: i64,
    pub(crate) delegated: Option<DelegatedCredentialEvidence>,
}

/// Identity of the exact durable access registration admitted by the JWT
/// extractor. Never accept these facts from a command payload or rederive
/// them from a later consent grant.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DelegatedCredentialEvidence {
    pub(crate) registration_id: Uuid,
    pub(crate) app_id: Uuid,
    pub(crate) client_id: Uuid,
    pub(crate) token_hash: String,
    pub(crate) issued_at: DateTime<Utc>,
    pub(crate) consent_granted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
    pub(crate) scopes: Vec<String>,
}

impl std::fmt::Debug for DelegatedCredentialEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DelegatedCredentialEvidence")
            .field("registration_id", &self.registration_id)
            .field("app_id", &self.app_id)
            .field("client_id", &self.client_id)
            .field("issued_at", &self.issued_at)
            .field("consent_granted_at", &self.consent_granted_at)
            .finish_non_exhaustive()
    }
}

impl DelegatedCredentialEvidence {
    pub(crate) fn from_registration(
        row: &crate::models::oauth_tokens::Model,
        client_id: Uuid,
        scopes: &[String],
    ) -> Self {
        Self {
            registration_id: row.id,
            app_id: row.app_id,
            client_id,
            token_hash: row.token_hash.clone(),
            issued_at: row.created_at.with_timezone(&Utc),
            consent_granted_at: row.consent_granted_at,
            scopes: scopes.to_vec(),
        }
    }
}

impl RbacRequestScope {
    pub fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        mut permissions: Vec<Permission>,
        role: UserRole,
    ) -> Self {
        permissions.sort_by_cached_key(|permission| permission.to_string());
        permissions.dedup();
        Self {
            tenant_id,
            actor_id,
            permissions,
            role,
            credential: None,
        }
    }

    /// Authenticated HTTP/WS scopes carry the original credential ceiling.
    /// The grant-only constructor remains for trusted internal scopes and fixtures.
    pub(crate) fn authenticated(
        current: &crate::extractors::auth::CurrentUser,
        deadline: DateTime<Utc>,
    ) -> Self {
        let mut scope = Self::new(
            current.user.tenant_id,
            current.user.id,
            current.permissions.clone(),
            current.inferred_role.clone(),
        );
        scope.credential = Some(RequestCredentialAuthority {
            principal_kind: current.principal_kind,
            session_id: current.session_id,
            expires_at: deadline.min(current.access_token_expires_at),
            recovery_revision: current.user.recovery_revision,
            delegated: current.delegated_credential.clone(),
        });
        scope
    }

    fn matches(&self, tenant_id: &Uuid, actor_id: &Uuid) -> bool {
        self.tenant_id == *tenant_id && self.actor_id == *actor_id
    }
}

struct RbacRequestScopeState {
    scope: RbacRequestScope,
    valid: AtomicBool,
}

impl RbacRequestScopeState {
    fn new(scope: RbacRequestScope) -> Self {
        Self {
            scope,
            valid: AtomicBool::new(true),
        }
    }

    fn matches_valid(&self, tenant_id: &Uuid, actor_id: &Uuid) -> bool {
        self.valid.load(Ordering::Acquire) && self.scope.matches(tenant_id, actor_id)
    }
}

tokio::task_local! {
    static CURRENT_RBAC_SCOPE: RbacRequestScopeState;
}

pub async fn with_rbac_request_scope<F>(scope: Option<RbacRequestScope>, future: F) -> F::Output
where
    F: Future,
{
    match scope {
        Some(scope) => {
            CURRENT_RBAC_SCOPE
                .scope(RbacRequestScopeState::new(scope), future)
                .await
        }
        None => future.await,
    }
}

pub fn permissions_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<Vec<Permission>> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state
                .matches_valid(tenant_id, actor_id)
                .then(|| state.scope.permissions.clone())
        })
        .ok()
        .flatten()
}

/// Distinguish an explicitly grant-only internal scope from a missing,
/// mismatched or invalidated scope. Never fall back to internal authority
/// when an authenticated request has lost its snapshot.
pub(crate) fn credential_for(
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> Option<Option<RequestCredentialAuthority>> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state
                .matches_valid(tenant_id, actor_id)
                .then(|| state.scope.credential.clone())
        })
        .ok()
        .flatten()
}

pub fn role_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<UserRole> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state
                .matches_valid(tenant_id, actor_id)
                .then(|| state.scope.role.clone())
        })
        .ok()
        .flatten()
}

/// Reject unscoped fallback for an invalidated matching request snapshot.
///
/// Absence of a scope, or a scope for a different tenant/actor, retains the
/// existing persisted-grant behavior. This is task-local and is not a fence for
/// in-flight checks, other tasks, or other replicas.
pub(crate) fn ensure_scope_not_invalidated(
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> crate::error::Result<()> {
    let invalidated = CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state.scope.matches(tenant_id, actor_id) && !state.valid.load(Ordering::Acquire)
        })
        .unwrap_or(false);
    if invalidated {
        return Err(crate::error::Error::Cache(
            "request RBAC snapshot was invalidated".to_string(),
        ));
    }
    Ok(())
}

/// Invalidate the current task-local RBAC snapshot when the mutation targets its exact actor.
///
/// Later scope-aware checks in this task fail rather than reuse the snapshot or fall back to
/// unscoped grants. Other concurrent tasks retain their own task-local state; this is not a
/// request-wide or transaction-spanning revocation fence.
pub(crate) fn invalidate_current_rbac_request_scope(tenant_id: &Uuid, actor_id: &Uuid) {
    if let Err(error) = CURRENT_RBAC_SCOPE.try_with(|state| {
        if state.scope.matches(tenant_id, actor_id) {
            state.valid.store(false, Ordering::Release);
        }
    }) {
        tracing::trace!(%error, "Current task has no active task-local RBAC scope to invalidate");
    }
}

/// Invalidate the snapshot in this task for a full-cache invalidation.
pub(crate) fn invalidate_current_rbac_request_scope_for_all() {
    if let Err(error) = CURRENT_RBAC_SCOPE.try_with(|state| state.valid.store(false, Ordering::Release)) {
        tracing::trace!(%error, "Current task has no active task-local RBAC scope to invalidate");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_scope_is_bound_to_exact_tenant_and_actor() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );

        with_rbac_request_scope(Some(scope), async move {
            assert_eq!(
                permissions_for(&tenant_id, &actor_id),
                Some(vec![Permission::USERS_READ])
            );
            assert_eq!(role_for(&tenant_id, &actor_id), Some(UserRole::Admin));
            assert!(permissions_for(&tenant_id, &Uuid::new_v4()).is_none());
            assert!(permissions_for(&Uuid::new_v4(), &actor_id).is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn request_scope_does_not_leak_after_future_completes() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::USERS_LIST],
                UserRole::Manager,
            )),
            async {
                assert!(permissions_for(&tenant_id, &actor_id).is_some());
            },
        )
        .await;

        assert!(permissions_for(&tenant_id, &actor_id).is_none());
        assert!(ensure_scope_not_invalidated(&tenant_id, &actor_id).is_ok());
    }

    #[tokio::test]
    async fn exact_actor_invalidation_expires_the_request_snapshot() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_MANAGE],
            UserRole::Admin,
        );

        with_rbac_request_scope(Some(scope), async move {
            assert!(permissions_for(&tenant_id, &actor_id).is_some());
            invalidate_current_rbac_request_scope(&tenant_id, &actor_id);
            assert!(permissions_for(&tenant_id, &actor_id).is_none());
            assert!(role_for(&tenant_id, &actor_id).is_none());
            assert!(matches!(
                ensure_scope_not_invalidated(&tenant_id, &actor_id),
                Err(crate::error::Error::Cache(_))
            ));
            assert!(ensure_scope_not_invalidated(&tenant_id, &Uuid::new_v4()).is_ok());
            assert!(ensure_scope_not_invalidated(&Uuid::new_v4(), &actor_id).is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn unrelated_actor_invalidation_preserves_the_request_snapshot() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Manager,
        );

        with_rbac_request_scope(Some(scope), async move {
            invalidate_current_rbac_request_scope(&tenant_id, &Uuid::new_v4());
            invalidate_current_rbac_request_scope(&Uuid::new_v4(), &actor_id);
            assert!(ensure_scope_not_invalidated(&tenant_id, &actor_id).is_ok());
            assert_eq!(
                permissions_for(&tenant_id, &actor_id),
                Some(vec![Permission::USERS_READ])
            );
        })
        .await;
    }

    #[tokio::test]
    async fn full_invalidation_expires_only_the_current_task_snapshot() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );
        with_rbac_request_scope(Some(scope), async {
            invalidate_current_rbac_request_scope_for_all();
            assert!(permissions_for(&tenant_id, &actor_id).is_none());
            assert!(matches!(
                ensure_scope_not_invalidated(&tenant_id, &actor_id),
                Err(crate::error::Error::Cache(_))
            ));
        })
        .await;
        assert!(ensure_scope_not_invalidated(&tenant_id, &actor_id).is_ok());
    }

    #[tokio::test]
    async fn credential_evidence_is_bound_to_exact_valid_scope() {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let mut scope = RbacRequestScope::new(
            tenant, actor, vec![Permission::USERS_MANAGE], UserRole::Admin,
        );
        let credential = RequestCredentialAuthority {
            principal_kind: AuthPrincipalKind::DirectUser,
            session_id: Uuid::new_v4(),
            expires_at: Utc::now(),
            recovery_revision: 7,
            delegated: None,
        };
        scope.credential = Some(credential.clone());
        with_rbac_request_scope(Some(scope), async {
            assert_eq!(credential_for(&tenant, &actor), Some(Some(credential)));
            assert_eq!(credential_for(&tenant, &Uuid::new_v4()), None);
            assert_eq!(credential_for(&Uuid::new_v4(), &actor), None);
            invalidate_current_rbac_request_scope(&tenant, &actor);
            assert_eq!(credential_for(&tenant, &actor), None);
        }).await;
        assert_eq!(credential_for(&tenant, &actor), None);
    }

    #[test]
    fn snapshots_canonicalize_permission_order() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let first = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_LIST, Permission::USERS_READ],
            UserRole::Admin,
        );
        let second = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ, Permission::USERS_LIST],
            UserRole::Admin,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn snapshots_detect_permission_or_role_drift() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let baseline = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );
        let changed_permissions = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_LIST],
            UserRole::Admin,
        );
        let changed_role = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Manager,
        );

        assert_ne!(baseline, changed_permissions);
        assert_ne!(baseline, changed_role);
    }
}
