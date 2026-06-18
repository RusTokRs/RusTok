use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::storage::ScriptRegistry;

use super::handlers::{self, AppState};

pub fn create_router<S: ScriptRegistry + 'static>(state: Arc<AppState<S>>) -> Router {
    Router::new()
        .route("/scripts", get(handlers::list_scripts::<S>))
        .route("/executions", get(handlers::list_recent_executions::<S>))
        .route("/scripts", post(handlers::create_script::<S>))
        .route("/scripts/validate", post(handlers::validate_script::<S>))
        .route("/scripts/{id}", get(handlers::get_script::<S>))
        .route("/scripts/{id}", put(handlers::update_script::<S>))
        .route("/scripts/{id}", delete(handlers::delete_script::<S>))
        .route("/scripts/{id}/run", post(handlers::run_script::<S>))
        .route(
            "/scripts/{id}/executions",
            get(handlers::list_script_executions::<S>),
        )
        .route(
            "/scripts/name/{name}/run",
            post(handlers::run_script_by_name::<S>),
        )
        .with_state(state)
}
