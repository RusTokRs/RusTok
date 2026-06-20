/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod config;
pub mod credentials;
pub mod error;
pub mod jwt;
pub mod migrations;

// Re-exports for convenience
pub use config::{AuthConfig, AuthSettingsOverrides, JwtAlgorithm};
pub use credentials::{generate_refresh_token, hash_password, hash_refresh_token, verify_password};
pub use error::AuthError;
pub use jwt::{
    decode_access_token, decode_email_verification_token, decode_invite_token,
    decode_password_reset_token, encode_access_token, encode_email_verification_token,
    encode_oauth_access_token, encode_password_reset_token, Claims, EmailVerificationClaims,
    InviteClaims, OauthAccessTokenInput, PasswordResetClaims,
};

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::permissions::Permission;

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
        AUTH_USER_PERMISSIONS.to_vec()
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthModule, AUTH_USER_PERMISSIONS};
    use rustok_core::module::{ModuleKind, RusToKModule};
    use rustok_core::Permission;

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
        assert!(module.ui_extensions().is_empty());
    }
}
