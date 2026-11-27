use std::future::Future;
use std::sync::Arc;
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
    admitted_epoch: Option<u64>,
    static_revision: Option<Arc<rustok_rbac::StaticActorAuthorityRevision>>,
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
            admitted_epoch: None,
            static_revision: None,
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
        scope.admitted_epoch = Some(current.rbac_epoch);
        scope.static_revision = current.static_authority_revision.clone();
        scope.credential = Some(RequestCredentialAuthority {
            principal_kind: current.principal_kind,
            session_id: current.session_id,
            expires_at: deadline.min(current.access_token_expires_at),
            recovery_revision: current.user.recovery_revision,
            oauth: current.oauth_credential.clone(),
        });
        scope
    }

    pub(crate) fn epoch_is_current(&self) -> bool {
        self.admitted_epoch.is_none_or(|epoch| {
            super::rbac_runtime::request_authority_epoch(self.tenant_id, self.actor_id)
                == Some(epoch)
        })
    }

    pub(crate) fn admin_credential(&self) -> Option<CredentialAuthority> {
        if !self.epoch_is_current() {
            return None;
        }
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

    fn is_valid(&self) -> bool {
        if !self.valid.load(Ordering::Acquire) {
            return false;
        }
        if let Some(credential) = &self.scope.credential {
            // Expiry and clock failure are terminal for this snapshot,
            // including all captured clones.
            if !rustok_auth::application_clock::local_now()
                .is_ok_and(|now| now < credential.expires_at)
            {
                self.valid.store(false, Ordering::Release);
                return false;
            }
        }
        // Check after sampling time, so invalidation during that check is
        // observed before returning the original authority.
        if !self.scope.epoch_is_current() {
            self.valid.store(false, Ordering::Release);
            return false;
        }
        self.valid.load(Ordering::Acquire)
    }

    fn matches_valid(&self, tenant_id: &Uuid, actor_id: &Uuid) -> bool {
        self.scope.matches(tenant_id, actor_id) && self.is_valid()
    }
}

tokio::task_local! {
    static CURRENT_RBAC_SCOPE: Option<Arc<RbacRequestScopeState>>;
}

pub async fn with_rbac_request_scope<F>(scope: Option<RbacRequestScope>, future: F) -> F::Output
where
    F: Future,
{
    // An explicit unauthenticated nested boundary must mask any outer
    // authenticated request. Never inherit its credential by accident.
    CURRENT_RBAC_SCOPE
        .scope(
            scope.map(|scope| Arc::new(RbacRequestScopeState::new(scope))),
            future,
        )
        .await
}

/// Host-only capture of the original request authority for isolated sandbox
/// callbacks. Clones share the invalidation flag; no credentials cross the
/// worker transport. The durable session/role fence must still be rechecked.
#[derive(Clone)]
pub(crate) struct CapturedArtifactRequestScope(Arc<RbacRequestScopeState>);

impl CapturedArtifactRequestScope {
    pub(crate) fn capture(tenant_id: Uuid, actor_id: Uuid) -> Option<Self> {
        CURRENT_RBAC_SCOPE
            .try_with(|state| {
                let state = state.as_ref()?;
                let credential = state.scope.credential.as_ref()?;
                (state.matches_valid(&tenant_id, &actor_id)
                    && credential.principal_kind == AuthPrincipalKind::DirectUser
                    && credential.typed().is_some())
                .then(|| Self(Arc::clone(state)))
            })
            .ok()
            .flatten()
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.0.is_valid()
    }

    pub(crate) fn expires_at(&self) -> DateTime<Utc> {
        // Construction requires the original authenticated credential.
        self.0
            .scope
            .credential
            .as_ref()
            .expect("captured credential")
            .expires_at
    }

    pub(crate) async fn run<F: Future>(&self, future: F) -> F::Output {
        CURRENT_RBAC_SCOPE
            .scope(Some(Arc::clone(&self.0)), future)
            .await
    }
}

