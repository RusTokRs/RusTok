use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use rustok_api::{AuthPrincipalKind, Permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CredentialAuthority,
    OAuthAdminCredentialEvidence,
};
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
    pub(crate) oauth: Option<OAuthCredentialEvidence>,
}

/// Compatibility name for the single owner DTO; construction stays in the
/// host JWT extractor, not in module transports.
pub(crate) type OAuthCredentialEvidence = OAuthAdminCredentialEvidence;

pub(crate) fn oauth_credential_from_registration(
    row: &crate::models::oauth_tokens::Model,
    client_id: Uuid,
    scopes: &[String],
) -> OAuthCredentialEvidence {
    OAuthCredentialEvidence {
        registration_id: row.id,
        app_id: row.app_id,
        client_id,
        token_hash: row.token_hash.clone(),
        issued_at: row.created_at.with_timezone(&Utc),
        consent_granted_at: row.consent_granted_at,
        scopes: scopes.to_vec(),
    }
}

impl RequestCredentialAuthority {
    pub(crate) fn typed(&self) -> Option<CredentialAuthority> {
        match self.principal_kind {
            AuthPrincipalKind::DirectUser if !self.session_id.is_nil() && self.oauth.is_none() => {
                Some(CredentialAuthority::DirectSession {
                    session_id: self.session_id,
                    expires_at: self.expires_at,
                    recovery_revision: self.recovery_revision,
                })
            }
            AuthPrincipalKind::DelegatedUser => Some(CredentialAuthority::DelegatedOAuth {
                registration: self.oauth.clone()?,
                expires_at: self.expires_at,
                recovery_revision: self.recovery_revision,
            }),
            AuthPrincipalKind::Service => Some(CredentialAuthority::ServiceOAuth {
                registration: self.oauth.clone()?,
                expires_at: self.expires_at,
            }),
            _ => None,
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
            oauth: current.oauth_credential.clone(),
        });
        scope
    }

    pub(crate) fn admin_credential(&self) -> Option<CredentialAuthority> {
        self.credential.as_ref()?.typed()
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
    static CURRENT_RBAC_SCOPE: Option<RbacRequestScopeState>;
}

pub async fn with_rbac_request_scope<F>(scope: Option<RbacRequestScope>, future: F) -> F::Output
where
    F: Future,
{
    // An explicit unauthenticated nested boundary must mask any outer
    // authenticated request. Never inherit its credential by accident.
    CURRENT_RBAC_SCOPE
        .scope(scope.map(RbacRequestScopeState::new), future)
        .await
}

pub fn permissions_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<Vec<Permission>> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            let state = state.as_ref()?;
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
            let state = state.as_ref()?;
            state
                .matches_valid(tenant_id, actor_id)
                .then(|| state.scope.credential.clone())
        })
        .ok()
        .flatten()
}

/// Explicit command evidence must match the currently valid admitted scope.
/// The grant-only constructor is not a production administrative capability.
pub(crate) fn credential_for_admin(
    context: &AuthAdminMutationContext,
) -> Result<Option<RequestCredentialAuthority>, AuthAdminMutationError> {
    let scoped = credential_for(&context.tenant_id, &context.actor_id)
        .ok_or(AuthAdminMutationError::Unauthorized)?;
    match scoped {
        Some(evidence)
            if evidence.typed().as_ref() == Some(&context.credential_authority)
                && context.principal_kind == Some(evidence.principal_kind) =>
        {
            Ok(Some(evidence))
        }
        #[cfg(test)]
        None if context.credential_authority == CredentialAuthority::TrustedInternal => Ok(None),
        _ => Err(AuthAdminMutationError::Unauthorized),
    }
}

pub(crate) fn bind_admin_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    principal_kind: AuthPrincipalKind,
    locale: Option<String>,
) -> Result<AuthAdminMutationContext, AuthAdminMutationError> {
    let evidence = credential_for(&tenant_id, &actor_id)
        .flatten()
        .filter(|e| e.principal_kind == principal_kind)
        .ok_or(AuthAdminMutationError::Unauthorized)?;
    Ok(AuthAdminMutationContext {
        tenant_id,
        actor_id,
        request_id: None,
        locale,
        principal_kind: Some(principal_kind),
        credential_authority: evidence
            .typed()
            .ok_or(AuthAdminMutationError::Unauthorized)?,
    })
}

pub fn role_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<UserRole> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            let state = state.as_ref()?;
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
            state.as_ref().is_some_and(|state| {
                state.scope.matches(tenant_id, actor_id) && !state.valid.load(Ordering::Acquire)
            })
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
        if let Some(state) = state
            .as_ref()
            .filter(|state| state.scope.matches(tenant_id, actor_id))
        {
            state.valid.store(false, Ordering::Release);
        }
    }) {
        tracing::trace!(%error, "Current task has no active task-local RBAC scope to invalidate");
    }
}

