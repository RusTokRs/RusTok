use async_trait::async_trait;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod entities;
pub mod error;
pub mod migrations;
pub mod model;
pub mod ports;
pub mod service;

pub use error::{SocialGraphError, SocialGraphResult};
pub use model::SocialRelationKind;
pub use ports::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphPairRequest,
    SocialGraphPrivacyReadPort, SocialGraphPrivacyRuntime,
};
pub use service::SocialGraphService;

pub struct SocialGraphModule;

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
}

impl MigrationSource for SocialGraphModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::{MigrationSource, RusToKModule};

    use super::SocialGraphModule;

    #[test]
    fn module_metadata_and_migrations_are_stable() {
        let module = SocialGraphModule;
        assert_eq!(module.slug(), "social_graph");
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 1);
        assert_eq!(module.migration_dependencies().len(), 1);
    }
}
