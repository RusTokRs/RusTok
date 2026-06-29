use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateOAuthAppCommand,
    OAuthAdminMutationPort, OAuthAppMutationRecord, OAuthAppSecretResult, UpdateOAuthAppCommand,
};
use rustok_core::Permission;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::models::{oauth_apps, oauth_tokens};
use crate::services::oauth_app::{self, OAuthAppService};
use crate::services::rbac_service::RbacService;

pub struct ServerOAuthAdminMutationProvider {
    db: DatabaseConnection,
}

impl ServerOAuthAdminMutationProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn authorize(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<(), AuthAdminMutationError> {
        let allowed = RbacService::has_any_permission(
            &self.db,
            &context.tenant_id,
            &context.actor_id,
            &[Permission::SETTINGS_MANAGE, Permission::USERS_MANAGE],
        )
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;

        if allowed {
            Ok(())
        } else {
            Err(AuthAdminMutationError::Forbidden(
                "settings:manage or users:manage required".to_string(),
            ))
        }
    }

    async fn record(
        &self,
        app: oauth_apps::Model,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        let active_token_count = oauth_tokens::Entity::count_active_by_app(&self.db, app.id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let redirect_uris = app.redirect_uris_list();
        let scopes = app.scopes_list();
        let grant_types = app.grant_types_list();
        let is_active = app.is_active();

        Ok(OAuthAppMutationRecord {
            id: app.id,
            name: app.name,
            slug: app.slug,
            description: app.description,
            icon_url: app.icon_url,
            app_type: app.app_type,
            client_id: app.client_id,
            redirect_uris,
            scopes,
            grant_types,
            manifest_ref: app.manifest_ref,
            auto_created: app.auto_created,
            is_active,
            active_token_count: i64::try_from(active_token_count).unwrap_or(i64::MAX),
            last_used_at: app.last_used_at.map(Into::into),
            created_at: app.created_at.into(),
        })
    }
}

fn map_service_error(error: crate::error::Error) -> AuthAdminMutationError {
    match error {
        crate::error::Error::NotFound => AuthAdminMutationError::NotFound("oauth app".to_string()),
        crate::error::Error::BadRequest(message) => AuthAdminMutationError::Validation(message),
        crate::error::Error::Unauthorized(_) => AuthAdminMutationError::Unauthorized,
        other => AuthAdminMutationError::Internal(other.to_string()),
    }
}

#[async_trait]
impl OAuthAdminMutationPort for ServerOAuthAdminMutationProvider {
    async fn create_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.authorize(context).await?;
        let result = OAuthAppService::create_app(
            &self.db,
            context.tenant_id,
            oauth_app::CreateOAuthAppInput {
                name: command.name,
                slug: command.slug,
                description: command.description,
                app_type: command.app_type,
                icon_url: command.icon_url,
                redirect_uris: command.redirect_uris,
                scopes: command.scopes,
                grant_types: command.grant_types,
                granted_permissions: command.granted_permissions,
            },
        )
        .await
        .map_err(map_service_error)?;

        Ok(OAuthAppSecretResult {
            app: self.record(result.app).await?,
            client_secret: result.client_secret,
        })
    }

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = OAuthAppService::update_app(
            &self.db,
            context.tenant_id,
            command.id,
            oauth_app::UpdateOAuthAppInput {
                name: command.name,
                description: command.description,
                icon_url: command.icon_url,
                redirect_uris: command.redirect_uris,
                scopes: command.scopes,
                grant_types: command.grant_types,
                granted_permissions: command.granted_permissions,
            },
        )
        .await
        .map_err(map_service_error)?;
        self.record(app).await
    }

    async fn rotate_oauth_app_secret(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .filter(|app| app.tenant_id == context.tenant_id)
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        let result = OAuthAppService::rotate_secret(&self.db, app.id)
            .await
            .map_err(map_service_error)?;
        Ok(OAuthAppSecretResult {
            app: self.record(result.app).await?,
            client_secret: result.client_secret,
        })
    }

    async fn revoke_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .filter(|app| app.tenant_id == context.tenant_id)
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        let revoked = OAuthAppService::revoke_app(&self.db, app.id)
            .await
            .map_err(map_service_error)?;
        self.record(revoked).await
    }
}
