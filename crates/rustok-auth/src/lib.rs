/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod admin_mutations;
pub mod backfill;
pub mod bootstrap;
pub mod config;
pub mod credentials;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod jwt;
pub mod lifecycle;
pub mod migrations;
pub mod rest;

// Re-exports for convenience
pub use admin_mutations::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, CreateUserCommand, OAuthAdminPort, OAuthAdminRuntime,
    OAuthAppMutationRecord, OAuthAppSecretResult, UpdateOAuthAppCommand, UpdateUserCommand,
    UserAdminMutationPort, UserAdminMutationRuntime, UserMutationRecord,
};
pub use backfill::AuthUserBackfillDbReader;
pub use bootstrap::{AuthUserBootstrapDbWriter, AuthUserBootstrapRecord, AuthUserBootstrapRequest};
pub use config::{
    AuthConfig, AuthSettingsOverrides, JwtAlgorithm, build_auth_config, build_auth_config_with_env,
    validate_auth_config,
};
pub use credentials::{generate_refresh_token, hash_password, hash_refresh_token, verify_password};
pub use error::AuthError;
pub use jwt::{
    Claims, EmailVerificationClaims, InviteClaims, OauthAccessTokenInput, PasswordResetClaims,
    decode_access_token, decode_email_verification_token, decode_invite_token,
    decode_password_reset_token, encode_access_token, encode_email_verification_token,
    encode_invite_token, encode_oauth_access_token, encode_password_reset_token,
};
pub use lifecycle::{
    AcceptInviteRecord, AuthLifecycleContext, AuthLifecycleMutationError, AuthLifecyclePort,
    AuthLifecycleRuntime, AuthSessionRecord, AuthTokenRecord, AuthUserBackfillReadPort,
    AuthUserBackfillReadRequest, AuthUserBackfillRecord, AuthUserBackfillRuntime, AuthUserRecord,
};
pub use rest::{
    AcceptInviteParams, AuthResponse, AuthorizeRequest, BrowserAuthorizeRequest,
    BrowserSessionResponse, ChangePasswordParams, ConfirmResetParams, ConfirmVerificationParams,
    ConsentRequest, GenericStatusResponse, InviteAcceptResponse, LoginParams, LogoutResponse,
    RefreshRequest, RegisterParams, RequestResetParams, RequestVerificationParams,
    ResetRequestResponse, RevokeRequest, SessionItem, SessionListParams, SessionsResponse,
    TokenErrorResponse, TokenRequest, TokenResponse, UpdateProfileParams, UserInfo, UserItem,
    UserResponse, UsersListParams, UsersResponse, VerificationRequestResponse,
};

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};

/// Canonical auth-owned RBAC surface published by the module.
///
/// Keep this list in sync with server integration tests and docs whenever
/// the `users:*` runtime contract changes.
pub const AUTH_USER_PERMISSIONS: [Permission; 6] = [
    Permission::USERS_CREATE,
    Permission::USERS_READ,
    Permission::USERS_UPDATE,
    Permission::USERS_DELETE,
    Permission::USERS_LIST,
    Permission::USERS_MANAGE,
];
use sea_orm_migration::MigrationTrait;

/// Core auth module — JWT lifecycle, credential hashing, token management.
///
/// Pure logic module with no framework dependencies. The server constructs
/// `AuthConfig` from its config source and passes it to the auth functions.
pub struct AuthModule;

impl MigrationSource for AuthModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for AuthModule {
    fn slug(&self) -> &'static str {
        "auth"
    }

    fn name(&self) -> &'static str {
        "Auth"
    }

    fn description(&self) -> &'static str {
        "JWT lifecycle, credential hashing, token management."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::USERS_CREATE,
            Permission::USERS_READ,
            Permission::USERS_UPDATE,
            Permission::USERS_DELETE,
            Permission::USERS_LIST,
            Permission::USERS_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::{AUTH_USER_PERMISSIONS, AuthModule};
    use rustok_api::Permission;
    use rustok_core::module::{ModuleKind, RusToKModule};

    #[test]
    fn auth_module_publishes_exact_users_permission_surface() {
        let module = AuthModule;

        assert_eq!(
            module.permissions(),
            vec![
                Permission::USERS_CREATE,
                Permission::USERS_READ,
                Permission::USERS_UPDATE,
                Permission::USERS_DELETE,
                Permission::USERS_LIST,
                Permission::USERS_MANAGE,
            ]
        );
        assert_eq!(module.permissions(), AUTH_USER_PERMISSIONS.to_vec());
    }

    #[test]
    fn auth_module_contract_stays_core_capability_only() {
        let module = AuthModule;

        assert_eq!(module.slug(), "auth");
        assert_eq!(module.kind(), ModuleKind::Core);
        assert!(module.dependencies().is_empty());
    }
}
