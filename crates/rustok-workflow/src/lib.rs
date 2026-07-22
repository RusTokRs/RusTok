//! Workflow module for RusToK platform
//!
//! Provides visual workflow automation — an n8n/Directus Flows-style
//! orchestrator that integrates with the platform's event infrastructure.
//!
//! # Architecture
//!
//! Workflows are triggered by domain events, cron schedules, webhooks, or
//! manually. Each workflow is a linear chain of steps (Phase 1). Steps can
//! perform actions, evaluate conditions, emit new domain events, etc.
//!
//! # Phase 1 (this implementation)
//!
//! - Data model: `workflows`, `workflow_steps`, `workflow_executions`,
//!   `workflow_step_executions`
//! - CRUD API via `WorkflowService`
//! - Linear step execution via `WorkflowEngine`
//! - Event trigger via `WorkflowTriggerHandler`
//! - Basic steps: `action`, `emit_event`, `condition`

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migration;
pub mod migrations;
pub mod ports;
pub mod services;
pub mod steps;
pub mod templates;

pub use dto::*;
pub use error::{WorkflowError, WorkflowResult};
pub use graphql::{WorkflowMutation, WorkflowQuery};
pub use migration::{WorkflowPhase4Migration, WorkflowsMigration};
pub use ports::*;
pub use services::{
    WorkflowCronScheduler, WorkflowEngine, WorkflowService, WorkflowTriggerHandler,
};
pub use steps::{AlloyScriptStep, NotificationSender, NotifyStep, ScriptRunner};
pub use templates::{BUILTIN_TEMPLATES, WorkflowTemplate};

pub struct WorkflowModule;

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

    #[test]
    fn module_metadata() {
        let module = WorkflowModule;
        assert_eq!(module.slug(), "workflow");
        assert_eq!(module.name(), "Workflow");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn module_permissions() {
        let module = WorkflowModule;
        let perms = module.permissions();
        assert!(
            perms
                .iter()
                .any(|p| p.resource == Resource::Workflows && p.action == Action::Execute)
        );
        assert!(
            perms
                .iter()
                .any(|p| p.resource == Resource::WorkflowExecutions && p.action == Action::List)
        );
    }
}
