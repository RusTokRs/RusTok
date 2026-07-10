use axum::routing::{get, post, put};
use rustok_api::HostRuntimeContext;
use sea_orm::DatabaseConnection;

pub mod executions;
pub mod steps;
pub mod webhook;
pub mod workflows;

#[derive(Clone)]
pub struct WorkflowHttpRuntime {
    db: DatabaseConnection,
}

impl WorkflowHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }
}

impl WorkflowHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> Self {
        Self {
            db: runtime.db_clone(),
        }
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = WorkflowHttpRuntime::from_host(runtime);
    Ok(axum::Router::new()
        .route(
            "/api/workflows/",
            get(workflows::list).post(workflows::create),
        )
        .route(
            "/api/workflows/{id}",
            get(workflows::get)
                .put(workflows::update)
                .delete(workflows::delete_workflow),
        )
        .route("/api/workflows/{id}/activate", post(workflows::activate))
        .route("/api/workflows/{id}/pause", post(workflows::pause))
        .route(
            "/api/workflows/{id}/trigger",
            post(workflows::trigger_manual),
        )
        .route("/api/workflows/{id}/steps", post(steps::add_step))
        .route(
            "/api/workflows/{id}/steps/{step_id}",
            put(steps::update_step).delete(steps::delete_step),
        )
        .route(
            "/api/workflows/{id}/executions",
            get(executions::list_executions),
        )
        .route(
            "/api/workflows/executions/{execution_id}",
            get(executions::get_execution),
        )
        .with_state(state))
}

pub fn axum_webhook_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = WorkflowHttpRuntime::from_host(runtime);
    Ok(axum::Router::new()
        .route(
            "/webhooks/{tenant_slug}/{webhook_slug}",
            post(webhook::receive),
        )
        .with_state(state))
}
