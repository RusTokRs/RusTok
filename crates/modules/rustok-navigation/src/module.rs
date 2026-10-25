use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Localized navigation menus and channel slot bindings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NavigationModule;

impl NavigationModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for NavigationModule {
    fn slug(&self) -> &'static str {
        "navigation"
    }

    fn name(&self) -> &'static str {
        "Navigation"
    }

    fn description(&self) -> &'static str {
        "Localized navigation menus and current-channel slot bindings"
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
        &["channel", "outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Navigation, Action::Create),
            Permission::new(Resource::Navigation, Action::Read),
            Permission::new(Resource::Navigation, Action::Update),
            Permission::new(Resource::Navigation, Action::Delete),
            Permission::new(Resource::Navigation, Action::List),
            Permission::new(Resource::Navigation, Action::Manage),
        ]
    }
}

impl MigrationSource for NavigationModule {
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
    async fn navigation_module_metadata_and_traits() {
        let module = NavigationModule::new();
        assert_eq!(module.slug(), "navigation");
        assert_eq!(module.name(), "Navigation");
        assert_eq!(
            module.description(),
            "Localized navigation menus and current-channel slot bindings"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.dependencies(), &["channel", "outbox"]);
        assert_eq!(module.permissions().len(), 6);
        assert!(!module.migrations().is_empty());
    }

    #[test]
    fn navigation_module_permissions_cover_lifecycle() {
        let module = NavigationModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Navigation, Action::Manage)));
    }
}
