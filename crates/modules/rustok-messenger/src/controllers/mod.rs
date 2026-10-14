use axum::{
    Router,
    http::StatusCode,
    routing::{get, post, put},
};
use rustok_api::HostRuntimeContext;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::MessengerService;

pub mod rest;

/// Shared HTTP runtime state for the messenger module.
#[derive(Clone)]
pub struct MessengerHttpRuntime {
    pub db: DatabaseConnection,
    pub service: MessengerService,
}

impl MessengerHttpRuntime {
    pub fn new(db: DatabaseConnection, worker_id: i64) -> Self {
        let service = MessengerService::new(db.clone(), worker_id);
        Self { db, service }
    }

    pub fn from_host(runtime: &HostRuntimeContext) -> Self {
        Self::new(runtime.db_clone(), 1)
    }
}

/// Enforces that the messenger module is actively enabled for the requesting tenant.
pub async fn ensure_messenger_module_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> HttpResult<()> {
    match rustok_api::is_tenant_module_enabled(db, tenant_id, "messenger").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpError::new(
            StatusCode::FORBIDDEN,
            "MODULE_NOT_ENABLED",
            "Messenger module is not enabled for this tenant",
        )),
        Err(err) => {
            tracing::error!("Failed to check messenger module status: {err}");
            Err(HttpError::internal("Failed to verify module enablement"))
        }
    }
}

/// Build Axum Router for messenger endpoints.
pub fn router(runtime: MessengerHttpRuntime) -> Router {
    Router::new()
        .route("/conversations", post(rest::create_conversation))
        .route("/conversations", get(rest::list_conversations))
        .route("/conversations/{id}", get(rest::get_conversation))
        .route(
            "/channels/{id}/translations",
            put(rest::update_channel_translation),
        )
        .route("/messages", post(rest::send_message))
        .route(
            "/conversations/{conversation_id}/messages",
            get(rest::list_messages),
        )
        .route(
            "/conversations/{conversation_id}/sync",
            get(rest::fast_sync),
        )
        .route(
            "/conversations/{conversation_id}/cursor",
            post(rest::update_cursor),
        )
        .with_state(runtime)
}
