//! Host composition boundary for artifact purge and settings recovery.

use async_trait::async_trait;
use rustok_modules::{
    ArtifactDataError, ArtifactDataPurgeAuthorizationContext, ArtifactDataPurgeAuthorizer,
    ArtifactDataPurgeRequest, ArtifactSettingsRecoveryAuthorizer, ArtifactSettingsRecoveryCipher,
    SeaOrmArtifactSettingsRecoveryService,
};
use rustok_api::{AuthPrincipalKind, Permission, has_effective_permission};
use rustok_rbac::graphql::{
    RbacGraphqlRoleWriteError, RbacLockedMutationAuthority, RbacMutationAuthorityHandle,
};
use sea_orm::DatabaseTransaction;

use crate::services::rbac_request_scope::{bind_admin_context, permissions_for};

/// Settings recovery requires host-composed policy and authenticated encryption.
/// Transports never construct a default authorizer or cipher.
pub type ArtifactSettingsRecoveryRuntime = SeaOrmArtifactSettingsRecoveryService<
    dyn ArtifactSettingsRecoveryAuthorizer,
    dyn ArtifactSettingsRecoveryCipher,
>;

/// Host authorization for artifact structured data purge.
#[derive(Clone)]
pub struct ServerArtifactDataPurgeAuthorizer(pub RbacMutationAuthorityHandle);

fn map_authority_error(error: RbacGraphqlRoleWriteError) -> ArtifactDataError {
    match error {
        RbacGraphqlRoleWriteError::Forbidden(_) | RbacGraphqlRoleWriteError::NotFound(_) => {
            ArtifactDataError::PolicyDenied
        }
        other => ArtifactDataError::Storage(other.to_string()),
    }
}

#[async_trait]
impl ArtifactDataPurgeAuthorizer for ServerArtifactDataPurgeAuthorizer {
    type LockedAuthority = RbacLockedMutationAuthority;

    async fn prepare_purge(&self) -> Result<(), ArtifactDataError> {
        self.0.0.prepare().await.map_err(map_authority_error)
    }

    async fn lock_purge_authority_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
    ) -> Result<Self::LockedAuthority, ArtifactDataError> {
        let tenant_id = request.context.tenant_id.ok_or(ArtifactDataError::PolicyDenied)?;
        let authority = self.0.0.lock(
            transaction,
            tenant_id,
            request.context.actor_id,
            None,
            &[Permission::MODULES_MANAGE],
        ).await.map_err(map_authority_error)?;
        if !has_effective_permission(&authority.permissions, &Permission::MODULES_MANAGE) {
            return Err(ArtifactDataError::PolicyDenied);
        }
        Ok(authority)
    }

    async fn authorize_purge_on(
        &self,
        _transaction: &DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
        owner: &ArtifactDataPurgeAuthorizationContext,
        _authority: &Self::LockedAuthority,
    ) -> Result<(), ArtifactDataError> {
        if request.context.validate().is_err()
            || request.reason.trim().is_empty()
            || request.installation_id.is_nil()
            || request.installation_id != owner.installation_id
            || owner.data_owner_id.is_nil()
            || owner.data_owner_id != owner.scope.data_owner_id
            || owner.installation_revision == 0
            || owner.scope.validate().is_err()
            || request.context.tenant_id != Some(owner.scope.tenant_id)
        {
            return Err(ArtifactDataError::PolicyDenied);
        }
        Ok(())
    }

    async fn ensure_purge_authority_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
        authority: &Self::LockedAuthority,
    ) -> Result<(), ArtifactDataError> {
        self.0.0.ensure_deadline(transaction, authority)
            .await
            .map_err(map_authority_error)?;
        let tenant_id = request.context.tenant_id.ok_or(ArtifactDataError::PolicyDenied)?;
        // A request scope invalidated during the owner work cannot release
        // a receipt or commit destructive state.
        bind_admin_context(tenant_id, request.context.actor_id, AuthPrincipalKind::DirectUser, None)
            .map_err(|_| ArtifactDataError::PolicyDenied)?;
        let admitted = permissions_for(&tenant_id, &request.context.actor_id)
            .ok_or(ArtifactDataError::PolicyDenied)?;
        if !has_effective_permission(&admitted, &Permission::MODULES_MANAGE) {
            return Err(ArtifactDataError::PolicyDenied);
        }
        Ok(())
    }
}
