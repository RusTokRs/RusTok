use axum::routing::{get, post, put};
use rustok_api::{HostRuntimeContext, SharedModuleEffectivePolicyReader};
use rustok_modules::ModuleEffectivePolicyReader;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;
use sea_orm::DatabaseConnection;

pub mod executions;
pub mod steps;
pub mod webhook;
pub mod workflows;

#[derive(Clone)]
pub struct WorkflowHttpRuntime {
    db: DatabaseConnection,
    effective_policy_reader: Option<SharedModuleEffectivePolicyReader>,
}

impl WorkflowHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }
}

    pub(crate) async fn ensure_module_enabled(&self, tenant_id: Uuid) -> HttpResult<()> {
        let Some(reader) = &self.effective_policy_reader else {
            return Err(HttpError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "MODULE_POLICY_UNAVAILABLE",
                "Workflow availability policy is unavailable",
            ));
        };
        let policy = reader.0.resolve(tenant_id).await.map_err(|_| {
            HttpError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "MODULE_POLICY_UNAVAILABLE",
                "Workflow availability policy is unavailable",
            )
        })?;
        let enabled = policy
            .decisions
            .iter()
            .find(|decision| decision.module_slug == "workflow")
            .is_some_and(|decision| decision.enabled);
        if enabled {
            Ok(())
        } else {
            Err(HttpError::new(
                axum::http::StatusCode::FORBIDDEN,
                "MODULE_NOT_ENABLED",
                "Module 'workflow' is not available for this tenant",
            ))
        }
    }

impl WorkflowHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let effective_policy_reader = runtime.shared_get::<SharedModuleEffectivePolicyReader>();
        Ok(Self {
            db: runtime.db_clone(),
            effective_policy_reader,
        })
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = WorkflowHttpRuntime::from_host(runtime)?;
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
