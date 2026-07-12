//! Database baseline report command implementation.

use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_cli_core::{CliCoreError, CliCoreResult, CommandOutcome};
use rustok_tenant::{TenantReadPort, TenantService};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use serde::Serialize;
use std::{fs, time::Duration as StdDuration};
use uuid::Uuid;

pub(super) async fn execute(
    db: &DatabaseConnection,
    args: &serde_json::Value,
) -> CliCoreResult<CommandOutcome> {
    let options = options(args)?;
    let tenant_id = resolve_tenant_id(db, options).await?;
    let top_n = parse_top_n(options)?;
    let report = collect_baseline_report(db, tenant_id, top_n).await?;
    let payload = serde_json::to_string_pretty(&report).map_err(command_failed)?;

    if let Some(path) = options.get("output").and_then(serde_json::Value::as_str) {
        fs::write(path, payload.as_bytes()).map_err(command_failed)?;
    }

    Ok(
        CommandOutcome::success("Database baseline report collected")
            .with_data(serde_json::to_value(report).map_err(command_failed)?),
    )
}

#[derive(Debug, Serialize)]
struct BaselineReport {
    generated_at: String,
    backend: String,
    tenant_id: Uuid,
    top_n: usize,
    pg_stat_statements: PgStatStatementsReport,
    explain_plans: Vec<ExplainPlanReport>,
}

#[derive(Debug, Serialize)]
struct PgStatStatementsReport {
    available: bool,
    error: Option<String>,
    statements: Vec<PgStatStatementEntry>,
}

#[derive(Debug, Serialize)]
struct PgStatStatementEntry {
    query_id: String,
    calls: i64,
    total_exec_time_ms: f64,
    mean_exec_time_ms: f64,
    rows: i64,
    query: String,
}

#[derive(Debug, Serialize)]
struct ExplainPlanReport {
    name: &'static str,
    sql: String,
    plan_lines: Vec<String>,
}

async fn collect_baseline_report(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    top_n: usize,
) -> CliCoreResult<BaselineReport> {
    let backend = db.get_database_backend();
    Ok(BaselineReport {
        generated_at: Utc::now().to_rfc3339(),
        backend: format!("{backend:?}").to_lowercase(),
        tenant_id,
        top_n,
        pg_stat_statements: collect_pg_stat_statements(db, top_n).await,
        explain_plans: collect_explain_plans(db, tenant_id).await?,
    })
}

async fn collect_pg_stat_statements(
    db: &DatabaseConnection,
    top_n: usize,
) -> PgStatStatementsReport {
    if db.get_database_backend() != DbBackend::Postgres {
        return PgStatStatementsReport {
            available: false,
            error: Some("pg_stat_statements is only available on PostgreSQL".to_string()),
            statements: Vec::new(),
        };
    }
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT queryid::text AS query_id, calls::bigint AS calls,
               total_exec_time::double precision AS total_exec_time_ms,
               mean_exec_time::double precision AS mean_exec_time_ms,
               rows::bigint AS rows,
               LEFT(REGEXP_REPLACE(query, '\s+', ' ', 'g'), 1000) AS query
        FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT $1
        "#,
        vec![(top_n as i64).into()],
    );
    match db.query_all(statement).await {
        Ok(rows) => PgStatStatementsReport {
            available: true,
            error: None,
            statements: rows
                .into_iter()
                .filter_map(|row| {
                    Some(PgStatStatementEntry {
                        query_id: row.try_get("", "query_id").ok()?,
                        calls: row.try_get("", "calls").ok()?,
                        total_exec_time_ms: row.try_get("", "total_exec_time_ms").ok()?,
                        mean_exec_time_ms: row.try_get("", "mean_exec_time_ms").ok()?,
                        rows: row.try_get("", "rows").ok()?,
                        query: row.try_get("", "query").ok()?,
                    })
                })
                .collect(),
        },
        Err(error) => PgStatStatementsReport {
            available: false,
            error: Some(format!("pg_stat_statements unavailable: {error}")),
            statements: Vec::new(),
        },
    }
}

