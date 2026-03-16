use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_content::dto::{NodeListItem, NodeResponse};
use rustok_content::{CreateNodeInput, ListNodesFilter, NodeService, UpdateNodeInput};
use rustok_telemetry::metrics;
use std::time::Instant;
use uuid::Uuid;

use crate::context::TenantContext;
use crate::extractors::rbac::{
    RequireNodesCreate, RequireNodesDelete, RequireNodesList, RequireNodesRead, RequireNodesUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;

/// List content nodes
#[utoipa::path(
    get,
    path = "/api/content/nodes",
    tag = "content",
    params(
        ListNodesFilter
    ),
    responses(
        (status = 200, description = "List of nodes", body = Vec<NodeListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_nodes(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireNodesList(user): RequireNodesList,
    Query(filter): Query<ListNodesFilter>,
) -> Result<Json<Vec<NodeListItem>>> {
    let requested_limit = Some(filter.per_page);
    let effective_limit = filter.per_page.min(100);
    let service = NodeService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let (items, _) = service
        .list_nodes(tenant.id, user.security_context(), filter)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "content.list_nodes",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        items.len() as u64,
    );

    metrics::record_read_path_budget(
        "http",
        "content.list_nodes",
        requested_limit,
        effective_limit,
        items.len(),
    );

    Ok(Json(items))
}

/// Get a single content node by ID
#[utoipa::path(
    get,
    path = "/api/content/nodes/{id}",
    tag = "content",
    params(
        ("id" = Uuid, Path, description = "Node ID")
    ),
    responses(
        (status = 200, description = "Node details", body = NodeResponse),
        (status = 404, description = "Node not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_node(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireNodesRead,
    Path(id): Path<Uuid>,
) -> Result<Json<NodeResponse>> {
    let service = NodeService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let node = service
        .get_node(tenant.id, id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(node))
}

/// Create a new content node
#[utoipa::path(
    post,
    path = "/api/content/nodes",
    tag = "content",
    request_body = CreateNodeInput,
    responses(
        (status = 201, description = "Node created", body = NodeResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_node(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireNodesCreate(user): RequireNodesCreate,
    Json(input): Json<CreateNodeInput>,
) -> Result<(StatusCode, Json<NodeResponse>)> {
    let service = NodeService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let node = service
        .create_node(tenant.id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(node)))
}

/// Update an existing content node
#[utoipa::path(
    put,
    path = "/api/content/nodes/{id}",
    tag = "content",
    params(
        ("id" = Uuid, Path, description = "Node ID")
    ),
    request_body = UpdateNodeInput,
    responses(
        (status = 200, description = "Node updated", body = NodeResponse),
        (status = 404, description = "Node not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_node(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireNodesUpdate(user): RequireNodesUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateNodeInput>,
) -> Result<Json<NodeResponse>> {
    let service = NodeService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let node = service
        .update_node(tenant.id, id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(node))
}

/// Delete a content node
#[utoipa::path(
    delete,
    path = "/api/content/nodes/{id}",
    tag = "content",
    params(
        ("id" = Uuid, Path, description = "Node ID")
    ),
    responses(
        (status = 204, description = "Node deleted"),
        (status = 404, description = "Node not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_node(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireNodesDelete(user): RequireNodesDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = NodeService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete_node(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
