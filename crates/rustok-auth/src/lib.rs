pub mod config;
pub mod credentials;
pub mod error;
pub mod jwt;
pub mod migrations;

// Re-exports for convenience
pub use config::{AuthConfig, AuthSettingsOverrides};
pub use credentials::{generate_refresh_token, hash_password, hash_refresh_token, verify_password};
pub use error::AuthError;
pub use jwt::{
    decode_access_token, decode_email_verification_token, decode_invite_token,
    decode_password_reset_token, encode_access_token, encode_email_verification_token,
    encode_oauth_access_token, encode_password_reset_token, Claims, EmailVerificationClaims,
    InviteClaims, PasswordResetClaims,
};

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
