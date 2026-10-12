use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::HttpResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DocsHttpRuntime, ensure_docs_module_enabled, ensure_docs_permission};
use crate::{
    dto::{
        ApplyDeltaDto, CreatePageDto, DocumentSyncDto, PageSummaryDto, PageTreeNodeDto,
        UpdatePageDto,
    },
    services::DocumentService,
};

/// Query parameters for listing pages.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListPagesQuery {
    pub include_archived: Option<bool>,
}

/// Query parameters for deleting a page.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeletePageQuery {
    pub hard: Option<bool>,
}

/// Query parameters for state synchronization.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncQuery {
    pub client_seq_id: Option<i64>,
}

/// Exported Markdown projection response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownExportDto {
    pub page_id: Uuid,
    pub markdown: String,
}

/// Response returned when appending CRDT deltas over REST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendDeltasResponse {
    pub page_id: Uuid,
    pub latest_seq_id: i64,
}

/// Response returned when triggering manual snapshot compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactSnapshotResponse {
    pub page_id: Uuid,
    pub status: String,
}

/// List collaborative pages for the authenticated tenant.
pub async fn list_pages(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<ListPagesQuery>,
) -> HttpResult<Json<Vec<PageSummaryDto>>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_LIST, Permission::PAGES_READ],
        "Permission denied: pages:list or pages:read required",
    )?;

    let pages = DocumentService::list_pages(
        &runtime.db,
        tenant.id,
        query.include_archived.unwrap_or(false),
    )
    .await?;

    Ok(Json(pages))
}

/// Retrieve the hierarchical navigation tree for all pages in the tenant.
pub async fn get_page_tree(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
) -> HttpResult<Json<Vec<PageTreeNodeDto>>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_READ, Permission::PAGES_LIST],
        "Permission denied: pages:read required",
    )?;

    let tree = DocumentService::get_page_tree(&runtime.db, tenant.id).await?;
    Ok(Json(tree))
}

/// Retrieve a collaborative page summary by UUID.
pub async fn get_page(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
) -> HttpResult<Json<PageSummaryDto>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_READ],
        "Permission denied: pages:read required",
    )?;

    let page = DocumentService::get_page(&runtime.db, tenant.id, page_id).await?;
    Ok(Json(page))
}

/// Retrieve a collaborative page summary by URL slug.
pub async fn get_page_by_slug(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(slug): Path<String>,
) -> HttpResult<Json<PageSummaryDto>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_READ],
        "Permission denied: pages:read required",
    )?;

    let page = DocumentService::get_page_by_slug(&runtime.db, tenant.id, &slug).await?;
    Ok(Json(page))
}

/// Create a new collaborative document page with initial snapshot.
pub async fn create_page(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(dto): Json<CreatePageDto>,
) -> HttpResult<(StatusCode, Json<PageSummaryDto>)> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_CREATE],
        "Permission denied: pages:create required",
    )?;

    let page = DocumentService::create_page(&runtime.db, tenant.id, dto).await?;
    Ok((StatusCode::CREATED, Json(page)))
}

/// Update metadata for an existing collaborative page.
pub async fn update_page(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
    Json(dto): Json<UpdatePageDto>,
) -> HttpResult<Json<PageSummaryDto>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_UPDATE],
        "Permission denied: pages:update required",
    )?;

    let page = DocumentService::update_page(&runtime.db, tenant.id, page_id, dto).await?;
    Ok(Json(page))
}

/// Delete a page (soft-delete with child reparenting, or hard purge).
pub async fn delete_page(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
    Query(query): Query<DeletePageQuery>,
) -> HttpResult<StatusCode> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_DELETE],
        "Permission denied: pages:delete required",
    )?;

    DocumentService::delete_page(&runtime.db, tenant.id, page_id, query.hard.unwrap_or(false))
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Render document content to canonical Markdown projection.
pub async fn render_markdown(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
) -> HttpResult<Json<MarkdownExportDto>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_READ],
        "Permission denied: pages:read required",
    )?;

    let markdown = DocumentService::render_markdown(&runtime.db, tenant.id, page_id).await?;
    Ok(Json(MarkdownExportDto { page_id, markdown }))
}

/// Retrieve differential CRDT synchronization payload (snapshot and subsequent updates).
pub async fn get_sync(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
    Query(query): Query<SyncQuery>,
) -> HttpResult<Json<DocumentSyncDto>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_READ],
        "Permission denied: pages:read required",
    )?;

    let payload =
        DocumentService::get_sync_payload(&runtime.db, tenant.id, page_id, query.client_seq_id)
            .await?;

    Ok(Json(payload))
}

/// Ingest a batch of CRDT deltas over REST, persisting monotonically and broadcasting to active WebSocket peers.
pub async fn append_deltas(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
    Json(dto): Json<ApplyDeltaDto>,
) -> HttpResult<Json<AppendDeltasResponse>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_UPDATE],
        "Permission denied: pages:update required",
    )?;

    let latest_seq_id =
        DocumentService::append_updates(&runtime.db, tenant.id, page_id, dto.deltas.clone())
            .await?;

    // Broadcast deltas to active collaborative WebSocket peers
    runtime.room_hub.broadcast(
        tenant.id,
        page_id,
        crate::controllers::realtime::ServerMessage::Deltas {
            sender_peer_id: 0,
            deltas: dto.deltas,
        },
    );

    Ok(Json(AppendDeltasResponse {
        page_id,
        latest_seq_id,
    }))
}

/// Trigger compaction of deltas into a consolidated snapshot and purge old update logs.
pub async fn compact_snapshot(
    State(runtime): State<DocsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(page_id): Path<Uuid>,
) -> HttpResult<Json<CompactSnapshotResponse>> {
    ensure_docs_module_enabled(&runtime.db, tenant.id).await?;
    ensure_docs_permission(
        &auth,
        &[Permission::PAGES_MANAGE, Permission::PAGES_UPDATE],
        "Permission denied: pages:manage or pages:update required",
    )?;

    DocumentService::compact_snapshot(&runtime.db, tenant.id, page_id).await?;

    Ok(Json(CompactSnapshotResponse {
        page_id,
        status: "compacted".to_string(),
    }))
}
