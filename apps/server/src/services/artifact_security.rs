use async_trait::async_trait;
use rustok_api::{
    AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission, has_effective_permission,
};
use rustok_modules::{
    ArtifactReleaseRef, ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecurityAuthorizer,
    ModuleArtifactSecurityCommand, ModuleArtifactSecurityError, ModuleArtifactSecurityReceipt,
    ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityStatus, ModuleCommandContext,
};
use crate::services::{
    auth_admin_mutation_provider::ServerAuthAdminMutationProvider,
    rbac_runtime::{authorize_request, AuthorizationCheck},
    server_runtime_context::ServerRuntimeContext,
};
use rustok_api::{
    AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission, has_effective_permission,
};
use rustok_auth::{
    AdminCredentialRequest, AuthAdminCredentialResolver, AuthAdminMutationError,
};
use rustok_modules::{
    ArtifactReleaseRef, ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecurityAuthorizer,
    ModuleArtifactSecurityCommand, ModuleArtifactSecurityError, ModuleArtifactSecurityReceipt,
    ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityStatus, ModuleCommandContext,
};
use rustok_rbac::{RbacControlPlanePrincipal, require_direct_control_plane_user};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactSecurityAuthorizationError {
    #[error("A direct authenticated user session is required")]
    DirectSessionRequired,
    #[error("Permission denied: modules:manage required")]
    ModulesManageRequired,
    #[error("Artifact security authorization lookup failed")]
    PermissionCheckFailed,
}

fn validate_direct_operator(
    auth: &AuthContext,
    principal: AuthPrincipalContext,
    tenant_id: Uuid,
) -> Result<(), ArtifactSecurityAuthorizationError> {
    if tenant_id.is_nil()
        || auth.user_id.is_nil()
        || auth.tenant_id != tenant_id
        || auth.grant_type != "direct"
        || auth.client_id.is_some()
        || auth.session_id.is_nil()
        || principal.kind != AuthPrincipalKind::DirectUser
    {
        return Err(ArtifactSecurityAuthorizationError::DirectSessionRequired);
    }

    require_direct_control_plane_user(
        RbacControlPlanePrincipal {
            tenant_id: auth.tenant_id,
            principal_kind: principal.kind,
        },
        tenant_id,
    )
    .map_err(|_| ArtifactSecurityAuthorizationError::DirectSessionRequired)?;

    if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
        return Err(ArtifactSecurityAuthorizationError::ModulesManageRequired);
    }

    Ok(())
}

/// Resolve the request-scoped effective tenant permission through the
/// canonical RBAC engine. Mutations use the transaction-bound authority below.
async fn current_modules_manage_permission(
    db: &DatabaseConnection,
    tenant_id: &Uuid,
    user_id: &Uuid,
) -> Result<bool, ArtifactSecurityAuthorizationError> {
    let outcome = authorize_request(
        db,
        tenant_id,
        user_id,
        AuthorizationCheck::Single(&Permission::MODULES_MANAGE),
    )
    .await
    .map_err(|_| ArtifactSecurityAuthorizationError::PermissionCheckFailed)?;

    Ok(outcome.decision.allowed)
}

pub async fn authorize_artifact_security_operator(
    db: &DatabaseConnection,
    auth: &AuthContext,
    principal: AuthPrincipalContext,
    tenant_id: Uuid,
) -> Result<(), ArtifactSecurityAuthorizationError> {
    validate_direct_operator(auth, principal, tenant_id)?;

    let allowed = current_modules_manage_permission(db, &tenant_id, &auth.user_id).await?;

    if !allowed {
        return Err(ArtifactSecurityAuthorizationError::ModulesManageRequired);
    }

    Ok(())
}

fn command_is_bound_to_operator(
    actor_id: Uuid,
    tenant_id: Uuid,
    principal_kind: AuthPrincipalKind,
    permissions: &[Permission],
    command: &ModuleArtifactSecurityCommand,
) -> bool {
    !actor_id.is_nil()
        && !tenant_id.is_nil()
        && principal_kind == AuthPrincipalKind::DirectUser
        && command.context.actor_id == actor_id
        && command.context.tenant_id.is_none()
        && !command.context.idempotency_key.is_nil()
        && command.context.correlation_id == command.context.idempotency_key
        && has_effective_permission(permissions, &Permission::MODULES_MANAGE)
}

