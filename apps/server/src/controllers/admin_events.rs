use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use loco_rs::Error;
use loco_rs::Result;
use rustok_outbox::entity::{self, SysEventStatus};
use rustok_telemetry::metrics;
use sea_orm::{
    sea_query::{Expr, SimpleExpr},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, Value,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::extractors::rbac::RequireLogsRead;

#[derive(Debug, Deserialize)]
pub struct DlqQuery {
    pub tenant_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DlqEventItem {
    pub id: Uuid,
    pub event_type: String,
    pub schema_version: i16,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DlqListResponse {
    pub total: usize,
    pub items: Vec<DlqEventItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DlqReplayResponse {
    pub id: Uuid,
    pub status: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/admin/events/dlq",
    params(
        ("tenant_id" = Option<Uuid>, Query, description = "Filter by tenant UUID"),
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("limit" = Option<u64>, Query, description = "Maximum number of results (1-200)"),
    ),
    responses(
        (status = 200, description = "DLQ event list", body = DlqListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn list_dlq(
    State(ctx): State<AppContext>,
    _user: RequireLogsRead,
    Query(query): Query<DlqQuery>,
) -> Result<Json<DlqListResponse>> {
    let requested_limit = Some(query.limit);
    let limit = query.limit.clamp(1, 200);

    let mut db_query = entity::Entity::find()
        .filter(entity::Column::Status.eq(SysEventStatus::Failed))
        .order_by_desc(entity::Column::CreatedAt)
        .limit(limit);

    if let Some(event_type) = query.event_type.as_ref() {
        db_query = db_query.filter(entity::Column::EventType.eq(event_type.as_str()));
    }

    if let Some(created_after) = query.created_after {
        db_query = db_query.filter(entity::Column::CreatedAt.gte(created_after));
    }

    if let Some(tenant_id) = query.tenant_id {
        db_query = db_query.filter(sys_event_tenant_condition(
            ctx.db.get_database_backend(),
            tenant_id,
        ));
    }

    let query_started_at = Instant::now();
    let models = db_query
        .all(&ctx.db)
        .await
        .map_err(|e| Error::BadRequest(format!("Failed to load DLQ events: {e}")))?;
    metrics::record_read_path_query(
        "http",
        "admin.list_dlq",
        "dlq_page",
        query_started_at.elapsed().as_secs_f64(),
        models.len() as u64,
    );

    let items = models
        .into_iter()
        .map(|model| DlqEventItem {
            id: model.id,
            event_type: model.event_type,
            schema_version: model.schema_version,
            retry_count: model.retry_count,
            last_error: model.last_error,
            created_at: model.created_at,
        })
        .collect::<Vec<_>>();

    metrics::record_read_path_budget(
        "http",
        "admin.list_dlq",
        requested_limit,
        limit,
        items.len(),
    );

    Ok(Json(DlqListResponse {
        total: items.len(),
        items,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/events/dlq/{id}/replay",
    params(
        ("id" = Uuid, Path, description = "DLQ event UUID to replay"),
    ),
    responses(
        (status = 200, description = "Event requeued for processing", body = DlqReplayResponse),
        (status = 400, description = "Event is not in failed status"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Event not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn replay_dlq_event(
    State(ctx): State<AppContext>,
    _user: RequireLogsRead,
    Path(id): Path<Uuid>,
) -> Result<Json<DlqReplayResponse>> {
    let model = entity::Entity::find_by_id(id)
        .one(&ctx.db)
        .await
        .map_err(|e| Error::BadRequest(format!("Failed to fetch sys_event: {e}")))?
        .ok_or_else(|| Error::NotFound)?;

    if model.status != SysEventStatus::Failed {
        return Err(Error::BadRequest(
            "Only failed (DLQ) events can be replayed".to_string(),
        ));
    }

    let mut active: entity::ActiveModel = model.into();
    active.status = Set(SysEventStatus::Pending);
    active.retry_count = Set(0);
    active.next_attempt_at = Set(None);
    active.last_error = Set(None);
    active.claimed_by = Set(None);
    active.claimed_at = Set(None);
    active.dispatched_at = Set(None);

    active
        .update(&ctx.db)
        .await
        .map_err(|e| Error::BadRequest(format!("Failed to replay sys_event: {e}")))?;

    Ok(Json(DlqReplayResponse {
        id,
        status: "requeued",
    }))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/admin/events")
        .add("/dlq", axum::routing::get(list_dlq))
        .add("/dlq/{id}/replay", axum::routing::post(replay_dlq_event))
}

fn default_limit() -> u64 {
    100
}

fn sys_event_tenant_condition(backend: DbBackend, tenant_id: Uuid) -> SimpleExpr {
    let tenant_id = tenant_id.to_string();
    let sql = match backend {
        DbBackend::Sqlite => {
            "json_extract(payload, '$.tenant_id') = ?1 OR json_extract(payload, '$.event.tenant_id') = ?1"
        }
        _ => "payload->>'tenant_id' = $1 OR payload->'event'->>'tenant_id' = $1",
    };

    Expr::cust_with_values(sql, vec![Value::from(tenant_id)])
}
