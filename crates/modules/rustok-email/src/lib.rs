pub mod config;
pub mod error;
pub mod ports;
pub mod service;
pub mod template;

pub use config::{EmailConfig, SmtpConfig};
pub use error::EmailError;
pub use ports::*;
pub use service::{
    EmailService, PasswordResetEmail, PasswordResetEmailSender, SmtpEmailSender,
    TransactionalEmailSender,
};
pub use template::{EmailTemplateProvider, RenderedEmail};

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Core email module — SMTP transport, templates, email lifecycle.
pub struct EmailModule;

impl MigrationSource for EmailModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for EmailModule {
    fn slug(&self) -> &'static str {
        "email"
    }

    fn name(&self) -> &'static str {
        "Email"
    }

    fn description(&self) -> &'static str {
        "SMTP transport, email templates, delivery lifecycle."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        // Module-level health has no host AppContext, so it cannot validate
        // the effective SMTP transport. The server readiness layer owns
        // the concrete `email_backend` check and metrics.
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::EmailModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn email_module_health_defers_to_host_readiness() {
        assert_eq!(EmailModule.health().await, HealthStatus::Degraded);
    }
}