/// A missing scope permits the caller's explicit unscoped path; an invalid
/// matching scope is an error, never an equivalent absence.
fn checked_scope_value<T>(
    tenant_id: &Uuid,
    actor_id: &Uuid,
    read: impl FnOnce(&RbacRequestScope) -> T,
) -> crate::error::Result<Option<T>> {
    CURRENT_RBAC_SCOPE
        .try_with(|state| {
            let Some(state) = state.as_ref().filter(|state| {
                state.scope.matches(tenant_id, actor_id)
            }) else {
                return Ok(None);
            };
            if !state.is_valid() {
                return Err(invalidated_scope_error());
            }
            let value = read(&state.scope);
            if !state.is_valid() {
                return Err(invalidated_scope_error());
            }
            Ok(Some(value))
        })
        .unwrap_or(Ok(None))
}

fn invalidated_scope_error() -> crate::error::Error {
    crate::error::Error::Cache("request RBAC snapshot was invalidated".to_string())
}

pub(crate) fn checked_permissions_for(
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> crate::error::Result<Option<Vec<Permission>>> {
    checked_scope_value(tenant_id, actor_id, |scope| scope.permissions.clone())
}

pub fn permissions_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<Vec<Permission>> {
    checked_permissions_for(tenant_id, actor_id).ok().flatten()
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

pub(crate) fn checked_role_for(
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> crate::error::Result<Option<UserRole>> {
    checked_scope_value(tenant_id, actor_id, |scope| scope.role.clone())
}

pub fn role_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<UserRole> {
    checked_role_for(tenant_id, actor_id).ok().flatten()
}

/// The HTTP/WS GraphQL admission marker is mandatory authority evidence.
/// Never interpret a lost or substituted task-local scope as a trusted call.
pub(crate) fn ensure_graphql_scope_binding(
    expected: &RbacRequestScope,
    auth: Option<&rustok_api::AuthContext>,
    principal: Option<AuthPrincipalKind>,
) -> crate::error::Result<()> {
    let auth = auth.ok_or_else(invalidated_scope_error)?;
    if !expected.matches(&auth.tenant_id, &auth.user_id)
        || !auth.permissions.iter().all(|permission| {
            rustok_rbac::has_effective_permission_in_set(&expected.permissions, permission)
        })
    {
        return Err(invalidated_scope_error());
    }
    if let Some(credential) = &expected.credential {
        let facts = AuthPrincipalKind::from_authenticated_facts(
            &auth.grant_type,
            auth.client_id,
            auth.session_id,
        );
        let oauth_matches = match &credential.oauth {
            Some(oauth) => auth.client_id == Some(oauth.client_id) && auth.scopes == oauth.scopes,
            None => auth.client_id.is_none() && auth.scopes.is_empty(),
        };
        if facts != Some(credential.principal_kind)
            || principal != facts
            || auth.session_id != credential.session_id
            || credential.typed().is_none()
            || !oauth_matches
        {
            return Err(invalidated_scope_error());
        }
    }

    let bound = CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state.as_ref().is_some_and(|state| {
                &state.scope == expected && state.is_valid()
            })
        })
        .unwrap_or(false);
    if !bound {
        return Err(invalidated_scope_error());
    }
    Ok(())
}

/// Reject unscoped fallback for an invalidated or expired matching request snapshot.
///
/// Absence of a scope, or a scope for a different tenant/actor, retains the
/// existing persisted-grant behavior. Authenticated snapshots also observe
/// the process-local epoch; this is not a fence for already admitted checks
/// or invalidations not yet delivered by another replica.
pub(crate) fn ensure_scope_not_invalidated(
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> crate::error::Result<()> {
    let invalidated = CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state.as_ref().is_some_and(|state| {
                state.scope.matches(tenant_id, actor_id) && !state.is_valid()
            })
        })
        .unwrap_or(false);
    if invalidated {
        return Err(invalidated_scope_error());
    }
    Ok(())
}

