use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod entity;
#[cfg(feature = "loco-adapter")]
pub mod loco;
pub mod migration;
pub mod ports;
pub mod relay;
pub mod transactional;
pub mod transport;

pub use entity::{Entity as SysEvents, Model as SysEvent};
pub use migration::SysEventsMigration;
pub use ports::*;
pub use relay::{OutboxRelay, RelayConfig, RelayMetricsSnapshot};
pub use transactional::TransactionalEventBus;
pub use transport::OutboxTransport;

/// Core outbox module — transactional event persistence and relay infrastructure.
pub struct OutboxModule;

impl MigrationSource for OutboxModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(SysEventsMigration)]
    }
}

#[async_trait]
impl RusToKModule for OutboxModule {
    fn slug(&self) -> &'static str {
        "outbox"
    }

    fn name(&self) -> &'static str {
        "Outbox"
    }

    fn description(&self) -> &'static str {
        "Transactional event persistence, relay, retry, and DLQ lifecycle."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        // Module-level health has no host AppContext, so it cannot inspect
        // sys_events, relay worker state, backlog, lag or DLQ. The server
        // readiness layer owns those concrete checks.
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod health_tests {
    use super::OutboxModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn outbox_module_health_defers_to_host_readiness() {
        assert_eq!(OutboxModule.health().await, HealthStatus::Degraded);
    }
}
