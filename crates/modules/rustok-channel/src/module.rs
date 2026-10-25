use async_trait::async_trait;
use rustok_core::{
    ModuleRuntimeExtensions,
    module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule},
};
use sea_orm_migration::MigrationTrait;

/// Typed marker proving that `ChannelModule` participated in runtime extension registration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelRuntimeSelected;

/// Channel module descriptor implementing the RusToK module system.
pub struct ChannelModule;

impl MigrationSource for ChannelModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        crate::migrations::migration_dependencies()
    }
}

#[async_trait]
impl RusToKModule for ChannelModule {
    fn slug(&self) -> &'static str {
        "channel"
    }

    fn name(&self) -> &'static str {
        "Channel"
    }

    fn description(&self) -> &'static str {
        "Experimental core channel-management context for external delivery surfaces."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.insert(ChannelRuntimeSelected);
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn channel_module_health_is_healthy() {
        assert_eq!(ChannelModule.health().await, HealthStatus::Healthy);
    }
}
