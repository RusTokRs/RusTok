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
use rustok_api::AuthPrincipalKind;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthAdminMutationContext {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
    pub request_id: Option<String>,
    pub locale: Option<String>,
    /// Trusted principal classification of the caller, when the command
    /// arrives through an authenticated request boundary.
    ///
    /// `None` is reserved for host-internal compositions that invoke the
    /// administration ports without a request principal (for example RBAC
    /// runtime wiring). Operations that act on the actor's own behalf —
    /// above all OAuth consent grant/revoke/list — must require
    /// `Some(AuthPrincipalKind::DirectUser)` and fail closed for delegated
    /// (`authorization_code`) and service (`client_credentials`) tokens:
    /// an application must never consent to applications as the user.
    pub principal_kind: Option<AuthPrincipalKind>,
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

impl std::fmt::Debug for CreateUserCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreateUserCommand")
            .field("email", &self.email)
            .field("password", &"<redacted>")
            .field("name", &self.name)
            .field("role", &self.role)
            .field("status", &self.status)
            .field("custom_fields", &self.custom_fields)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UpdateUserCommand {
    pub id: Uuid,
    pub email: Option<String>,
    pub password: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
}

impl std::fmt::Debug for UpdateUserCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UpdateUserCommand")
            .field("id", &self.id)
            .field("email", &self.email)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("name", &self.name)
            .field("role", &self.role)
            .field("status", &self.status)
            .field("custom_fields", &self.custom_fields)
            .finish()
    }
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

#[derive(Clone, PartialEq)]
pub struct OAuthAppSecretResult {
    pub app: OAuthAppMutationRecord,
    pub client_secret: String,
}

impl std::fmt::Debug for OAuthAppSecretResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthAppSecretResult")
            .field("app", &self.app)
            .field("client_secret", &"<redacted>")
            .finish()
    }
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
    fn oauth_secret_result_debug_redacts_client_secret() {
        let app = OAuthAppMutationRecord {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            slug: "test".to_string(),
            description: None,
            icon_url: None,
            app_type: "confidential".to_string(),
            client_id: Uuid::new_v4(),
            redirect_uris: vec![],
            scopes: vec![],
            grant_types: vec![],
            granted_permissions: vec![],
            manifest_ref: None,
            auto_created: false,
            managed_by_manifest: false,
            is_active: true,
            can_edit: true,
            can_rotate_secret: true,
            can_revoke: true,
            active_token_count: 0,
            last_used_at: None,
            created_at: Utc::now(),
        };
        let result = OAuthAppSecretResult {
            app,
            client_secret: "super-secret-client-value".to_string(),
        };

        let rendered = format!("{result:?}");
        assert!(!rendered.contains("super-secret-client-value"));
        assert!(rendered.contains("<redacted>"));
    }

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

    #[test]
    fn user_commands_debug_redacts_passwords() {
        let create_cmd = CreateUserCommand {
            email: "user@example.com".to_string(),
            password: "super-secret-password-123".to_string(),
            name: Some("User".to_string()),
            role: Some("Admin".to_string()),
            status: Some("Active".to_string()),
            custom_fields: None,
        };
        let create_dbg = format!("{create_cmd:?}");
        assert!(!create_dbg.contains("super-secret-password-123"));
        assert!(create_dbg.contains("<redacted>"));

        let update_cmd = UpdateUserCommand {
            id: Uuid::new_v4(),
            email: None,
            password: Some("new-secret-password-456".to_string()),
            name: None,
            role: None,
            status: None,
            custom_fields: None,
        };
        let update_dbg = format!("{update_cmd:?}");
        assert!(!update_dbg.contains("new-secret-password-456"));
        assert!(update_dbg.contains("<redacted>"));
    }
}
