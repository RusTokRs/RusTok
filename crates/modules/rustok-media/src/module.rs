use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Media asset management module for RusToK.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaModule;

impl MediaModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for MediaModule {
    fn slug(&self) -> &'static str {
        "media"
    }

    fn name(&self) -> &'static str {
        "Media"
    }

    fn description(&self) -> &'static str {
        "Media library, uploads and localized asset metadata"
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
        &["outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Media, Action::Create),
            Permission::new(Resource::Media, Action::Read),
            Permission::new(Resource::Media, Action::Update),
            Permission::new(Resource::Media, Action::Delete),
            Permission::new(Resource::Media, Action::List),
            Permission::new(Resource::Media, Action::Manage),
        ]
    }
}

impl MigrationSource for MediaModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        crate::migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::MediaModule;
    use rustok_core::module::{HealthStatus, ModuleKind, RusToKModule};

    #[tokio::test]
    async fn media_module_metadata() {
        let module = MediaModule::new();
        assert_eq!(module.slug(), "media");
        assert_eq!(module.name(), "Media");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.dependencies(), &["outbox"]);
        assert!(!module.permissions().is_empty());
    }
}
