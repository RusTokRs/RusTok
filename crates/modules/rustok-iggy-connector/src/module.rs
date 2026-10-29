use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Native module definition for Iggy Connector.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IggyConnectorModule;

impl MigrationSource for IggyConnectorModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        #[cfg(feature = "migrations")]
        {
            crate::migrations::migrations()
        }
        #[cfg(not(feature = "migrations"))]
        {
            Vec::new()
        }
    }
}

#[async_trait]
impl RusToKModule for IggyConnectorModule {
    fn slug(&self) -> &'static str {
        "iggy_connector"
    }

    fn name(&self) -> &'static str {
        "Iggy Connector"
    }

    fn description(&self) -> &'static str {
        "Selectable bundled or external Iggy connectivity for event delivery."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
