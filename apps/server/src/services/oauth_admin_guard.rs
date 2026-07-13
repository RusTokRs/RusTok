use std::sync::Arc;

use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::oauth_app::OAuthAppService;

pub struct GuardedOAuthAdminProvider {
    db: DatabaseConnection,
    inner: Arc<dyn OAuthAdminPort>,
}

impl GuardedOAuthAdminProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn OAuthAdminPort>) -> Self {
        Self { db, inner }
    }
}

fn validate_grant_dependencies(grant_types: &[String]) -> Result<(), AuthAdminMutationError> {
    let has_authorization_code = grant_types
        .iter()
        .any(|grant| grant.trim() == "authorization_code");
    let has_refresh_token = grant_types
        .iter()
        .any(|grant| grant.trim() == "refresh_token");

    if has_refresh_token && !has_authorization_code {
        return Err(AuthAdminMutationError::Validation(
            "refresh_token grant requires authorization_code grant".to_string(),
        ));
    }

    Ok(())
}

#[async_trait]
impl OAuthAdminPort for GuardedOAuthAdminProvider {
    async fn list_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        app_type: Option<String>,
        limit: u64,
    ) -> Result<Vec<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.inner.list_oauth_apps(context, app_type, limit).await
    }

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.inner.get_oauth_app(context, app_id).await
    }

    async fn list_authorized_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<AuthorizedOAuthAppRecord>, AuthAdminMutationError> {
        self.inner.list_authorized_oauth_apps(context, limit).await
    }

    async fn create_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        validate_grant_dependencies(&command.grant_types)?;
        self.inner.create_oauth_app(context, command).await
    }

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        validate_grant_dependencies(&command.grant_types)?;
        self.inner.update_oauth_app(context, command).await
    }

    async fn rotate_oauth_app_secret(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.inner.rotate_oauth_app_secret(context, app_id).await
    }

    async fn revoke_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.inner.revoke_oauth_app(context, app_id).await
    }

    async fn grant_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<(), AuthAdminMutationError> {
        OAuthAppService::grant_consent_strict(
            &self.db,
            app_id,
            context.actor_id,
            context.tenant_id,
            scopes,
        )
        .await
        .map_err(map_consent_error)
    }

    async fn revoke_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        self.inner.revoke_oauth_app_consent(context, app_id).await
    }
}

fn map_consent_error(error: crate::error::Error) -> AuthAdminMutationError {
    match error {
        crate::error::Error::NotFound => AuthAdminMutationError::NotFound("oauth app".to_string()),
        crate::error::Error::BadRequest(message) => AuthAdminMutationError::Validation(message),
        crate::error::Error::Unauthorized(_) => AuthAdminMutationError::Unauthorized,
        other => AuthAdminMutationError::Internal(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_grant_dependencies;

    #[test]
    fn refresh_grant_requires_authorization_code() {
        assert!(validate_grant_dependencies(&["refresh_token".to_string()]).is_err());
        assert!(validate_grant_dependencies(&[
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ])
        .is_ok());
    }

    #[test]
    fn non_refresh_grants_remain_independent() {
        assert!(validate_grant_dependencies(&["client_credentials".to_string()]).is_ok());
    }
}
