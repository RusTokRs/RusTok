//! Host composition boundary for artifact purge and settings recovery.

use async_trait::async_trait;
use rustok_modules::{
    ArtifactDataError, ArtifactDataPurgeAuthorizationContext, ArtifactDataPurgeAuthorizer,
    ArtifactDataPurgeRequest, ArtifactSettingsRecoveryAuthorizer, ArtifactSettingsRecoveryCipher,
    SeaOrmArtifactSettingsRecoveryService,
};
use sea_orm::DatabaseConnection;

/// Settings recovery requires host-composed policy and authenticated encryption.
/// Transports never construct a default authorizer or cipher.
pub type ArtifactSettingsRecoveryRuntime = SeaOrmArtifactSettingsRecoveryService<
    dyn ArtifactSettingsRecoveryAuthorizer,
    dyn ArtifactSettingsRecoveryCipher,
>;

/// Host authorization for artifact structured data purge.
#[derive(Clone)]
pub struct ServerArtifactDataPurgeAuthorizer {
    db: DatabaseConnection,
}

impl ServerArtifactDataPurgeAuthorizer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ArtifactDataPurgeAuthorizer for ServerArtifactDataPurgeAuthorizer {
    async fn authorize_purge(
        &self,
        request: &ArtifactDataPurgeRequest,
        owner: &ArtifactDataPurgeAuthorizationContext,
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
        let decision = rustok_rbac::authorize_current_permission(
            &self.db,
            &owner.scope.tenant_id,
            &request.context.actor_id,
            &rustok_api::Permission::MODULES_MANAGE,
        )
        .await
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
        if !decision.allowed {
            return Err(ArtifactDataError::PolicyDenied);
        }
        Ok(())
    }
}