/// Invalidate the snapshot in this task for a full-cache invalidation.
pub(crate) fn invalidate_current_rbac_request_scope_for_all() {
    if let Err(error) = CURRENT_RBAC_SCOPE.try_with(|state| {
        if let Some(state) = state {
            state.valid.store(false, Ordering::Release);
        }
    }) {
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
            tenant,
            actor,
            vec![Permission::USERS_MANAGE],
            UserRole::Admin,
        );
        let credential = RequestCredentialAuthority {
            principal_kind: AuthPrincipalKind::DirectUser,
            session_id: Uuid::new_v4(),
            expires_at: Utc::now(),
            recovery_revision: 7,
            oauth: None,
        };
        scope.credential = Some(credential.clone());
        with_rbac_request_scope(Some(scope), async {
            assert_eq!(credential_for(&tenant, &actor), Some(Some(credential)));
            assert_eq!(credential_for(&tenant, &Uuid::new_v4()), None);
            assert_eq!(credential_for(&Uuid::new_v4(), &actor), None);
            invalidate_current_rbac_request_scope(&tenant, &actor);
            assert_eq!(credential_for(&tenant, &actor), None);
        })
        .await;
        assert_eq!(credential_for(&tenant, &actor), None);
    }

    #[tokio::test]
    async fn explicit_admin_credential_rejects_substitution_grant_only_and_nested_absence() {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let mut scope = RbacRequestScope::new(
            tenant,
            actor,
            vec![Permission::USERS_MANAGE],
            UserRole::Admin,
        );
        scope.credential = Some(RequestCredentialAuthority {
            principal_kind: AuthPrincipalKind::DirectUser,
            session_id: Uuid::new_v4(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            recovery_revision: 1,
            oauth: None,
        });
        with_rbac_request_scope(Some(scope), async {
            let context = bind_admin_context(
                tenant,
                actor,
                AuthPrincipalKind::DirectUser,
                Some("en".into()),
            )
            .unwrap();
            assert!(credential_for_admin(&context).unwrap().is_some());
            let mut wrong = context.clone();
            if let CredentialAuthority::DirectSession { session_id, .. } =
                &mut wrong.credential_authority
            {
                *session_id = Uuid::new_v4();
            }
            assert!(matches!(
                credential_for_admin(&wrong),
                Err(AuthAdminMutationError::Unauthorized)
            ));
            wrong = context.clone();
            wrong.principal_kind = Some(AuthPrincipalKind::Service);
            assert!(credential_for_admin(&wrong).is_err());
            wrong = context.clone();
            wrong.credential_authority = CredentialAuthority::TrustedInternal;
            assert!(credential_for_admin(&wrong).is_err());
            with_rbac_request_scope(None, async {
                assert!(permissions_for(&tenant, &actor).is_none());
                assert!(credential_for_admin(&context).is_err());
            })
            .await;
            let grant_only = RbacRequestScope::new(
                tenant,
                actor,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            );
            with_rbac_request_scope(Some(grant_only), async {
                assert!(credential_for_admin(&context).is_err());
                assert!(
                    bind_admin_context(tenant, actor, AuthPrincipalKind::DirectUser, None).is_err()
                );
            })
            .await;
            assert!(credential_for_admin(&context).is_ok());
            invalidate_current_rbac_request_scope_for_all();
            assert!(credential_for_admin(&context).is_err());
        })
        .await;
    }

    #[tokio::test]
    async fn artifact_runtime_rejects_non_direct_and_unbound_credentials() {
        use crate::services::artifact_binding::require_artifact_direct_credential;
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let make_scope = || {
            RbacRequestScope::new(
                tenant,
                actor,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )
        };
        assert!(require_artifact_direct_credential(tenant, actor).is_err());
        with_rbac_request_scope(Some(make_scope()), async {
            assert!(require_artifact_direct_credential(tenant, actor).is_err());
        })
        .await;
        for kind in [
            AuthPrincipalKind::DirectUser,
            AuthPrincipalKind::DelegatedUser,
            AuthPrincipalKind::Service,
        ] {
            let mut scope = make_scope();
            let oauth = (kind != AuthPrincipalKind::DirectUser).then(|| OAuthCredentialEvidence {
                registration_id: Uuid::new_v4(),
                app_id: Uuid::new_v4(),
                client_id: Uuid::new_v4(),
                token_hash: "digest".into(),
                issued_at: Utc::now(),
                consent_granted_at: None,
                scopes: vec!["modules:manage".into()],
            });
            scope.credential = Some(RequestCredentialAuthority {
                principal_kind: kind,
                session_id: if oauth.is_some() {
                    Uuid::nil()
                } else {
                    Uuid::new_v4()
                },
                expires_at: Utc::now() + chrono::Duration::minutes(5),
                recovery_revision: 1,
                oauth,
            });
            with_rbac_request_scope(Some(scope), async {
                assert_eq!(
                    require_artifact_direct_credential(tenant, actor).is_ok(),
                    kind == AuthPrincipalKind::DirectUser
                );
                assert!(require_artifact_direct_credential(tenant, Uuid::new_v4()).is_err());
                invalidate_current_rbac_request_scope(&tenant, &actor);
                assert!(require_artifact_direct_credential(tenant, actor).is_err());
            })
            .await;
        }
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
