use axum::routing::{delete, get, post, put};
use loco_rs::controller::Routes;

pub mod executions;
pub mod steps;
pub mod webhook;
pub mod workflows;

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/workflows")
        // Workflow CRUD
        .add("/", get(workflows::list).post(workflows::create))
        .add("/:id", get(workflows::get).put(workflows::update).delete(workflows::delete_workflow))
        .add("/:id/activate", post(workflows::activate))
        .add("/:id/pause", post(workflows::pause))
        // Manual trigger
        .add("/:id/trigger", post(workflows::trigger_manual))
        // Step management
        .add("/:id/steps", post(steps::add_step))
        .add("/:id/steps/:step_id", put(steps::update_step).delete(steps::delete_step))
        // Execution history
        .add("/:id/executions", get(executions::list_executions))
        .add("/executions/:execution_id", get(executions::get_execution))
}

/// Public routes (no auth) for incoming external webhooks.
pub fn webhook_routes() -> Routes {
    Routes::new()
        .prefix("webhooks")
        .add("/:tenant_slug/:webhook_slug", post(webhook::receive))
}
