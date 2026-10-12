use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};
use rustok_api::{AuthContext, HostRuntimeContext, Permission, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

pub mod pages;
pub mod realtime;

pub use pages::*;
pub use realtime::{ClientMessage, DocsRoomHub, ServerMessage, ws_handler};

/// Shared HTTP runtime state for the collaborative documents module.
#[derive(Clone)]
pub struct DocsHttpRuntime {
    pub db: DatabaseConnection,
    pub room_hub: Arc<DocsRoomHub>,
}

impl DocsHttpRuntime {
    /// Create a new runtime with the default in-memory room hub.
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            room_hub: Arc::new(DocsRoomHub::new()),
        }
    }

    /// Create a runtime with an explicit collaborative room hub.
    pub fn with_room_hub(db: DatabaseConnection, room_hub: Arc<DocsRoomHub>) -> Self {
        Self { db, room_hub }
    }

    /// Construct runtime from the host platform application context.
    pub fn from_host(runtime: &HostRuntimeContext) -> Self {
        let room_hub = runtime
            .shared_get::<Arc<DocsRoomHub>>()
            .unwrap_or_else(|| Arc::new(DocsRoomHub::new()));
        Self {
            db: runtime.db_clone(),
            room_hub,
        }
    }
}

/// Enforces that the docs module is actively enabled for the requesting tenant.
pub async fn ensure_docs_module_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> HttpResult<()> {
    match rustok_api::is_tenant_module_enabled(db, tenant_id, "docs").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpError::new(
            StatusCode::FORBIDDEN,
            "MODULE_NOT_ENABLED",
            "Module 'docs' is not enabled for this tenant",
        )),
        Err(error) => {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "failed to verify Docs tenant-module lifecycle state"
            );
            Err(HttpError::internal(
                "The Docs operation could not be completed",
            ))
        }
    }
}

/// Verifies that the authenticated principal possesses at least one of the required permissions.
pub fn ensure_docs_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    error_message: &str,
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden("permission_denied", error_message));
    }
    Ok(())
}

/// Mounts all collaborative document REST and WebSocket routes with runtime state.
pub fn router(runtime: DocsHttpRuntime) -> Router {
    Router::new()
        // Page CRUD & tree
        .route(
            "/api/docs/pages",
            get(pages::list_pages).post(pages::create_page),
        )
        .route(
            "/api/docs/pages/tree",
            get(pages::get_page_tree),
        )
        .route(
            "/api/docs/pages/by-slug/{slug}",
            get(pages::get_page_by_slug),
        )
        .route(
            "/api/docs/pages/{id}",
            get(pages::get_page)
                .patch(pages::update_page)
                .delete(pages::delete_page),
        )
        // Markdown projection export
        .route(
            "/api/docs/pages/{id}/markdown",
            get(pages::render_markdown),
        )
        // CRDT Sync and delta ingestion
        .route(
            "/api/docs/pages/{id}/sync",
            get(pages::get_sync),
        )
        .route(
            "/api/docs/pages/{id}/deltas",
            post(pages::append_deltas),
        )
        .route(
            "/api/docs/pages/{id}/compact",
            post(pages::compact_snapshot),
        )
        // Real-time WebSocket collaborative session
        .route(
            "/api/docs/pages/{id}/ws",
            get(realtime::ws_handler),
        )
        .with_state(runtime)
}

/// Construct the Axum router directly from the platform host runtime.
pub fn axum_router(host_runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let runtime = DocsHttpRuntime::from_host(host_runtime);
    Ok(router(runtime))
}
