use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::{ModuleEventListenerContext, ModuleEventListenerRegistry};
use sea_orm_migration::MigrationTrait;

use crate::migrations;
use crate::services::WorkflowTriggerHandler;

/// Visual workflow automation and event orchestration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkflowModule;

impl WorkflowModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for WorkflowModule {
    fn slug(&self) -> &'static str {
        "workflow"
    }

    fn name(&self) -> &'static str {
        "Workflow"
    }

    fn description(&self) -> &'static str {
        "Visual workflow automation and event orchestration"
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
            Permission::new(Resource::Workflows, Action::Create),
            Permission::new(Resource::Workflows, Action::Read),
            Permission::new(Resource::Workflows, Action::Update),
            Permission::new(Resource::Workflows, Action::Delete),
            Permission::new(Resource::Workflows, Action::List),
            Permission::new(Resource::Workflows, Action::Execute),
            Permission::new(Resource::Workflows, Action::Manage),
            Permission::new(Resource::WorkflowExecutions, Action::Read),
            Permission::new(Resource::WorkflowExecutions, Action::List),
        ]
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(WorkflowTriggerHandler::new(ctx.db.clone()));
    }
}

impl MigrationSource for WorkflowModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn workflow_module_metadata_and_traits() {
        let module = WorkflowModule::new();
        assert_eq!(module.slug(), "workflow");
        assert_eq!(module.name(), "Workflow");
        assert_eq!(
            module.description(),
            "Visual workflow automation and event orchestration"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.permissions().len(), 9);
        assert!(!module.migrations().is_empty());
    }
}