/// Retrieve the same host credential authority used by artifact execution and
/// RBAC owner mutations. Missing composition is an authorization failure.
pub(crate) fn artifact_security_authority_provider(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<Arc<ServerAuthAdminMutationProvider>, ArtifactSecurityAuthorizationError> {
    runtime_ctx
        .shared_get::<Arc<rustok_core::ModuleRuntimeExtensions>>()
        .and_then(|extensions| {
            extensions
                .get::<Arc<ServerAuthAdminMutationProvider>>()
                .cloned()
        })
        .ok_or(ArtifactSecurityAuthorizationError::PermissionCheckFailed)
}

fn map_admin_authorization_error(error: AuthAdminMutationError) -> ModuleArtifactSecurityError {
    match error {
        AuthAdminMutationError::Unauthorized
        | AuthAdminMutationError::Forbidden(_)
        | AuthAdminMutationError::Validation(_)
        | AuthAdminMutationError::NotFound(_) => {
            ModuleArtifactSecurityError::AuthorizationDenied(
                "direct session user with modules:manage required".to_string(),
            )
        }
        AuthAdminMutationError::CustomFieldsValidation(_)
        | AuthAdminMutationError::Conflict(_)
        | AuthAdminMutationError::Internal(_) => ModuleArtifactSecurityError::Store(
            "artifact security authorization lookup failed".to_string(),
        ),
    }
}

/// Request-bound authorization evidence passed into the Modules owner. The
/// owner invokes this adapter inside its transition transaction, before
/// idempotency replay and before taking the platform release slug fence.
#[derive(Clone)]
pub struct ServerArtifactSecurityAuthorizer {
    provider: Arc<ServerAuthAdminMutationProvider>,
    actor_id: Uuid,
    tenant_id: Uuid,
    principal_kind: AuthPrincipalKind,
    permissions: Vec<Permission>,
}

impl ServerArtifactSecurityAuthorizer {
    pub async fn new(
        provider: Arc<ServerAuthAdminMutationProvider>,
        auth: &AuthContext,
        principal: AuthPrincipalContext,
        tenant_id: Uuid,
    ) -> Result<Self, ArtifactSecurityAuthorizationError> {
        validate_direct_operator(auth, principal, tenant_id)?;
        // Bind the transport's exact session identity to the current admitted
        // request scope; the transaction check below then revalidates that same
        // credential while holding its user/session fences.
        provider
            .bind(
                AdminCredentialRequest {
                    tenant_id,
                    user_id: auth.user_id,
                    session_id: auth.session_id,
                    client_id: auth.client_id,
                    grant_type: &auth.grant_type,
                    scopes: &auth.scopes,
                },
                principal.kind,
                None,
            )
            .map_err(|_| ArtifactSecurityAuthorizationError::DirectSessionRequired)?;
        // Admit the Auth application clock before the owner opens a transaction.
        provider
            .prepare_artifact_effect_authority()
            .await
            .map_err(|_| ArtifactSecurityAuthorizationError::PermissionCheckFailed)?;
        Ok(Self {
            provider,
            actor_id: auth.user_id,
            tenant_id,
            principal_kind: principal.kind,
            permissions: auth.permissions.clone(),
        })
    }

    async fn authorize_command(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError> {
        if !command_is_bound_to_operator(
            self.actor_id,
            self.tenant_id,
            self.principal_kind,
            &self.permissions,
            command,
        ) {
            return Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "direct session user with modules:manage required".to_string(),
            ));
        }

        let required = [Permission::MODULES_MANAGE];
        let authority = self
            .provider
            .lock_rbac_control_plane(
                transaction,
                self.tenant_id,
                self.actor_id,
                None,
                &required,
            )
            .await
            .map_err(map_admin_authorization_error)?;

        if has_effective_permission(&authority.permissions, &Permission::MODULES_MANAGE) {
            Ok(())
        } else {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "modules:manage permission is no longer granted".to_string(),
            ))
        }
    }
}