/// Revalidate the original static authority at an asynchronous authorization
/// boundary. Never refresh the snapshot or replace its permission ceiling.
/// The owner read is a single statement, not a transaction-spanning fence.
pub(crate) async fn ensure_scope_authority(
    db: Option<&sea_orm::DatabaseConnection>,
    tenant_id: &Uuid,
    actor_id: &Uuid,
) -> crate::error::Result<()> {
    ensure_scope_not_invalidated(tenant_id, actor_id)?;
    let state = CURRENT_RBAC_SCOPE
        .try_with(|state| {
            state
                .as_ref()
                .filter(|state| state.scope.matches(tenant_id, actor_id))
                .map(Arc::clone)
        })
        .ok()
        .flatten();
    let Some(state) = state else {
        return Ok(());
    };

    // Trusted grant-only scopes and service principals do not have a
    // persisted static user membership. Authenticated human principals do.
    let Some(expected) = state.scope.static_revision.as_ref() else {
        if state.scope.admitted_epoch.is_some()
            && state.scope.credential.as_ref().is_some_and(|credential| {
                credential.principal_kind != AuthPrincipalKind::Service
            })
        {
            state.valid.store(false, Ordering::Release);
        }
        return ensure_scope_not_invalidated(tenant_id, actor_id);
    };
    let current = match db {
        Some(db) => {
            rustok_rbac::read_static_actor_authority_revision_on(db, *tenant_id, *actor_id)
                .await
                .ok()
        }
        None => None,
    };
    if current.as_ref() != Some(expected.as_ref()) {
        // Missing/corrupt data, storage failure and revision drift all deny.
        // All captured clones share the terminal flag.
        state.valid.store(false, Ordering::Release);
    }
    // Recheck the local epoch and credential deadline after the DB await.
    ensure_scope_not_invalidated(tenant_id, actor_id)
}

