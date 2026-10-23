use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Standalone FBA-first visual page builder reference module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageBuilderModule;

impl PageBuilderModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for PageBuilderModule {
    fn slug(&self) -> &'static str {
        "page_builder"
    }

    fn name(&self) -> &'static str {
        "Page Builder"
    }

    fn description(&self) -> &'static str {
        "Standalone FBA-first visual page builder reference module"
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

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ]
    }
}

impl MigrationSource for PageBuilderModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn page_builder_module_metadata_and_traits() {
        let module = PageBuilderModule::new();
        assert_eq!(module.slug(), "page_builder");
        assert_eq!(module.name(), "Page Builder");
        assert_eq!(
            module.description(),
            "Standalone FBA-first visual page builder reference module"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.permissions().len(), 6);
        assert!(module.migrations().is_empty());
    }

    #[test]
    fn page_builder_module_permissions_cover_lifecycle() {
        let module = PageBuilderModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Publish)));
        assert!(permissions.contains(&Permission::new(Resource::Pages, Action::Manage)));
    }
}
