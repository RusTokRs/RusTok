use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Newsletter module — subscriber management, campaign lifecycle, delivery.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NewsletterModule;

impl NewsletterModule {
    pub fn new() -> Self {
        Self
    }
}

impl MigrationSource for NewsletterModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }
}

#[async_trait]
impl RusToKModule for NewsletterModule {
    fn slug(&self) -> &'static str {
        "newsletter"
    }

    fn name(&self) -> &'static str {
        "Newsletter"
    }

    fn description(&self) -> &'static str {
        "Subscriber management, campaign lifecycle, content aggregation, and email delivery orchestration"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["email", "outbox"]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::NewsletterModule;
    use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};

    #[tokio::test]
    async fn newsletter_module_metadata_and_health() {
        let module = NewsletterModule;
        assert_eq!(module.slug(), "newsletter");
        assert_eq!(module.name(), "Newsletter");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["email", "outbox"]);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert!(!module.migrations().is_empty());
    }
}
