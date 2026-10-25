use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::{MigrationDependencyDescriptor, ModuleRuntimeExtensions};
use sea_orm_migration::MigrationTrait;

#[cfg(feature = "index")]
use crate::index;
#[cfg(feature = "index")]
use crate::index_source;
use crate::migrations;

/// Tenant-scoped social relation owner for blocks, mutes, follows, and friendship policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SocialGraphModule;

impl SocialGraphModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for SocialGraphModule {
    fn slug(&self) -> &'static str {
        "social_graph"
    }

    fn name(&self) -> &'static str {
        "Social Graph"
    }

    fn description(&self) -> &'static str {
        "Tenant-scoped social relation owner for blocks, mutes, follows, and friendship policy"
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

    fn dependencies(&self) -> &[&'static str] {
        #[cfg(feature = "index")]
        {
            &["index", "outbox"]
        }
        #[cfg(not(feature = "index"))]
        {
            &["outbox"]
        }
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        #[cfg(feature = "index")]
        {
            let schema = index::social_graph_relation_index_schema().map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "Social Graph Index schema construction failed: {error}"
                ))
            })?;
            rustok_index::register_index_schema_source(extensions, self.slug(), schema).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "Social Graph Index schema source registration failed: {error}"
                    ))
                },
            )?;
            index_source::register_social_graph_index_source_contracts(extensions).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "Social Graph Index source contract registration failed: {error}"
                    ))
                },
            )?;
        }
        #[cfg(not(feature = "index"))]
        let _ = extensions;
        Ok(())
    }
}

impl MigrationSource for SocialGraphModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn social_graph_module_metadata_and_traits() {
        let module = SocialGraphModule::new();
        assert_eq!(module.slug(), "social_graph");
        assert_eq!(module.name(), "Social Graph");
        assert_eq!(
            module.description(),
            "Tenant-scoped social relation owner for blocks, mutes, follows, and friendship policy"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        #[cfg(feature = "index")]
        assert_eq!(module.dependencies(), &["index", "outbox"]);
        #[cfg(not(feature = "index"))]
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 4);
        assert_eq!(module.migration_dependencies().len(), 4);
    }
}
