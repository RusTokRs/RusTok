use crate::error::{Error, Result, http_error};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use rustok_api::{Action, Permission, Resource, has_effective_permission};
use rustok_outbox::entity::{self, SysEventStatus};
use rustok_telemetry::metrics;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Set, Value,
    sea_query::{Expr, SimpleExpr},
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::extractors::{auth::CurrentUser, rbac::RequireLogsRead, tenant::CurrentTenant};
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Debug, Deserialize)]
pub struct DlqQuery {
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
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("limit" = Option<u64>, Query, description = "Maximum number of results (1-200)"),
    ),
    responses(
        (status = 200, description = "Tenant-scoped DLQ event list", body = DlqListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn list_dlq(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    _user: RequireLogsRead,
    Query(query): Query<DlqQuery>,
) -> Result<Json<DlqListResponse>> {
    let requested_limit = Some(query.limit);
    let limit = query.limit.clamp(1, 200);

    let mut db_query = entity::Entity::find()
        .filter(entity::Column::Status.eq(SysEventStatus::Failed))
        .filter(sys_event_tenant_condition(
            ctx.db().get_database_backend(),
            tenant.id,
        ))
        .order_by_desc(entity::Column::CreatedAt)
        .limit(limit);

    if let Some(event_type) = query.event_type.as_ref() {
        db_query = db_query.filter(entity::Column::EventType.eq(event_type.as_str()));
    }

    if let Some(created_after) = query.created_after {
        db_query = db_query.filter(entity::Column::CreatedAt.gte(created_after));
    }

    let query_started_at = Instant::now();
    let models = db_query
        .all(ctx.db())
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
        ("id" = Uuid, Path, description = "Tenant-owned DLQ event UUID to replay"),
    ),
    responses(
        (status = 200, description = "Event requeued for processing", body = DlqReplayResponse),
        (status = 400, description = "Event is not in failed status"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "logs:manage required"),
        (status = 404, description = "Tenant-owned event not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn replay_dlq_event(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<DlqReplayResponse>> {
    let required_permission = Permission::new(Resource::Logs, Action::Manage);
    if !has_effective_permission(&user.permissions, &required_permission) {
        return Err(forbidden_error("Permission denied: logs:manage required"));
    }

    let model = entity::Entity::find()
        .filter(entity::Column::Id.eq(id))
        .filter(sys_event_tenant_condition(
            ctx.db().get_database_backend(),
            tenant.id,
        ))
        .one(ctx.db())
        .await
        .map_err(|e| Error::BadRequest(format!("Failed to fetch sys_event: {e}")))?
        .ok_or(Error::NotFound)?;

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
        .update(ctx.db())
        .await
        .map_err(|e| Error::BadRequest(format!("Failed to replay sys_event: {e}")))?;

    Ok(Json(DlqReplayResponse {
        id,
        status: "requeued",
    }))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/admin/events/dlq", axum::routing::get(list_dlq))
        .route(
            "/api/admin/events/dlq/{id}/replay",
            axum::routing::post(replay_dlq_event),
        )
}

fn default_limit() -> u64 {
    100
}

fn sys_event_tenant_condition(backend: DbBackend, tenant_id: Uuid) -> SimpleExpr {
    Expr::cust_with_values(
        sys_event_tenant_sql(backend),
        vec![Value::from(tenant_id.to_string())],
    )
}

fn sys_event_tenant_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Sqlite => {
            "(json_extract(payload, '$.tenant_id') = ?1 OR json_extract(payload, '$.event.tenant_id') = ?1)"
        }
        _ => "(payload->>'tenant_id' = $1 OR payload->'event'->>'tenant_id' = $1)",
    }
}

fn forbidden_error(description: impl Into<String>) -> Error {
    http_error(rustok_web::HttpError::forbidden("forbidden", description))
}

#[cfg(test)]
mod tests {
    use rustok_api::{Action, Permission, Resource};
    use sea_orm::DbBackend;

    use super::sys_event_tenant_sql;

    #[test]
    fn dlq_replay_permission_is_manage_not_read() {
        assert_ne!(
            Permission::new(Resource::Logs, Action::Manage),
            Permission::LOGS_READ
        );
    }

    #[test]
    fn tenant_condition_covers_current_and_legacy_envelope_shapes() {
        for backend in [DbBackend::Postgres, DbBackend::Sqlite] {
            let sql = sys_event_tenant_sql(backend);
            assert!(sql.starts_with('('));
            assert!(sql.ends_with(')'));
            assert!(sql.contains("tenant_id"));
            assert!(sql.contains("event"));
        }
    }
}