async fn collect_explain_plans(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> CliCoreResult<Vec<ExplainPlanReport>> {
    let backend = db.get_database_backend();
    let specs = hot_path_specs(backend, tenant_id);
    let mut reports = Vec::with_capacity(specs.len());
    for spec in specs {
        let plan_lines =
            explain_lines(db, backend, explain_sql(backend, &spec.sql), spec.values).await?;
        reports.push(ExplainPlanReport {
            name: spec.name,
            sql: spec.sql,
            plan_lines,
        });
    }
    Ok(reports)
}

struct HotPathSpec {
    name: &'static str,
    sql: String,
    values: Vec<Value>,
}

fn hot_path_specs(backend: DbBackend, tenant_id: Uuid) -> Vec<HotPathSpec> {
    let now = Utc::now();
    let current_period_start = now - Duration::days(30);
    let previous_period_start = current_period_start - Duration::days(30);
    let tenant_id_string = tenant_id.to_string();
    match backend {
        DbBackend::Sqlite => vec![
            HotPathSpec { name: "root.users.count", sql: "SELECT COUNT(*) FROM users WHERE tenant_id = ?1".to_string(), values: vec![tenant_id.into()] },
            HotPathSpec { name: "root.users.page", sql: "SELECT * FROM users WHERE tenant_id = ?1 LIMIT ?2 OFFSET ?3".to_string(), values: vec![tenant_id.into(), 20_i64.into(), 0_i64.into()] },
            HotPathSpec { name: "root.dashboard_stats.users_snapshot", sql: r#"SELECT CAST(COUNT(*) AS INTEGER) AS total_count, CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count, CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count FROM users WHERE tenant_id = ?1"#.to_string(), values: vec![tenant_id.into(), current_period_start.into(), previous_period_start.into()] },
            HotPathSpec { name: "root.dashboard_stats.posts_snapshot", sql: r#"SELECT CAST(COUNT(*) AS INTEGER) AS total_count, CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count, CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count FROM nodes WHERE tenant_id = ?1 AND kind = ?4"#.to_string(), values: vec![tenant_id.into(), current_period_start.into(), previous_period_start.into(), "post".into()] },
            HotPathSpec { name: "root.dashboard_stats.orders_snapshot", sql: r#"SELECT CAST(COUNT(*) AS INTEGER) AS total_orders, CAST(COALESCE(SUM(COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)), 0) AS INTEGER) AS total_revenue, CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_orders, CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_orders, CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0) ELSE 0 END), 0) AS INTEGER) AS current_revenue, CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0) ELSE 0 END), 0) AS INTEGER) AS previous_revenue FROM sys_events WHERE event_type = 'order.placed' AND (json_extract(payload, '$.tenant_id') = ?1 OR json_extract(payload, '$.event.tenant_id') = ?1)"#.to_string(), values: vec![tenant_id_string.into(), current_period_start.into(), previous_period_start.into()] },
            HotPathSpec { name: "root.recent_activity.recent_users", sql: "SELECT * FROM users WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2".to_string(), values: vec![tenant_id.into(), 20_i64.into()] },
        ],
        _ => vec![
            HotPathSpec { name: "root.users.count", sql: "SELECT COUNT(*) FROM users WHERE tenant_id = $1".to_string(), values: vec![tenant_id.into()] },
            HotPathSpec { name: "root.users.page", sql: "SELECT * FROM users WHERE tenant_id = $1 LIMIT $2 OFFSET $3".to_string(), values: vec![tenant_id.into(), 20_i64.into(), 0_i64.into()] },
            HotPathSpec { name: "root.dashboard_stats.users_snapshot", sql: r#"SELECT COUNT(*)::bigint AS total_count, COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count, COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count FROM users WHERE tenant_id = $1"#.to_string(), values: vec![tenant_id.into(), current_period_start.into(), previous_period_start.into()] },
            HotPathSpec { name: "root.dashboard_stats.posts_snapshot", sql: r#"SELECT COUNT(*)::bigint AS total_count, COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count, COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count FROM nodes WHERE tenant_id = $1 AND kind = $4"#.to_string(), values: vec![tenant_id.into(), current_period_start.into(), previous_period_start.into(), "post".into()] },
            HotPathSpec { name: "root.dashboard_stats.orders_snapshot", sql: r#"SELECT COUNT(*)::bigint AS total_orders, COALESCE(SUM(COALESCE((payload->'event'->'data'->>'total')::bigint, 0)), 0)::bigint AS total_revenue, COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_orders, COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_orders, COALESCE(SUM(CASE WHEN created_at >= $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0) ELSE 0 END), 0)::bigint AS current_revenue, COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0) ELSE 0 END), 0)::bigint AS previous_revenue FROM sys_events WHERE event_type = 'order.placed' AND (payload->>'tenant_id' = $1 OR payload->'event'->>'tenant_id' = $1)"#.to_string(), values: vec![tenant_id_string.into(), current_period_start.into(), previous_period_start.into()] },
            HotPathSpec { name: "root.recent_activity.recent_users", sql: "SELECT * FROM users WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2".to_string(), values: vec![tenant_id.into(), 20_i64.into()] },
        ],
    }
}

fn explain_sql(backend: DbBackend, sql: &str) -> String {
    match backend {
        DbBackend::Sqlite => format!("EXPLAIN QUERY PLAN {sql}"),
        _ => format!("EXPLAIN (FORMAT TEXT) {sql}"),
    }
}

async fn explain_lines(
    db: &DatabaseConnection,
    backend: DbBackend,
    sql: String,
    values: Vec<Value>,
) -> CliCoreResult<Vec<String>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(command_failed)?;
    Ok(match backend {
        DbBackend::Sqlite => rows
            .into_iter()
            .filter_map(|row| row.try_get::<String>("", "detail").ok())
            .collect(),
        _ => rows
            .into_iter()
            .filter_map(|row| row.try_get::<String>("", "QUERY PLAN").ok())
            .collect(),
    })
}

async fn resolve_tenant_id(
    db: &DatabaseConnection,
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<Uuid> {
    if let Some(raw) = options.get("tenant_id").and_then(serde_json::Value::as_str) {
        return Uuid::parse_str(raw).map_err(|error| CliCoreError::InvalidInput {
            message: format!("--tenant-id must be a UUID: {error}"),
        });
    }
    TenantService::new(db.clone())
        .read_default_active_tenant(
            PortContext::new("platform", PortActor::system(), "en", "db-baseline")
                .with_deadline(StdDuration::from_secs(5)),
        )
        .await
        .map(|tenant| tenant.id)
        .map_err(|error| command_failed(error.message))
}

fn parse_top_n(options: &serde_json::Map<String, serde_json::Value>) -> CliCoreResult<usize> {
    options
        .get("top_n")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| CliCoreError::InvalidInput {
                    message: "--top-n must be a positive integer".to_string(),
                })
                .and_then(|raw| {
                    raw.parse().map_err(|_| CliCoreError::InvalidInput {
                        message: "--top-n must be a positive integer".to_string(),
                    })
                })
        })
        .transpose()
        .map(|value| value.unwrap_or(10))
}

fn options(args: &serde_json::Value) -> CliCoreResult<&serde_json::Map<String, serde_json::Value>> {
    args.get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "core db-baseline expects normalized command options".to_string(),
        })
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
