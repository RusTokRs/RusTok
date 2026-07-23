use std::{fs, path::Path, time::Instant};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, TryGetable,
};
use serde::Serialize;

use super::{
    BenchmarkConfig, DatasetConfig, Prototype, analyze_sql, churn_cycle_sql,
    connect_benchmark_database, full_prototype_sql, source_dataset_sql, vacuum_sql,
};

#[derive(Debug, Serialize)]
pub struct MaintenanceBenchmarkReport {
    pub generated_at: DateTime<Utc>,
    pub dataset_scale: String,
    pub cycles: u32,
    pub prototypes: Vec<PrototypeMaintenanceReport>,
}

#[derive(Debug, Serialize)]
pub struct PrototypeMaintenanceReport {
    pub prototype: Prototype,
    pub schema: &'static str,
    pub baseline: MaintenanceSnapshot,
    pub after_churn: MaintenanceSnapshot,
    pub vacuum_duration_ms: u128,
    pub after_vacuum: MaintenanceSnapshot,
}

#[derive(Debug, Serialize)]
pub struct MaintenanceSnapshot {
    pub captured_at: DateTime<Utc>,
    pub schema_bytes: i64,
    pub entity_rows: i64,
    pub link_rows: i64,
    pub table_stats: Vec<TableMaintenanceStats>,
}

#[derive(Debug, Serialize)]
pub struct TableMaintenanceStats {
    pub relation: String,
    pub estimated_live_tuples: i64,
    pub estimated_dead_tuples: i64,
    pub tuples_inserted: i64,
    pub tuples_updated: i64,
    pub tuples_deleted: i64,
    pub hot_updates: i64,
    pub vacuum_count: i64,
    pub autovacuum_count: i64,
    pub analyze_count: i64,
    pub autoanalyze_count: i64,
}

#[derive(Debug, Clone, Copy)]
struct Cardinality {
    entity_rows: i64,
    link_rows: i64,
}

pub async fn run_maintenance(
    config: &BenchmarkConfig,
    cycles: u32,
) -> Result<MaintenanceBenchmarkReport> {
    ensure!(cycles > 0, "maintenance benchmark cycles must be greater than zero");
    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared("SET jit = off; SET statement_timeout = '30min';")
        .await
        .context("failed to configure maintenance benchmark session")?;
    db.execute_unprepared(&source_dataset_sql(&config.dataset))
        .await
        .context("failed to create maintenance benchmark source dataset")?;

    let mut prototypes = Vec::with_capacity(Prototype::ALL.len());
    for prototype in Prototype::ALL {
        db.execute_unprepared(&full_prototype_sql(prototype))
            .await
            .with_context(|| format!("failed to prepare {:?} maintenance prototype", prototype))?;
        let baseline = snapshot(&db, prototype, &config.dataset).await?;

        let cycle_sql = churn_cycle_sql(prototype, &config.dataset);
        for cycle in 1..=cycles {
            let transaction = db.begin().await?;
            let result = transaction.execute_unprepared(&cycle_sql).await;
            match result {
                Ok(_) => transaction.commit().await?,
                Err(error) => {
                    transaction.rollback().await?;
                    return Err(error).with_context(|| {
                        format!("failed {:?} maintenance churn cycle {cycle}", prototype)
                    });
                }
            }
        }

        db.execute_unprepared(&analyze_sql(prototype))
            .await
            .with_context(|| format!("failed to analyze {:?} after churn", prototype))?;
        let after_churn = snapshot(&db, prototype, &config.dataset).await?;

        let vacuum_started = Instant::now();
        db.execute_unprepared(&vacuum_sql(prototype))
            .await
            .with_context(|| format!("failed to vacuum {:?} prototype", prototype))?;
        let vacuum_duration_ms = vacuum_started.elapsed().as_millis();
        let after_vacuum = snapshot(&db, prototype, &config.dataset).await?;

        prototypes.push(PrototypeMaintenanceReport {
            prototype,
            schema: prototype.schema(),
            baseline,
            after_churn,
            vacuum_duration_ms,
            after_vacuum,
        });
    }

    Ok(MaintenanceBenchmarkReport {
        generated_at: Utc::now(),
        dataset_scale: format!("{:?}", config.dataset.scale),
        cycles,
        prototypes,
    })
}

pub fn write_maintenance_report(
    path: &Path,
    report: &MaintenanceBenchmarkReport,
) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create maintenance report directory {parent:?}"))?;
    }
    fs::write(path, serde_json::to_vec_pretty(report)?)
        .with_context(|| format!("failed to write maintenance report to {path:?}"))?;
    Ok(())
}

