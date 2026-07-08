//! Database baseline collection task
//!
//! Collects a repeatable evidence bundle for hot-path queries:
//! - top statements from `pg_stat_statements` when available
//! - EXPLAIN plans for known GraphQL/admin read paths
//!
//! Current legacy server task bridge; target entrypoint:
//! `rustok-cli task db-baseline --tenant-id <uuid> --top-n 15 --output tmp/db-baseline.json`

use crate::error::{Error, Result};
use crate::tasks::{Task, TaskAppContext as AppContext, TaskInfo, Vars};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, Statement,
};
use serde::Serialize;
use std::fs;
use uuid::Uuid;

use crate::models::_entities::tenants::Column as TenantsColumn;
use crate::models::tenants;

pub struct DbBaselineTask;

#[async_trait]
impl Task for DbBaselineTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "db_baseline".to_string(),
            detail: "Collect pg_stat_statements and EXPLAIN plans for hot-path queries".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let tenant_id = resolve_tenant_id(ctx, vars).await?;
        let top_n = parse_top_n(vars)?;
        let report = collect_baseline_report(ctx, tenant_id, top_n).await?;
        let payload = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::Message(format!("Failed to serialize baseline report: {error}"))
        })?;

        if let Some(path) = vars.cli.get("output") {
            fs::write(path, payload.as_bytes()).map_err(|error| {
                Error::Message(format!("Failed to write baseline report: {error}"))
            })?;
            tracing::info!(output = %path, "Database baseline report written");
        } else {
            println!("{payload}");
        }

        Ok(())
    }
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
    ctx: &AppContext,
    tenant_id: Uuid,
    top_n: usize,
) -> Result<BaselineReport> {
    let backend = ctx.db.get_database_backend();

    Ok(BaselineReport {
        generated_at: Utc::now().to_rfc3339(),
        backend: format!("{backend:?}").to_lowercase(),
        tenant_id,
        top_n,
        pg_stat_statements: collect_pg_stat_statements(ctx, top_n).await,
        explain_plans: collect_explain_plans(ctx, tenant_id).await?,
    })
}

async fn collect_pg_stat_statements(ctx: &AppContext, top_n: usize) -> PgStatStatementsReport {
    if ctx.db.get_database_backend() != DbBackend::Postgres {
        return PgStatStatementsReport {
            available: false,
            error: Some("pg_stat_statements is only available on PostgreSQL".to_string()),
            statements: Vec::new(),
        };
    }

    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT
            queryid::text AS query_id,
            calls::bigint AS calls,
            total_exec_time::double precision AS total_exec_time_ms,
            mean_exec_time::double precision AS mean_exec_time_ms,
            rows::bigint AS rows,
            LEFT(REGEXP_REPLACE(query, '\s+', ' ', 'g'), 1000) AS query
        FROM pg_stat_statements
        ORDER BY total_exec_time DESC
        LIMIT $1
        "#,
        vec![(top_n as i64).into()],
    );

    match ctx.db.query_all(statement).await {
        Ok(rows) => {
            let statements = rows
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
                .collect();

            PgStatStatementsReport {
                available: true,
                error: None,
                statements,
            }
        }
        Err(error) => PgStatStatementsReport {
            available: false,
            error: Some(format!("pg_stat_statements unavailable: {error}")),
            statements: Vec::new(),
        },
    }
}

async fn collect_explain_plans(
    ctx: &AppContext,
    tenant_id: Uuid,
) -> Result<Vec<ExplainPlanReport>> {
    let specs = hot_path_specs(ctx.db.get_database_backend(), tenant_id);
    let mut reports = Vec::with_capacity(specs.len());

    for spec in specs {
        let plan_sql = explain_sql(ctx.db.get_database_backend(), &spec.sql);
        let lines =
            explain_lines(ctx, ctx.db.get_database_backend(), plan_sql, spec.values).await?;
        reports.push(ExplainPlanReport {
            name: spec.name,
            sql: spec.sql,
            plan_lines: lines,
        });
    }

    Ok(reports)
}

struct HotPathSpec {
    name: &'static str,
    sql: String,
    values: Vec<sea_orm::Value>,
}

