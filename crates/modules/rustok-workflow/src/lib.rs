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

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migration;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;
pub mod steps;
pub mod templates;

pub use dto::*;
pub use error::{WorkflowError, WorkflowResult};
pub use graphql::{WorkflowMutation, WorkflowQuery};
pub use migration::{WorkflowPhase4Migration, WorkflowsMigration};
pub use module::WorkflowModule;
pub use ports::*;
pub use services::{
    WorkflowCronScheduler, WorkflowEngine, WorkflowService, WorkflowTriggerHandler,
};
pub use steps::{AlloyScriptStep, NotificationSender, NotifyStep, ScriptRunner};
pub use templates::{BUILTIN_TEMPLATES, WorkflowTemplate};

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use rustok_core::module::RusToKModule;

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
                .any(|p| p.resource == rustok_api::Resource::Workflows
                    && p.action == rustok_api::Action::Execute)
        );
        assert!(
            perms
                .iter()
                .any(|p| p.resource == rustok_api::Resource::WorkflowExecutions
                    && p.action == rustok_api::Action::List)
        );
    }
}
