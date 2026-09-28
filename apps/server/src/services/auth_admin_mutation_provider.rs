use async_trait::async_trait;
use rustok_api::{Permission, has_any_effective_permission, has_effective_permission};
use rustok_core::UserRole;
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

fn internal_admin_error<E>(error: E) -> AuthAdminMutationError
where
    E: std::fmt::Display,
{
    tracing::error!(
        error = %error,
        "Auth administration operation failed"
    );
    AuthAdminMutationError::Internal("Auth administration operation failed".to_string())
}

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

    fn user_record(
        user: users::Model,
        role: UserRole,
        tenant_name: Option<String>,
    ) -> UserMutationRecord {
        UserMutationRecord {
            id: user.id,
            email: user.email,
            name: user.name,
            role: role.to_string(),
            status: user.status.to_string(),
            created_at: user.created_at.with_timezone(&chrono::Utc),
            tenant_name,
            tenant_id: user.tenant_id,
            metadata: user.metadata,
        }
    }

    async fn tenant_name<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
    ) -> Result<Option<String>, AuthAdminMutationError>
    where
        C: sea_orm::ConnectionTrait,
    {
        tenants::Entity::find_by_id(tenant_id)
            .one(db)
            .await
            .map_err(internal_admin_error)
            .map(|tenant| tenant.map(|value| value.name))
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