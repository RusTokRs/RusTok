use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Newsletter module — subscriber management, campaign lifecycle, delivery.
pub struct NewsletterModule;

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

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["email", "outbox"]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
