/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthAdminMutationContext {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
    pub request_id: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CreateUserCommand {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateUserCommand {
    pub id: Uuid,
    pub email: Option<String>,
    pub password: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserMutationRecord {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub tenant_name: Option<String>,
    pub tenant_id: Uuid,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOAuthAppCommand {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub app_type: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOAuthAppCommand {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthAppMutationRecord {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub app_type: String,
    pub client_id: Uuid,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub manifest_ref: Option<String>,
    pub auto_created: bool,
    pub managed_by_manifest: bool,
    pub is_active: bool,
    pub can_edit: bool,
    pub can_rotate_secret: bool,
    pub can_revoke: bool,
    pub active_token_count: i64,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthorizedOAuthAppRecord {
    pub app: OAuthAppMutationRecord,
    pub scopes: Vec<String>,
    pub granted_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthAppSecretResult {
    pub app: OAuthAppMutationRecord,
    pub client_secret: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AuthAdminMutationError {
    #[error("authentication required")]
    Unauthorized,
    #[error("permission denied: {0}")]
    Forbidden(String),
    #[error("invalid auth admin mutation: {0}")]
    Validation(String),
    #[error("custom field validation failed: {0}")]
    CustomFieldsValidation(serde_json::Value),
    #[error("auth admin resource not found: {0}")]
    NotFound(String),
    #[error("auth admin mutation conflict: {0}")]
    Conflict(String),
    #[error("auth admin mutation failed: {0}")]
    Internal(String),
}

#[async_trait]
pub trait UserAdminMutationPort: Send + Sync {
    async fn create_user(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError>;

    async fn update_user(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError>;

    async fn delete_user(
        &self,
        context: &AuthAdminMutationContext,
        user_id: Uuid,
    ) -> Result<(), AuthAdminMutationError>;
}

#[async_trait]
pub trait OAuthAdminPort: Send + Sync {
    async fn list_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        app_type: Option<String>,
        limit: u64,
    ) -> Result<Vec<OAuthAppMutationRecord>, AuthAdminMutationError>;

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError>;

    async fn list_authorized_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<AuthorizedOAuthAppRecord>, AuthAdminMutationError>;

    async fn create_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError>;

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError>;

    async fn rotate_oauth_app_secret(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError>;

    async fn revoke_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError>;

    async fn grant_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<(), AuthAdminMutationError>;

    async fn revoke_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError>;
}

#[derive(Clone)]
pub struct UserAdminMutationRuntime {
    port: Arc<dyn UserAdminMutationPort>,
}

impl UserAdminMutationRuntime {
    pub fn new(port: Arc<dyn UserAdminMutationPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn UserAdminMutationPort {
        self.port.as_ref()
    }
}

#[derive(Clone)]
pub struct OAuthAdminRuntime {
    port: Arc<dyn OAuthAdminPort>,
}

impl OAuthAdminRuntime {
    pub fn new(port: Arc<dyn OAuthAdminPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn OAuthAdminPort {
        self.port.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_errors_have_stable_operator_safe_categories() {
        assert_eq!(
            AuthAdminMutationError::Forbidden("users:create required".into()).to_string(),
            "permission denied: users:create required"
        );
        assert_eq!(
            AuthAdminMutationError::NotFound("oauth app".into()).to_string(),
            "auth admin resource not found: oauth app"
        );
    }
}
