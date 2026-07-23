//! Runtime registration adapter for the `rustok-events` capability.
//!
//! Event contracts and transport-facing types remain owned by
//! [`rustok_events`]. This small adapter exists because `rustok-core` already
//! consumes those contracts; keeping the runtime implementation in a separate
//! crate preserves the dependency direction and avoids a cycle.

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Core runtime module for the platform event capability.
pub struct EventsModule;

impl MigrationSource for EventsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for EventsModule {
    fn slug(&self) -> &'static str {
        "events"
    }

    fn name(&self) -> &'static str {
        "Events"
    }

    fn description(&self) -> &'static str {
        "Platform event contracts, delivery profile control, and transport orchestration."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    async fn health(&self) -> HealthStatus {
        // Concrete readiness is reported by the server event runtime because
        // this module-level hook has no host runtime context.
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::EventsModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn events_module_defers_runtime_health_to_host() {
        assert_eq!(EventsModule.health().await, HealthStatus::Degraded);
    }
}
