use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{Permission, has_effective_permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand, generate_refresh_token, hash_password,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::models::oauth_apps;

use super::oauth_app::OAuthAppService;
use super::rbac_request_scope::permissions_for;

pub struct GuardedOAuthAdminProvider {
    db: DatabaseConnection,
    inner: Arc<dyn OAuthAdminPort>,
}

impl GuardedOAuthAdminProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn OAuthAdminPort>) -> Self {
        Self { db, inner }
    }

    fn request_permissions(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "OAuth administration requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }

    fn require_settings_manage(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        let authority = self.request_permissions(context)?;
        if has_effective_permission(&authority, &Permission::SETTINGS_MANAGE) {
            Ok(authority)
        } else {
            Err(AuthAdminMutationError::Forbidden(
                "settings:manage required for OAuth application administration".to_string(),
            ))
        }
    }

    fn validate_permission_strings(
        &self,
        authority: &[Permission],
        granted_permissions: &[String],
    ) -> Result<(), AuthAdminMutationError> {
        for raw in granted_permissions {
            let permission = Permission::from_str(raw.trim()).map_err(|error| {
                AuthAdminMutationError::Validation(format!(
                    "OAuth app contains invalid delegated permission `{raw}`: {error}"
                ))
            })?;
            if !has_effective_permission(authority, &permission) {
                return Err(AuthAdminMutationError::Forbidden(format!(
                    "OAuth app permission `{permission}` exceeds the current request authority"
                )));
            }
        }
        Ok(())
    }

    async fn rotate_secret_transactionally(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        response_record: OAuthAppMutationRecord,
        authority: &[Permission],
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let app = lock_oauth_app(&tx, context.tenant_id, app_id).await?;
        self.validate_permission_strings(authority, &app.granted_permissions_list())?;
        if !app.can_rotate_secret() {
            return Err(AuthAdminMutationError::Validation(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }

        let client_secret = format!(
            "sk_live_{}{}",
            generate_refresh_token(),
            generate_refresh_token()
        );
        let secret_hash = hash_password(&client_secret)
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let mut active: oauth_apps::ActiveModel = app.into();
        active.client_secret_hash = Set(Some(secret_hash));
        active.updated_at = Set(Utc::now().into());
        active
            .update(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;

        Ok(OAuthAppSecretResult {
            app: response_record,
            client_secret,
        })
    }
}

async fn lock_oauth_app<C>(
    db: &C,
    tenant_id: Uuid,
    app_id: Uuid,
) -> Result<oauth_apps::Model, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let query = || {
        oauth_apps::Entity::find_by_id(app_id).filter(oauth_apps::Column::TenantId.eq(tenant_id))
    };
    let app = match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query()
            .lock_exclusive()
            .one(db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?,
        DbBackend::Sqlite => {
            let app = query()
                .one(db)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            if let Some(app) = app.as_ref() {
                oauth_apps::Entity::update_many()
                    .col_expr(
                        oauth_apps::Column::UpdatedAt,
                        Expr::col(oauth_apps::Column::UpdatedAt).into(),
                    )
                    .filter(oauth_apps::Column::Id.eq(app.id))
                    .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                    .exec(db)
                    .await
                    .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            }
            app
        }
    };

    app.ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))
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
        self.require_settings_manage(context)?;
        self.inner.list_oauth_apps(context, app_type, limit).await
    }

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.require_settings_manage(context)?;
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
        let authority = self.require_settings_manage(context)?;
        validate_grant_dependencies(&command.grant_types)?;
        self.validate_permission_strings(&authority, &command.granted_permissions)?;
        self.inner.create_oauth_app(context, command).await
    }

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        let authority = self.require_settings_manage(context)?;
        validate_grant_dependencies(&command.grant_types)?;
        self.validate_permission_strings(&authority, &command.granted_permissions)?;
        self.inner.update_oauth_app(context, command).await
    }

    async fn rotate_oauth_app_secret(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let authority = self.require_settings_manage(context)?;
        let app = self
            .inner
            .get_oauth_app(context, app_id)
            .await?
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        self.rotate_secret_transactionally(context, app_id, app, &authority)
            .await
    }

    async fn revoke_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.require_settings_manage(context)?;
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
        assert!(
            validate_grant_dependencies(&[
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ])
            .is_ok()
        );
    }

    #[test]
    fn non_refresh_grants_remain_independent() {
        assert!(validate_grant_dependencies(&["client_credentials".to_string()]).is_ok());
    }
}
