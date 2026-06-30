use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateOAuthAppCommand, CreateUserCommand,
    OAuthAdminMutationPort, OAuthAppMutationRecord, OAuthAppSecretResult, UpdateOAuthAppCommand,
    UpdateUserCommand, UserAdminMutationPort, UserMutationRecord,
};
use rustok_core::Permission;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::hash_password;
use crate::models::{oauth_apps, oauth_tokens, tenants, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::oauth_app::{self, OAuthAppService};
use crate::services::rbac_service::RbacService;

#[derive(Clone)]
pub struct ServerAuthAdminMutationProvider {
    db: DatabaseConnection,
}

fn parse_user_status(value: &str) -> Result<rustok_core::UserStatus, AuthAdminMutationError> {
    match value {
        "active" => Ok(rustok_core::UserStatus::Active),
        "inactive" => Ok(rustok_core::UserStatus::Inactive),
        "banned" => Ok(rustok_core::UserStatus::Banned),
        other => Err(AuthAdminMutationError::Validation(format!(
            "unsupported user status: {other}"
        ))),
    }
}

fn map_lifecycle_error(error: AuthLifecycleError) -> AuthAdminMutationError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => {
            AuthAdminMutationError::Conflict("user email already exists".to_string())
        }
        other => AuthAdminMutationError::Internal(crate::error::Error::from(other).to_string()),
    }
}

fn map_custom_field_error(
    error: rustok_core::field_schema::FlexError,
) -> AuthAdminMutationError {
    match error {
        rustok_core::field_schema::FlexError::ValidationFailed(errors) => {
            AuthAdminMutationError::CustomFieldsValidation(
                serde_json::to_value(errors).unwrap_or_else(|_| serde_json::json!([])),
            )
        }
        other => AuthAdminMutationError::Internal(other.to_string()),
    }
}

#[async_trait]
impl UserAdminMutationPort for ServerAuthAdminMutationProvider {
    async fn create_user(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_CREATE, Permission::USERS_MANAGE],
            "users:create or users:manage required",
        )
        .await?;
        let role = command
            .role
            .as_deref()
            .unwrap_or("customer")
            .parse::<rustok_core::UserRole>()
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))?;
        let status = command
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;
        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_core::PLATFORM_FALLBACK_LOCALE);
        let prepared = FlexAttachedValuesService::prepare_create(
            &self.db,
            context.tenant_id,
            "user",
            locale,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let mut user = AuthLifecycleService::create_user_db(
            &self.db,
            context.tenant_id,
            &command.email,
            &command.password,
            command.name,
            role,
            status,
        )
        .await
        .map_err(map_lifecycle_error)?;

        if let Some(metadata) = prepared.metadata {
            let mut active: users::ActiveModel = user.into();
            active.metadata = Set(metadata);
            user = active
                .update(&self.db)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        }
        if let (Some(locale), Some(values)) = (
            prepared.locale.as_deref(),
            prepared.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &self.db,
                context.tenant_id,
                "user",
                user.id,
                locale,
                values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }
        self.user_record(user).await
    }

    async fn update_user(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_UPDATE, Permission::USERS_MANAGE],
            "users:update or users:manage required",
        )
        .await?;
        let user = users::Entity::find_by_id(command.id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;

        if let Some(email) = command.email.as_deref() {
            let existing = users::Entity::find_by_email(&self.db, context.tenant_id, email)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            if existing
                .as_ref()
                .is_some_and(|existing| existing.id != user.id)
            {
                return Err(AuthAdminMutationError::Conflict(
                    "user email already exists".to_string(),
                ));
            }
        }

        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_core::PLATFORM_FALLBACK_LOCALE);
        let prepared = FlexAttachedValuesService::prepare_update(
            &self.db,
            context.tenant_id,
            "user",
            user.id,
            locale,
            &user.metadata,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let user_id = user.id;
        let mut active: users::ActiveModel = user.into();
        if let Some(email) = command.email {
            active.email = Set(email.to_lowercase());
        }
        if let Some(name) = command.name {
            active.name = Set(Some(name));
        }
        if let Some(status) = command.status.as_deref() {
            active.status = Set(parse_user_status(status)?);
        }
        if let Some(password) = command.password {
            active.password_hash = Set(hash_password(&password)
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?);
        }
        if let Some(metadata) = prepared.metadata {
            active.metadata = Set(metadata);
        }
        let requested_role = command
            .role
            .as_deref()
            .map(rustok_core::UserRole::from_str)
            .transpose()
            .map_err(map_custom_field_error)?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let user = active
            .update(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        if let Some(role) = requested_role {
            RbacService::replace_user_role(&tx, &user.id, &context.tenant_id, role)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        }
        if let (Some(locale), Some(values)) = (
            prepared.locale.as_deref(),
            prepared.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &tx,
                context.tenant_id,
                "user",
                user_id,
                locale,
                values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        self.user_record(user).await
    }

    async fn delete_user(
        &self,
        context: &AuthAdminMutationContext,
        user_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_MANAGE],
            "users:manage required",
        )
        .await?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;
        FlexAttachedValuesService::delete_localized_values(&tx, context.tenant_id, "user", user_id)
            .await
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))?;
        let active: users::ActiveModel = user.into();
        active
            .delete(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))
    }
}

impl ServerAuthAdminMutationProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn authorize_user(
        &self,
        context: &AuthAdminMutationContext,
        permissions: &[Permission],
        message: &str,
    ) -> Result<(), AuthAdminMutationError> {
        let allowed = RbacService::has_any_permission(
            &self.db,
            &context.tenant_id,
            &context.actor_id,
            permissions,
        )
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        if allowed {
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
        let tenant_name = tenants::Entity::find_by_id(user.tenant_id)
            .one(&self.db)
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
impl OAuthAdminMutationPort for ServerAuthAdminMutationProvider {
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