fn hot_path_specs(backend: DbBackend, tenant_id: Uuid) -> Vec<HotPathSpec> {
    let now = Utc::now();
    let current_period_start = now - Duration::days(30);
    let previous_period_start = current_period_start - Duration::days(30);
    let tenant_id_string = tenant_id.to_string();

    match backend {
        DbBackend::Sqlite => vec![
            HotPathSpec {
                name: "root.users.count",
                sql: "SELECT COUNT(*) FROM users WHERE tenant_id = ?1".to_string(),
                values: vec![tenant_id.into()],
            },
            HotPathSpec {
                name: "root.users.page",
                sql: "SELECT * FROM users WHERE tenant_id = ?1 LIMIT ?2 OFFSET ?3".to_string(),
                values: vec![tenant_id.into(), 20_i64.into(), 0_i64.into()],
            },
            HotPathSpec {
                name: "root.dashboard_stats.users_snapshot",
                sql: r#"
                    SELECT
                        CAST(COUNT(*) AS INTEGER) AS total_count,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                    FROM users
                    WHERE tenant_id = ?1
                "#
                .to_string(),
                values: vec![
                    tenant_id.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                ],
            },
            HotPathSpec {
                name: "root.dashboard_stats.posts_snapshot",
                sql: r#"
                    SELECT
                        CAST(COUNT(*) AS INTEGER) AS total_count,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                    FROM nodes
                    WHERE tenant_id = ?1 AND kind = ?4
                "#
                .to_string(),
                values: vec![
                    tenant_id.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                    "post".into(),
                ],
            },
            HotPathSpec {
                name: "root.dashboard_stats.orders_snapshot",
                sql: r#"
                    SELECT
                        CAST(COUNT(*) AS INTEGER) AS total_orders,
                        CAST(COALESCE(SUM(COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)), 0) AS INTEGER) AS total_revenue,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_orders,
                        CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_orders,
                        CAST(COALESCE(SUM(CASE
                            WHEN created_at >= ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                            ELSE 0
                        END), 0) AS INTEGER) AS current_revenue,
                        CAST(COALESCE(SUM(CASE
                            WHEN created_at >= ?3 AND created_at < ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                            ELSE 0
                        END), 0) AS INTEGER) AS previous_revenue
                    FROM sys_events
                    WHERE event_type = 'order.placed'
                      AND (
                          json_extract(payload, '$.tenant_id') = ?1
                          OR json_extract(payload, '$.event.tenant_id') = ?1
                      )
                "#
                .to_string(),
                values: vec![
                    tenant_id_string.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                ],
            },
            HotPathSpec {
                name: "root.recent_activity.recent_users",
                sql: "SELECT * FROM users WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT ?2"
                    .to_string(),
                values: vec![tenant_id.into(), 20_i64.into()],
            },
        ],
        _ => vec![
            HotPathSpec {
                name: "root.users.count",
                sql: "SELECT COUNT(*) FROM users WHERE tenant_id = $1".to_string(),
                values: vec![tenant_id.into()],
            },
            HotPathSpec {
                name: "root.users.page",
                sql: "SELECT * FROM users WHERE tenant_id = $1 LIMIT $2 OFFSET $3".to_string(),
                values: vec![tenant_id.into(), 20_i64.into(), 0_i64.into()],
            },
            HotPathSpec {
                name: "root.dashboard_stats.users_snapshot",
                sql: r#"
                    SELECT
                        COUNT(*)::bigint AS total_count,
                        COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                        COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                    FROM users
                    WHERE tenant_id = $1
                "#
                .to_string(),
                values: vec![
                    tenant_id.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                ],
            },
            HotPathSpec {
                name: "root.dashboard_stats.posts_snapshot",
                sql: r#"
                    SELECT
                        COUNT(*)::bigint AS total_count,
                        COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                        COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                    FROM nodes
                    WHERE tenant_id = $1 AND kind = $4
                "#
                .to_string(),
                values: vec![
                    tenant_id.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                    "post".into(),
                ],
            },
            HotPathSpec {
                name: "root.dashboard_stats.orders_snapshot",
                sql: r#"
                    SELECT
                        COUNT(*)::bigint AS total_orders,
                        COALESCE(SUM(COALESCE((payload->'event'->'data'->>'total')::bigint, 0)), 0)::bigint AS total_revenue,
                        COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_orders,
                        COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_orders,
                        COALESCE(SUM(CASE
                            WHEN created_at >= $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                            ELSE 0
                        END), 0)::bigint AS current_revenue,
                        COALESCE(SUM(CASE
                            WHEN created_at >= $3 AND created_at < $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                            ELSE 0
                        END), 0)::bigint AS previous_revenue
                    FROM sys_events
                    WHERE event_type = 'order.placed'
                      AND (
                          payload->>'tenant_id' = $1
                          OR payload->'event'->>'tenant_id' = $1
                      )
                "#
                .to_string(),
                values: vec![
                    tenant_id_string.into(),
                    current_period_start.into(),
                    previous_period_start.into(),
                ],
            },
            HotPathSpec {
                name: "root.recent_activity.recent_users",
                sql: "SELECT * FROM users WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2"
                    .to_string(),
                values: vec![tenant_id.into(), 20_i64.into()],
            },
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
    ctx: &AppContext,
    backend: DbBackend,
    sql: String,
    values: Vec<sea_orm::Value>,
) -> Result<Vec<String>> {
    let statement = Statement::from_sql_and_values(backend, sql, values);
    let rows = ctx.db.query_all(statement).await?;

    let lines = match backend {
        DbBackend::Sqlite => rows
            .into_iter()
            .filter_map(|row| row.try_get::<String>("", "detail").ok())
            .collect(),
        _ => rows
            .into_iter()
            .filter_map(|row| row.try_get::<String>("", "QUERY PLAN").ok())
            .collect(),
    };

    Ok(lines)
}

async fn resolve_tenant_id(ctx: &AppContext, vars: &Vars) -> Result<Uuid> {
    if let Some(raw) = vars.cli.get("tenant_id") {
        return Uuid::parse_str(raw)
            .map_err(|error| Error::Message(format!("Invalid tenant_id `{raw}`: {error}")));
    }

    tenants::Entity::find()
        .filter(TenantsColumn::IsActive.eq(true))
        .order_by_asc(TenantsColumn::CreatedAt)
        .one(&ctx.db)
        .await?
        .map(|tenant| tenant.id)
        .ok_or_else(|| Error::Message("No active tenant found for baseline collection".to_string()))
}

fn parse_top_n(vars: &Vars) -> Result<usize> {
    match vars.cli.get("top_n") {
        Some(raw) => raw
            .parse::<usize>()
            .map_err(|error| Error::Message(format!("Invalid top_n `{raw}`: {error}"))),
        None => Ok(10),
    }
}