#[async_trait]
impl ModuleArtifactSecurityAuthorizer for ServerArtifactSecurityAuthorizer {
    async fn authorize_quarantine(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError> {
        self.authorize_command(transaction, command).await
    }

    async fn authorize_clear_quarantine(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError> {
        self.authorize_command(transaction, command).await
    }

    async fn authorize_revoke(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError> {
        self.authorize_command(transaction, command).await
    }
}

/// Builds the platform-scoped owner command. The release identity is immutable,
/// while the expected revision fences stale operator screens and the UUID is
/// used for both durable idempotency and correlation.
pub fn build_module_artifact_security_command(
    release: ArtifactReleaseRef,
    expected_revision: u64,
    policy_revision: String,
    reason_code: String,
    reason_detail: String,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> ModuleArtifactSecurityCommand {
    let trace_id = rustok_telemetry::current_trace_id()
        .filter(|trace_id| !trace_id.trim().is_empty())
        .unwrap_or_else(|| format!("artifact-security:{idempotency_key}"));

    ModuleArtifactSecurityCommand {
        release,
        expected_revision,
        policy_revision,
        reason_code,
        reason_detail,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id,
            correlation_id: idempotency_key,
            idempotency_key,
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSecuritySnapshotView {
    pub module_slug: String,
    pub version: String,
    pub payload_digest: String,
    /// Decimal string preserving the owner's full unsigned 64-bit revision.
    pub revision: String,
    pub status: String,
    pub registry_status: String,
    pub policy_revision: Option<String>,
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
}

impl From<ModuleArtifactSecuritySnapshot> for ArtifactSecuritySnapshotView {
    fn from(snapshot: ModuleArtifactSecuritySnapshot) -> Self {
        let status = match snapshot.status {
            ModuleArtifactSecurityStatus::Clear => "clear",
            ModuleArtifactSecurityStatus::Quarantined => "quarantined",
            ModuleArtifactSecurityStatus::Revoked => "revoked",
        };
        let registry_status = match snapshot.registry_status {
            ModuleArtifactRegistryReleaseStatus::Unlisted => "unlisted",
            ModuleArtifactRegistryReleaseStatus::Active => "active",
            ModuleArtifactRegistryReleaseStatus::Yanked => "yanked",
            ModuleArtifactRegistryReleaseStatus::Unavailable => "unavailable",
        };

        Self {
            module_slug: snapshot.release.slug,
            version: snapshot.release.version,
            payload_digest: snapshot.release.digest,
            revision: snapshot.revision.to_string(),
            status: status.to_string(),
            registry_status: registry_status.to_string(),
            policy_revision: snapshot.policy_revision,
            reason_code: snapshot.reason_code,
            reason_detail: snapshot.reason_detail,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSecurityReceiptView {
    pub snapshot: ArtifactSecuritySnapshotView,
    pub created: bool,
}

impl From<ModuleArtifactSecurityReceipt> for ArtifactSecurityReceiptView {
    fn from(receipt: ModuleArtifactSecurityReceipt) -> Self {
        Self {
            snapshot: receipt.snapshot.into(),
            created: receipt.created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn direct_auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn direct_principal() -> AuthPrincipalContext {
        AuthPrincipalContext::new(AuthPrincipalKind::DirectUser)
    }

    fn command(actor_id: Uuid) -> ModuleArtifactSecurityCommand {
        build_module_artifact_security_command(
            ArtifactReleaseRef {
                slug: "example".to_string(),
                version: "1.2.3".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            0,
            "policy-r1".to_string(),
            "security-review".to_string(),
            "A reviewed release compromise".to_string(),
            actor_id,
            Uuid::new_v4(),
        )
    }

    #[test]
    fn direct_session_with_modules_manage_is_validated() {
        let tenant_id = Uuid::new_v4();
        let auth = direct_auth(tenant_id, vec![Permission::MODULES_MANAGE]);

        assert!(validate_direct_operator(&auth, direct_principal(), tenant_id).is_ok());
    }

    #[test]
    fn delegated_service_and_inconsistent_direct_principals_are_rejected() {
        let tenant_id = Uuid::new_v4();
        let auth = direct_auth(tenant_id, vec![Permission::MODULES_MANAGE]);

        for kind in [AuthPrincipalKind::DelegatedUser, AuthPrincipalKind::Service] {
            assert_eq!(
                validate_direct_operator(&auth, AuthPrincipalContext::new(kind), tenant_id),
                Err(ArtifactSecurityAuthorizationError::DirectSessionRequired)
            );
        }

        let mut inconsistent = auth.clone();
        inconsistent.grant_type = "client_credentials".to_string();
        assert_eq!(
            validate_direct_operator(&inconsistent, direct_principal(), tenant_id),
            Err(ArtifactSecurityAuthorizationError::DirectSessionRequired)
        );
    }

    #[test]
    fn tenant_mismatch_and_missing_module_permission_are_rejected() {
        let tenant_id = Uuid::new_v4();
        let auth = direct_auth(tenant_id, vec![Permission::MODULES_MANAGE]);
        assert_eq!(
            validate_direct_operator(&auth, direct_principal(), Uuid::new_v4()),
            Err(ArtifactSecurityAuthorizationError::DirectSessionRequired)
        );

        let read_only = direct_auth(tenant_id, vec![Permission::MODULES_READ]);
        assert_eq!(
            validate_direct_operator(&read_only, direct_principal(), tenant_id),
            Err(ArtifactSecurityAuthorizationError::ModulesManageRequired)
        );
    }

    #[test]
    fn owner_authorizer_binds_actor_and_global_scope() {
        let tenant_id = Uuid::new_v4();
        let auth = direct_auth(tenant_id, vec![Permission::MODULES_MANAGE]);

        let valid = command(auth.user_id);
        assert!(command_is_bound_to_operator(
            auth.user_id,
            tenant_id,
            AuthPrincipalKind::DirectUser,
            &auth.permissions,
            &valid,
        ));

        let wrong_actor = command(Uuid::new_v4());
        assert!(!command_is_bound_to_operator(
            auth.user_id,
            tenant_id,
            AuthPrincipalKind::DirectUser,
            &auth.permissions,
            &wrong_actor,
        ));

        let mut tenant_scoped = command(auth.user_id);
        tenant_scoped.context.tenant_id = Some(tenant_id);
        assert!(!command_is_bound_to_operator(
            auth.user_id,
            tenant_id,
            AuthPrincipalKind::DirectUser,
            &auth.permissions,
            &tenant_scoped,
        ));
    }

    #[test]
    fn command_context_is_global_and_idempotency_bound() {
        let actor_id = Uuid::new_v4();
        let key = Uuid::new_v4();
        let command = build_module_artifact_security_command(
            ArtifactReleaseRef {
                slug: "example".to_string(),
                version: "1.2.3".to_string(),
                digest: format!("sha256:{}", "b".repeat(64)),
            },
            0,
            "policy-r1".to_string(),
            "security-review".to_string(),
            "Reviewed release".to_string(),
            actor_id,
            key,
        );

        assert_eq!(command.expected_revision, 0);
        assert_eq!(command.context.actor_id, actor_id);
        assert_eq!(command.context.tenant_id, None);
        assert_eq!(command.context.idempotency_key, key);
        assert_eq!(command.context.correlation_id, key);
        assert!(command.context.validate().is_ok());
    }

    #[test]
    fn snapshot_view_preserves_release_identity_and_security_revision() {
        let snapshot = ModuleArtifactSecuritySnapshot {
            release: ArtifactReleaseRef {
                slug: "example".to_string(),
                version: "1.2.3".to_string(),
                digest: format!("sha256:{}", "c".repeat(64)),
            },
            revision: u64::MAX,
            status: ModuleArtifactSecurityStatus::Quarantined,
            registry_status: ModuleArtifactRegistryReleaseStatus::Yanked,
            policy_revision: Some("policy-r7".to_string()),
            reason_code: Some("security-review".to_string()),
            reason_detail: Some("Reviewed release".to_string()),
        };

        let view = ArtifactSecuritySnapshotView::from(snapshot);
        assert_eq!(view.module_slug, "example");
        assert_eq!(view.revision, u64::MAX.to_string());
        assert_eq!(view.status, "quarantined");
        assert_eq!(view.registry_status, "yanked");
        assert_eq!(view.reason_code.as_deref(), Some("security-review"));
    }
}