async fn snapshot(
    db: &DatabaseConnection,
    prototype: Prototype,
    dataset: &DatasetConfig,
) -> Result<MaintenanceSnapshot> {
    db.execute_unprepared("SELECT pg_stat_force_next_flush(); SELECT pg_stat_clear_snapshot();")
        .await
        .context("failed to flush and clear PostgreSQL statistics snapshot")?;
    let cardinality = prototype_cardinality(db, prototype).await?;
    validate_cardinality(prototype.schema(), cardinality, dataset)?;

    Ok(MaintenanceSnapshot {
        captured_at: Utc::now(),
        schema_bytes: schema_size_bytes(db, prototype.schema()).await?,
        entity_rows: cardinality.entity_rows,
        link_rows: cardinality.link_rows,
        table_stats: table_stats(db, prototype.schema()).await?,
    })
}

async fn table_stats(
    db: &DatabaseConnection,
    schema: &str,
) -> Result<Vec<TableMaintenanceStats>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT relname, n_live_tup::bigint AS n_live_tup, n_dead_tup::bigint AS n_dead_tup, n_tup_ins::bigint AS n_tup_ins, n_tup_upd::bigint AS n_tup_upd, n_tup_del::bigint AS n_tup_del, n_tup_hot_upd::bigint AS n_tup_hot_upd, vacuum_count::bigint AS vacuum_count, autovacuum_count::bigint AS autovacuum_count, analyze_count::bigint AS analyze_count, autoanalyze_count::bigint AS autoanalyze_count FROM pg_stat_user_tables WHERE schemaname = $1 ORDER BY relname",
            vec![schema.into()],
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(TableMaintenanceStats {
                relation: row.try_get("", "relname")?,
                estimated_live_tuples: row.try_get("", "n_live_tup")?,
                estimated_dead_tuples: row.try_get("", "n_dead_tup")?,
                tuples_inserted: row.try_get("", "n_tup_ins")?,
                tuples_updated: row.try_get("", "n_tup_upd")?,
                tuples_deleted: row.try_get("", "n_tup_del")?,
                hot_updates: row.try_get("", "n_tup_hot_upd")?,
                vacuum_count: row.try_get("", "vacuum_count")?,
                autovacuum_count: row.try_get("", "autovacuum_count")?,
                analyze_count: row.try_get("", "analyze_count")?,
                autoanalyze_count: row.try_get("", "autoanalyze_count")?,
            })
        })
        .collect()
}

async fn schema_size_bytes(db: &DatabaseConnection, schema: &str) -> Result<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(sum(pg_total_relation_size(class.oid)), 0)::bigint AS bytes FROM pg_class AS class JOIN pg_namespace AS namespace ON namespace.oid = class.relnamespace WHERE namespace.nspname = $1 AND class.relkind IN ('r', 'm')",
            vec![schema.into()],
        ))
        .await?
        .context("maintenance schema size query returned no row")?;
    row.try_get("", "bytes").map_err(Into::into)
}

async fn prototype_cardinality(
    db: &DatabaseConnection,
    prototype: Prototype,
) -> Result<Cardinality> {
    let sql = match prototype {
        Prototype::Jsonb => "SELECT (SELECT count(*) FROM idx_bench_jsonb.entity)::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_jsonb.link)::bigint AS link_rows",
        Prototype::TypedEav => "SELECT (SELECT count(*) FROM idx_bench_eav.entity)::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_eav.link)::bigint AS link_rows",
        Prototype::HotProjection => "SELECT ((SELECT count(*) FROM idx_bench_hot.product) + (SELECT count(*) FROM idx_bench_hot.variant) + (SELECT count(*) FROM idx_bench_hot.sales_channel))::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_hot.link)::bigint AS link_rows",
    };
    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .context("maintenance cardinality query returned no row")?;
    Ok(Cardinality {
        entity_rows: row.try_get("", "entity_rows")?,
        link_rows: row.try_get("", "link_rows")?,
    })
}

fn validate_cardinality(
    label: &str,
    actual: Cardinality,
    dataset: &DatasetConfig,
) -> Result<()> {
    let expected_entities = i64::try_from(dataset.total_entity_rows())?;
    let expected_links = i64::try_from(dataset.total_link_rows())?;
    ensure!(
        actual.entity_rows == expected_entities,
        "{label} entity cardinality drift after maintenance: expected {expected_entities}, got {}",
        actual.entity_rows
    );
    ensure!(
        actual.link_rows == expected_links,
        "{label} link cardinality drift after maintenance: expected {expected_links}, got {}",
        actual.link_rows
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_types_keep_vacuum_and_dead_tuple_evidence_explicit() {
        let stats = TableMaintenanceStats {
            relation: "entity".to_owned(),
            estimated_live_tuples: 10,
            estimated_dead_tuples: 5,
            tuples_inserted: 10,
            tuples_updated: 5,
            tuples_deleted: 2,
            hot_updates: 1,
            vacuum_count: 0,
            autovacuum_count: 0,
            analyze_count: 1,
            autoanalyze_count: 0,
        };
        assert_eq!(stats.estimated_dead_tuples, 5);
        assert_eq!(stats.vacuum_count, 0);
    }
}
