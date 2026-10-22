use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Form submission intake, abuse controls and lead triage inbox.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormsModule;

impl FormsModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for FormsModule {
    fn slug(&self) -> &'static str {
        "forms"
    }

    fn name(&self) -> &'static str {
        "Forms"
    }

    fn description(&self) -> &'static str {
        "Form submission intake, abuse controls and lead triage inbox"
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
        &["outbox", "email"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Forms, Action::Read),
            Permission::new(Resource::Forms, Action::List),
            Permission::new(Resource::Forms, Action::Update),
            Permission::new(Resource::Forms, Action::Manage),
        ]
    }
}

impl MigrationSource for FormsModule {
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
    async fn forms_module_metadata_and_traits() {
        let module = FormsModule::new();
        assert_eq!(module.slug(), "forms");
        assert_eq!(module.name(), "Forms");
        assert_eq!(
            module.description(),
            "Form submission intake, abuse controls and lead triage inbox"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.dependencies(), &["outbox", "email"]);
        assert_eq!(module.permissions().len(), 4);
        assert!(!module.migrations().is_empty());
    }

    #[test]
    fn forms_module_permissions_cover_lifecycle() {
        let module = FormsModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Forms, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Forms, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Forms, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Forms, Action::Manage)));
    }
}
