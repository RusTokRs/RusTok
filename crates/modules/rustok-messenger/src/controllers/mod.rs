use axum::{
    Router,
    http::StatusCode,
    routing::{delete, get, patch, post, put},
};
use rustok_api::HostRuntimeContext;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::services::MessengerService;

pub mod realtime;
pub mod rest;

pub use realtime::{MessengerClientMessage, MessengerRoomHub, MessengerServerMessage};

/// Shared HTTP runtime state for the messenger module.
#[derive(Clone)]
pub struct MessengerHttpRuntime {
    pub db: DatabaseConnection,
    pub service: MessengerService,
    pub room_hub: Arc<MessengerRoomHub>,
}

impl MessengerHttpRuntime {
    pub fn new(db: DatabaseConnection, worker_id: i64) -> Self {
        let service = MessengerService::new(db.clone(), worker_id);
        let room_hub = Arc::new(MessengerRoomHub::new());
        Self {
            db,
            service,
            room_hub,
        }
    }

    pub fn from_host(runtime: &HostRuntimeContext) -> Self {
        let device_keys = runtime
            .shared_get::<Arc<crate::e2ee::DeviceKeyRegistry>>()
            .unwrap_or_else(|| Arc::new(crate::e2ee::DeviceKeyRegistry::new()));
        let room_hub = runtime
            .shared_get::<Arc<MessengerRoomHub>>()
            .unwrap_or_else(|| Arc::new(MessengerRoomHub::new()));
        let service = MessengerService::new(runtime.db_clone(), 1).with_device_keys(device_keys);
        Self {
            db: runtime.db_clone(),
            service,
            room_hub,
        }
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
        // Canonical API routes
        .route(
            "/api/messenger/conversations",
            post(rest::create_conversation).get(rest::list_conversations),
        )
        .route(
            "/api/messenger/conversations/{id}",
            get(rest::get_conversation),
        )
        .route(
            "/api/messenger/channels/{id}/translations",
            put(rest::update_channel_translation),
        )
        .route("/api/messenger/messages", post(rest::send_message))
        .route(
            "/api/messenger/messages/{message_id}",
            patch(rest::patch_message).delete(rest::delete_message),
        )
        .route(
            "/api/messenger/messages/search",
            get(rest::search_messages),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/messages",
            get(rest::list_messages),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/sync",
            get(rest::fast_sync),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/cursor",
            post(rest::update_cursor),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/members",
            post(rest::add_member).get(rest::list_members),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/members/{user_id}",
            delete(rest::remove_member),
        )
        .route(
            "/api/messenger/conversations/{conversation_id}/ws",
            get(realtime::ws_handler),
        )
        // E2EE cryptographic identity and prekey exchange routes
        .route("/api/messenger/e2ee/keys", post(rest::upload_device_keys))
        .route(
            "/api/messenger/e2ee/devices/{user_id}",
            get(rest::list_user_devices),
        )
        .route(
            "/api/messenger/e2ee/prekeys/{target_user_id}/{device_id}/claim",
            post(rest::claim_prekey_bundle),
        )
        // Legacy path aliases preserved for backwards-compatibility
        .route(
            "/conversations",
            post(rest::create_conversation).get(rest::list_conversations),
        )
        .route("/conversations/{id}", get(rest::get_conversation))
        .route(
            "/channels/{id}/translations",
            put(rest::update_channel_translation),
        )
        .route("/messages", post(rest::send_message))
        .route(
            "/messages/{message_id}",
            patch(rest::patch_message).delete(rest::delete_message),
        )
        .route("/messages/search", get(rest::search_messages))
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
        .route(
            "/conversations/{conversation_id}/members",
            post(rest::add_member).get(rest::list_members),
        )
        .route(
            "/conversations/{conversation_id}/members/{user_id}",
            delete(rest::remove_member),
        )
        .route(
            "/conversations/{conversation_id}/ws",
            get(realtime::ws_handler),
        )
        .with_state(runtime)
}

/// Construct the Axum router directly from the platform host runtime.
pub fn axum_router(host_runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let runtime = MessengerHttpRuntime::from_host(host_runtime);
    Ok(router(runtime))
}
