pub mod config;
pub mod error;
pub mod service;
pub mod template;

pub use config::{EmailConfig, SmtpConfig};
pub use error::EmailError;
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
        HealthStatus::Healthy
    }
}
