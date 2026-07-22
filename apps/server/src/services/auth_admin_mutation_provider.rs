use async_trait::async_trait;
use rustok_api::{Permission, has_any_effective_permission, has_effective_permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand, UserMutationRecord,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::str::FromStr;
use uuid::Uuid;

use crate::models::{oauth_apps, oauth_consents, oauth_tokens, tenants, users};
use crate::services::oauth_app::{self, OAuthAppService};
use crate::services::rbac_request_scope::permissions_for;
use crate::services::rbac_service::RbacService;

mod super_admin_guard;
mod user_admin;

#[derive(Clone)]
pub struct ServerAuthAdminMutationProvider {
    db: DatabaseConnection,
}

impl ServerAuthAdminMutationProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn request_permissions(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "auth administration requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }

    fn effective_locale(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<String, AuthAdminMutationError> {
        let locale = context.locale.as_deref().ok_or_else(|| {
            AuthAdminMutationError::Validation(
                "auth administration requires a host-resolved effective locale".to_string(),
            )
        })?;
        oauth_apps::normalize_runtime_copy_locale(locale).map_err(|_| {
            AuthAdminMutationError::Validation(
                "auth administration requires a valid effective locale other than `und`"
                    .to_string(),
            )
        })
    }

    async fn authorize_user(
        &self,
        context: &AuthAdminMutationContext,
        permissions: &[Permission],
        message: &str,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_permissions = self.request_permissions(context)?;
        if has_any_effective_permission(&actor_permissions, permissions) {
            Ok(())
        } else {
            Err(AuthAdminMutationError::Forbidden(message.to_string()))
        }
    }

    async fn user_record(
        &self,
        user: users::Model,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        let role = RbacService::get_user_role(&self.db, &user.tenant_id, &user.id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let tenant_name = tenants::Entity::find_by_id(&self.db, user.tenant_id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .map(|tenant| tenant.name);
        Ok(UserMutationRecord {
            id: user.id,
            email: user.email,
            name: user.name,
            role: role.to_string(),
            status: user.status.to_string(),
            created_at: user.created_at.with_timezone(&chrono::Utc),
            tenant_name,
            tenant_id: user.tenant_id,
            metadata: user.metadata,
        })
    }

    async fn authorize(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_permissions = self.request_permissions(context)?;
        if has_effective_permission(&actor_permissions, &Permission::SETTINGS_MANAGE) {
            Ok(())
        } else {
            Err(AuthAdminMutationError::Forbidden(
                "settings:manage required for OAuth application administration".to_string(),
            ))
        }
    }

    async fn authorize_delegated_permissions(
        &self,
        context: &AuthAdminMutationContext,
        requested_permissions: &[String],
    ) -> Result<(), AuthAdminMutationError> {
        let actor_permissions = self.request_permissions(context)?;
        for value in requested_permissions {
            let permission = Permission::from_str(value.trim()).map_err(|error| {
                AuthAdminMutationError::Validation(format!(
                    "invalid delegated permission `{value}`: {error}"
                ))
            })?;
            if !has_effective_permission(&actor_permissions, &permission) {
                return Err(AuthAdminMutationError::Forbidden(format!(
                    "cannot delegate permission outside the current request authority: {permission}"
                )));
            }
        }
        Ok(())
    }

    async fn localize_app(
        &self,
        context: &AuthAdminMutationContext,
        app: oauth_apps::Model,
    ) -> Result<oauth_apps::Model, AuthAdminMutationError> {
        let locale = self.effective_locale(context)?;
        oauth_apps::hydrate_exact_translation(&self.db, app, locale.as_str())
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))
    }

    async fn record(
        &self,
        context: &AuthAdminMutationContext,
        app: oauth_apps::Model,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        let app = self.localize_app(context, app).await?;
        let active_token_count = oauth_tokens::Entity::count_active_by_app(&self.db, app.id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let redirect_uris = app.redirect_uris_list();
        let scopes = app.scopes_list();
        let grant_types = app.grant_types_list();
        let granted_permissions = app.granted_permissions_list();
        let managed_by_manifest = app.managed_by_manifest();
        let is_active = app.is_active();
        let can_edit = app.can_edit();
        let can_rotate_secret = app.can_rotate_secret();
        let can_revoke = app.can_revoke();

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
            granted_permissions,
            manifest_ref: app.manifest_ref,
            auto_created: app.auto_created,
            managed_by_manifest,
            is_active,
            can_edit,
            can_rotate_secret,
            can_revoke,
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
impl OAuthAdminPort for ServerAuthAdminMutationProvider {
    async fn list_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        app_type: Option<String>,
        limit: u64,
    ) -> Result<Vec<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.authorize(context).await?;
        let mut query = oauth_apps::Entity::find()
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .filter(oauth_apps::Column::IsActive.eq(true))
            .filter(oauth_apps::Column::RevokedAt.is_null())
            .order_by_desc(oauth_apps::Column::CreatedAt)
            .limit(limit);
        if let Some(app_type) = app_type {
            query = query.filter(oauth_apps::Column::AppType.eq(app_type));
        }
        let apps = query
            .all(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let mut records = Vec::with_capacity(apps.len());
        for app in apps {
            records.push(self.record(context, app).await?);
        }
        Ok(records)
    }

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        match app {
            Some(app) => Ok(Some(self.record(context, app).await?)),
            None => Ok(None),
        }
    }

    async fn list_authorized_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<AuthorizedOAuthAppRecord>, AuthAdminMutationError> {
        let consents = oauth_consents::Entity::find()
            .filter(oauth_consents::Column::UserId.eq(context.actor_id))
            .filter(oauth_consents::Column::TenantId.eq(context.tenant_id))
            .filter(oauth_consents::Column::RevokedAt.is_null())
            .order_by_desc(oauth_consents::Column::GrantedAt)
            .limit(limit)
            .find_also_related(oauth_apps::Entity)
            .all(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let mut records = Vec::with_capacity(consents.len());
        for (consent, app) in consents {
            if let Some(app) = app.filter(|app| app.is_active()) {
                records.push(AuthorizedOAuthAppRecord {
                    app: self.record(context, app).await?,
                    scopes: consent.scopes_list(),
                    granted_at: consent.granted_at.into(),
                });
            }
        }
        Ok(records)
    }

    async fn create_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.authorize(context).await?;
        self.authorize_delegated_permissions(context, &command.granted_permissions)
            .await?;
        let locale = self.effective_locale(context)?;
        let result = oauth_apps::scope_runtime_copy_locale(
            locale,
            OAuthAppService::create_app(
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
            ),
        )
        .await
        .map_err(map_service_error)?;

        Ok(OAuthAppSecretResult {
            app: self.record(context, result.app).await?,
            client_secret: result.client_secret,
        })
    }

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.authorize(context).await?;
        self.authorize_delegated_permissions(context, &command.granted_permissions)
            .await?;
        let locale = self.effective_locale(context)?;
        let app = oauth_apps::scope_runtime_copy_locale(
            locale,
            OAuthAppService::update_app(
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
            ),
        )
        .await
        .map_err(map_service_error)?;
        self.record(context, app).await
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
            app: self.record(context, result.app).await?,
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
        self.record(context, revoked).await
    }

    async fn grant_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<(), AuthAdminMutationError> {
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .filter(|app| app.is_active())
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        OAuthAppService::grant_consent(
            &self.db,
            app.id,
            context.actor_id,
            context.tenant_id,
            scopes,
        )
        .await
        .map_err(map_service_error)
    }

    async fn revoke_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        OAuthAppService::revoke_user_consent(&self.db, app_id, context.actor_id, context.tenant_id)
            .await
            .map_err(map_service_error)
    }
}
