use async_trait::async_trait;
use rustok_core::{
    module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule},
    permissions::{Action, Permission, Resource},
};
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm_migration::MigrationTrait;

pub struct AlloyModule {
    db: Option<DatabaseConnection>,
}

impl Default for AlloyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AlloyModule {
    pub fn new() -> Self {
        Self { db: None }
    }

    pub fn with_db(db: DatabaseConnection) -> Self {
        Self { db: Some(db) }
    }
}

impl MigrationSource for AlloyModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(alloy_scripting::ScriptsMigration),
            Box::new(alloy_scripting::ScriptExecutionsMigration),
        ]
    }
}

#[async_trait]
impl RusToKModule for AlloyModule {
    fn slug(&self) -> &'static str {
        "alloy"
    }

    fn name(&self) -> &'static str {
        "Alloy Scripting"
    }

    fn description(&self) -> &'static str {
        "Rhai-based scripting engine for custom automation and validation."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Scripts, Action::Create),
            Permission::new(Resource::Scripts, Action::Read),
            Permission::new(Resource::Scripts, Action::Update),
            Permission::new(Resource::Scripts, Action::Delete),
            Permission::new(Resource::Scripts, Action::List),
            Permission::new(Resource::Scripts, Action::Manage),
        ]
    }

    async fn health(&self) -> HealthStatus {
        if let Some(ref db) = self.db {
            if alloy_scripting::storage::ScriptsEntity::find()
                .one(db)
                .await
                .is_err()
            {
                return HealthStatus::Unhealthy;
            }
        }
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = AlloyModule::new();
        assert_eq!(module.slug(), "alloy");
        assert_eq!(module.name(), "Alloy Scripting");
        assert_eq!(module.kind(), ModuleKind::Optional);
    }

    #[test]
    fn module_permissions() {
        let module = AlloyModule::new();
        let permissions = module.permissions();
        assert!(permissions
            .iter()
            .any(|p| p.resource == Resource::Scripts && p.action == Action::Create));
        assert!(permissions
            .iter()
            .any(|p| p.resource == Resource::Scripts && p.action == Action::Manage));
    }

    #[test]
    fn module_has_migrations() {
        let module = AlloyModule::new();
        assert!(!module.migrations().is_empty());
    }
}