/// Invalidate the current task-local RBAC snapshot when the mutation targets its exact actor.
///
/// Later scope-aware checks in this task fail rather than reuse the snapshot or fall back to
/// unscoped grants. This helper affects only the current task; cache invalidation also
/// advances the shared epoch checked by authenticated scopes in other tasks.
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
    #[serial_test::serial]
    async fn admitted_scope_rejects_cross_task_and_pre_entry_epoch_changes() {
        use super::super::rbac_runtime::{
            invalidate_all_user_permissions_cache, invalidate_user_permissions_cache,
            request_authority_epoch,
        };

        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let fresh_scope = || {
            let mut scope = RbacRequestScope::new(
                tenant,
                actor,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            );
            scope.admitted_epoch = Some(request_authority_epoch(tenant, actor).unwrap());
            scope.credential = Some(RequestCredentialAuthority {
                principal_kind: AuthPrincipalKind::DirectUser,
                session_id: Uuid::new_v4(),
                expires_at: Utc::now() + chrono::Duration::minutes(5),
                recovery_revision: 1,
                oauth: None,
            });
            scope
        };

        let stale = fresh_scope();
        invalidate_user_permissions_cache(&tenant, &actor).await;
        with_rbac_request_scope(Some(stale), async {
            assert!(permissions_for(&tenant, &actor).is_none());
            assert!(ensure_scope_not_invalidated(&tenant, &actor).is_err());
        })
        .await;

        for full in [false, true] {
            with_rbac_request_scope(Some(fresh_scope()), async {
                assert!(permissions_for(&tenant, &actor).is_some());
                let captured = CapturedArtifactRequestScope::capture(tenant, actor).unwrap();
                tokio::spawn(async move {
                    if full {
                        invalidate_all_user_permissions_cache().await;
                    } else {
                        invalidate_user_permissions_cache(&tenant, &actor).await;
                    }
                })
                .await
                .unwrap();
                assert!(permissions_for(&tenant, &actor).is_none());
                assert!(role_for(&tenant, &actor).is_none());
                assert!(credential_for(&tenant, &actor).is_none());
                assert!(ensure_scope_not_invalidated(&tenant, &actor).is_err());
                assert!(!captured.is_valid());
                captured
                    .run(async {
                        assert!(ensure_scope_not_invalidated(&tenant, &actor).is_err());
                    })
                    .await;
            })
            .await;
        }
        with_rbac_request_scope(Some(fresh_scope()), async {
            assert!(permissions_for(&tenant, &actor).is_some());
        })
        .await;
    }

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
    async fn checked_scope_read_distinguishes_absence_from_invalidation() {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant,
            actor,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );
        assert!(checked_permissions_for(&tenant, &actor).unwrap().is_none());
        with_rbac_request_scope(Some(scope), async {
            assert_eq!(
                checked_permissions_for(&tenant, &actor).unwrap(),
                Some(vec![Permission::USERS_READ])
            );
            assert_eq!(checked_role_for(&tenant, &actor).unwrap(), Some(UserRole::Admin));
            // Model invalidation while the snapshot value is being obtained.
            let result = checked_scope_value(&tenant, &actor, |scope| {
                invalidate_current_rbac_request_scope(&tenant, &actor);
                scope.permissions.clone()
            });
            assert!(matches!(result, Err(crate::error::Error::Cache(_))));
            assert!(checked_permissions_for(&tenant, &actor).is_err());
            assert!(checked_role_for(&tenant, &actor).is_err());
            assert!(checked_permissions_for(&tenant, &Uuid::new_v4()).unwrap().is_none());
        })
        .await;
        assert!(checked_role_for(&tenant, &actor).unwrap().is_none());
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
            expires_at: Utc::now() + chrono::Duration::minutes(5),
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
    async fn expired_authenticated_scope_rejects_matching_lookups_without_fallback() {
        for kind in [
            AuthPrincipalKind::DirectUser,
            AuthPrincipalKind::DelegatedUser,
            AuthPrincipalKind::Service,
        ] {
            let tenant = Uuid::new_v4();
            let actor = Uuid::new_v4();
            let mut scope = RbacRequestScope::new(
                tenant,
                actor,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            );
            scope.credential = Some(RequestCredentialAuthority {
                principal_kind: kind,
                session_id: Uuid::new_v4(),
                expires_at: Utc::now() - chrono::Duration::seconds(1),
                recovery_revision: 1,
                oauth: (kind != AuthPrincipalKind::DirectUser).then(|| OAuthCredentialEvidence {
                    registration_id: Uuid::new_v4(),
                    app_id: Uuid::new_v4(),
                    client_id: Uuid::new_v4(),
                    token_hash: "digest".into(),
                    issued_at: Utc::now() - chrono::Duration::minutes(1),
                    consent_granted_at: None,
                    scopes: vec!["users:manage".into()],
                }),
            });
            with_rbac_request_scope(Some(scope), async {
                assert!(permissions_for(&tenant, &actor).is_none());
                assert!(role_for(&tenant, &actor).is_none());
                assert!(credential_for(&tenant, &actor).is_none());
                assert!(bind_admin_context(tenant, actor, kind, None).is_err());
                assert!(CapturedArtifactRequestScope::capture(tenant, actor).is_none());
                assert!(matches!(
                    ensure_scope_not_invalidated(&tenant, &actor),
                    Err(crate::error::Error::Cache(_))
                ));
                assert!(ensure_scope_not_invalidated(&tenant, &Uuid::new_v4()).is_ok());
                assert!(ensure_scope_not_invalidated(&Uuid::new_v4(), &actor).is_ok());
                let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
                use crate::services::rbac_service::RbacService;
                assert!(matches!(
                    RbacService::has_permission(&db, &tenant, &actor, &Permission::USERS_READ).await,
                    Err(crate::error::Error::Cache(_))
                ));
                assert!(matches!(
                    RbacService::get_user_permissions(&db, &tenant, &actor).await,
                    Err(crate::error::Error::Cache(_))
                ));
                assert!(matches!(
                    RbacService::get_user_role(&db, &tenant, &actor).await,
                    Err(crate::error::Error::Cache(_))
                ));
            })
            .await;
        }
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
                let capture = CapturedArtifactRequestScope::capture(tenant, actor);
                assert_eq!(capture.is_some(), kind == AuthPrincipalKind::DirectUser);
                assert!(CapturedArtifactRequestScope::capture(Uuid::new_v4(), actor).is_none());
                if let Some(capture) = capture.as_ref() {
                    let child = capture.clone();
                    tokio::spawn(async move {
                        assert!(require_artifact_direct_credential(tenant, actor).is_err());
                        child
                            .run(async {
                                assert!(require_artifact_direct_credential(tenant, actor).is_ok());
                            })
                            .await;
                        assert!(require_artifact_direct_credential(tenant, actor).is_err());
                    })
                    .await
                    .unwrap();
                }
                invalidate_current_rbac_request_scope(&tenant, &actor);
                assert!(require_artifact_direct_credential(tenant, actor).is_err());
                if let Some(capture) = capture {
                    assert!(!capture.is_valid());
                    capture
                        .run(async {
                            assert!(require_artifact_direct_credential(tenant, actor).is_err());
                        })
                        .await;
                }
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
