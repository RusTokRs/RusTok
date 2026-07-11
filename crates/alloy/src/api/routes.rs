use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::storage::ScriptRegistry;

use super::handlers::{self, AppState};

pub const AXUM_EXECUTION_HISTORY_ROUTES: &[&str] = &["/executions", "/scripts/{id}/executions"];

pub fn create_router<S: ScriptRegistry + 'static>(state: Arc<AppState<S>>) -> Router {
    Router::new()
        .route("/scripts", get(handlers::list_scripts::<S>))
        .route(
            AXUM_EXECUTION_HISTORY_ROUTES[0],
            get(handlers::list_recent_executions::<S>),
        )
        .route("/scripts", post(handlers::create_script::<S>))
        .route("/scripts/validate", post(handlers::validate_script::<S>))
        .route("/scripts/{id}", get(handlers::get_script::<S>))
        .route("/scripts/{id}", put(handlers::update_script::<S>))
        .route("/scripts/{id}", delete(handlers::delete_script::<S>))
        .route("/scripts/{id}/run", post(handlers::run_script::<S>))
        .route(
            AXUM_EXECUTION_HISTORY_ROUTES[1],
            get(handlers::list_script_executions::<S>),
        )
        .route(
            "/scripts/name/{name}/run",
            post(handlers::run_script_by_name::<S>),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::AXUM_EXECUTION_HISTORY_ROUTES;

    #[test]
    fn axum_execution_history_routes_match_operator_contract() {
        assert_eq!(
            AXUM_EXECUTION_HISTORY_ROUTES,
            &["/executions", "/scripts/{id}/executions"]
        );
    }
}
