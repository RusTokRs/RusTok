use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, ensure};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use super::{connect_benchmark_database, ensure_database_metadata_stable, read_database_metadata};

const MANIFEST_CONTRACT: &str = "index_partition_evidence_manifest_v1";
const SHADOW_PLAN_VERSION: &str = "tenant_hash_shadow_v1";
const PLAN_DIGEST_CONTRACT: &str = "normalized_partition_plan_v1";
const RELATION_DIGEST_CONTRACT: &str = "index_partition_maintenance_relation_v1";
const MAINTENANCE_OPT_IN: &str = "INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE";
const DEFAULT_CHURN_CYCLES: usize = 3;
const MAX_CHURN_CYCLES: usize = 100;
const DEFAULT_CHURN_BATCH: i64 = 128;
const MAX_CHURN_BATCH: i64 = 10_000;

#[derive(Debug, Clone)]
pub struct PartitionMaintenanceConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub output_path: PathBuf,
    pub churn_cycles: usize,
    pub churn_batch: i64,
}

impl PartitionMaintenanceConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(MAINTENANCE_OPT_IN).as_deref(), Ok("1")),
            "{MAINTENANCE_OPT_IN}=1 is required because the runner creates retained evidence clones and commits churn into them"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition maintenance evidence")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let output_path = env::var("INDEX_PARTITION_MAINTENANCE_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("INDEX_PARTITION_EVIDENCE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"))
                    .join("maintenance.json")
            });
        let churn_cycles = env::var("INDEX_PARTITION_MAINTENANCE_CYCLES")
            .unwrap_or_else(|_| DEFAULT_CHURN_CYCLES.to_string())
            .parse::<usize>()
            .context("INDEX_PARTITION_MAINTENANCE_CYCLES must be an integer")?;
        ensure!(
            (1..=MAX_CHURN_CYCLES).contains(&churn_cycles),
            "INDEX_PARTITION_MAINTENANCE_CYCLES must be between 1 and {MAX_CHURN_CYCLES}"
        );
        let churn_batch = env::var("INDEX_PARTITION_MAINTENANCE_BATCH")
            .unwrap_or_else(|_| DEFAULT_CHURN_BATCH.to_string())
            .parse::<i64>()
            .context("INDEX_PARTITION_MAINTENANCE_BATCH must be an integer")?;
        ensure!(
            (1..=MAX_CHURN_BATCH).contains(&churn_batch),
            "INDEX_PARTITION_MAINTENANCE_BATCH must be between 1 and {MAX_CHURN_BATCH}"
        );
        ensure!(
            manifest_path != output_path,
            "manifest and maintenance evidence output paths must be distinct"
        );
        Ok(Self {
            database_url,
            manifest_path,
            output_path,
            churn_cycles,
            churn_batch,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PreparedManifest {
    contract: String,
    repository: String,
    commit: String,
    run_key: String,
    postgres_image: String,
    strategy: String,
    plan_digest_contract: String,
    modulus: u32,
    locales: Vec<String>,
    repetitions: EvidenceRepetitions,
    thresholds: JsonValue,
    evidence_id: String,
    shadow_plan_version: String,
    shadow_relations: ShadowRelations,
}

#[derive(Debug, Clone, Deserialize)]
struct EvidenceRepetitions {
    query: usize,
    mutation: usize,
    maintenance: usize,
    cutover: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ShadowRelations {
    definition_hash: String,
    entities: RelationPlan,
    links: RelationPlan,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationPlan {
    source: String,
    parent: String,
    partitions: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum RelationKind {
    Entities,
    Links,
}

impl RelationKind {
    fn label(self) -> &'static str {
        match self {
            Self::Entities => "entities",
            Self::Links => "links",
        }
    }

    fn digest_order(self) -> &'static str {
        match self {
            Self::Entities => {
                "tenant_id, module_name, entity_name, schema_version, locale_key, entity_id"
            }
            Self::Links => {
                "tenant_id, source_module, source_entity, source_schema_version, source_locale_key, source_entity_id, source_version, link_name, ordinal"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LogicalRelationEvidence {
    rows: i64,
    digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceSnapshot {
    canonical_entities: LogicalRelationEvidence,
    canonical_links: LogicalRelationEvidence,
    shadow_entities: LogicalRelationEvidence,
    shadow_links: LogicalRelationEvidence,
}

#[derive(Debug, Clone)]
struct MaintenanceLayout {
    schema: String,
    baseline_entities: String,
    baseline_links: String,
    shadow_entities: String,
    shadow_links: String,
    shadow_entity_partitions: Vec<String>,
    shadow_link_partitions: Vec<String>,
}

impl MaintenanceLayout {
    fn derive(manifest: &PreparedManifest) -> Result<Self> {
        let schema = format!("index_pe_maintenance_{}", &manifest.evidence_id[..16]);
        validate_identifier(&schema)?;
        let baseline_entities = "baseline_entities".to_owned();
        let baseline_links = "baseline_links".to_owned();
        let shadow_entities = "shadow_entities".to_owned();
        let shadow_links = "shadow_links".to_owned();
        for relation in [
            &baseline_entities,
            &baseline_links,
            &shadow_entities,
            &shadow_links,
        ] {
            validate_identifier(relation)?;
        }
        let shadow_entity_partitions = (0..manifest.modulus)
            .map(|remainder| format!("shadow_entities_p{remainder:03}"))
            .collect::<Vec<_>>();
        let shadow_link_partitions = (0..manifest.modulus)
            .map(|remainder| format!("shadow_links_p{remainder:03}"))
            .collect::<Vec<_>>();
        for relation in shadow_entity_partitions
            .iter()
            .chain(shadow_link_partitions.iter())
        {
            validate_identifier(relation)?;
        }
        Ok(Self {
            schema,
            baseline_entities,
            baseline_links,
            shadow_entities,
            shadow_links,
            shadow_entity_partitions,
            shadow_link_partitions,
        })
    }

    fn baseline_physical_relations(&self) -> Vec<String> {
        vec![self.baseline_entities.clone(), self.baseline_links.clone()]
    }

    fn shadow_physical_relations(&self) -> Vec<String> {
        self.shadow_entity_partitions
            .iter()
            .chain(self.shadow_link_partitions.iter())
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChurnEffect {
    entities: u64,
    links: u64,
}

#[derive(Debug, Clone, Serialize)]
struct MaintenanceTableStats {
    relation: String,
    estimated_live_tuples: i64,
    estimated_dead_tuples: i64,
    tuples_inserted: i64,
    tuples_updated: i64,
    tuples_deleted: i64,
    hot_updates: i64,
    vacuum_count: i64,
    autovacuum_count: i64,
    analyze_count: i64,
    autoanalyze_count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct MaintenanceSideStats {
    estimated_dead_tuples: i64,
    tables: Vec<MaintenanceTableStats>,
}

#[derive(Debug, Clone, Serialize)]
struct PartitionMaintenanceRunEvidence {
    name: String,
    schema: String,
    churn_cycles: usize,
    churn_batch: i64,
    affected_entities_per_cycle: u64,
    affected_links_per_cycle: u64,
    baseline_vacuum_ms: f64,
    shadow_vacuum_ms: f64,
    baseline_dead_tuples: i64,
    shadow_dead_tuples: i64,
    baseline_after_vacuum_dead_tuples: i64,
    shadow_after_vacuum_dead_tuples: i64,
    baseline_before_vacuum: MaintenanceSideStats,
    shadow_before_vacuum: MaintenanceSideStats,
    baseline_after_vacuum: MaintenanceSideStats,
    shadow_after_vacuum: MaintenanceSideStats,
    entities_after_vacuum: LogicalRelationEvidence,
    links_after_vacuum: LogicalRelationEvidence,
}

#[derive(Debug, Clone)]
pub struct PartitionMaintenanceCapture {
    pub evidence_id: String,
    pub output_path: PathBuf,
    pub schema: String,
    pub runs: usize,
}

pub async fn capture_partition_maintenance_evidence(
    config: &PartitionMaintenanceConfig,
) -> Result<PartitionMaintenanceCapture> {
    ensure_output_available(&config.output_path)?;
    let (manifest, raw_manifest) = read_manifest(&config.manifest_path)?;
    validate_manifest(&manifest, &raw_manifest)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared(
        "SET jit = off; SET lock_timeout = '5s'; SET statement_timeout = 0; SET enable_partition_pruning = on; SET synchronous_commit = on; SET vacuum_cost_delay = 0;",
    )
    .await
    .context("failed to pin partition maintenance evidence session settings")?;
    let database_metadata = read_database_metadata(&db).await?;
    ensure!(
        database_metadata.server_version_num.starts_with("16"),
        "partition maintenance evidence requires PostgreSQL 16, got {}",
        database_metadata.server_version_num
    );
    ensure!(
        database_metadata.jit == "off",
        "partition maintenance evidence requires jit=off"
    );
    ensure_session_setting(&db, "enable_partition_pruning", "on").await?;
    ensure_session_setting(&db, "synchronous_commit", "on").await?;
    ensure_session_setting(&db, "vacuum_cost_delay", "0").await?;
    ensure_unpartitioned_source(&db, "index_entities").await?;
    ensure_unpartitioned_source(&db, "index_links").await?;
    validate_shadow_catalog(&db, &manifest).await?;

    acquire_maintenance_lock(&db, &manifest.evidence_id).await?;
    let capture_result = capture_locked_maintenance(&db, &manifest, config).await;
    let release_result = release_maintenance_lock(&db, &manifest.evidence_id).await;
    let (layout, runs) = match capture_result {
        Ok(value) => {
            release_result?;
            value
        }
        Err(error) => {
            let _ = release_result;
            return Err(error);
        }
    };

    ensure_database_metadata_stable(&db, &database_metadata, "partition maintenance evidence")
        .await?;
    publish_maintenance_artifact(&config.output_path, &runs)?;
    Ok(PartitionMaintenanceCapture {
        evidence_id: manifest.evidence_id,
        output_path: config.output_path.clone(),
        schema: layout.schema,
        runs: runs.len(),
    })
}

async fn capture_locked_maintenance(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    config: &PartitionMaintenanceConfig,
) -> Result<(MaintenanceLayout, Vec<PartitionMaintenanceRunEvidence>)> {
    let source_before = source_snapshot(db, manifest).await?;
    ensure_source_parity(&source_before)?;

    let layout = MaintenanceLayout::derive(manifest)?;
    ensure_schema_absent(db, &layout.schema).await?;
    create_maintenance_clones(db, manifest, &layout).await?;
    analyze_all_relations(db, &layout).await?;
    ensure_clone_parity(db, &layout).await?;
    ensure_source_unchanged(db, manifest, &source_before).await?;

    let mut runs = Vec::with_capacity(manifest.repetitions.maintenance);
    for index in 0..manifest.repetitions.maintenance {
        runs.push(capture_maintenance_run(db, &layout, config, index + 1).await?);
    }
    ensure!(
        runs.len() == manifest.repetitions.maintenance,
        "partition maintenance runner did not produce the exact manifest maintenance run count"
    );
    let names = runs
        .iter()
        .map(|run| run.name.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        names.len() == runs.len(),
        "partition maintenance runs contain duplicate names"
    );
    ensure_source_unchanged(db, manifest, &source_before).await?;
    Ok((layout, runs))
}

async fn capture_maintenance_run(
    db: &DatabaseConnection,
    layout: &MaintenanceLayout,
    config: &PartitionMaintenanceConfig,
    ordinal: usize,
) -> Result<PartitionMaintenanceRunEvidence> {
    force_stats_flush(db).await?;
    let baseline_clean = side_stats(db, layout, MaintenanceSide::Baseline).await?;
    let shadow_clean = side_stats(db, layout, MaintenanceSide::Shadow).await?;
    ensure!(
        baseline_clean.estimated_dead_tuples == 0,
        "baseline maintenance clone must start each run with zero estimated dead tuples"
    );
    ensure!(
        shadow_clean.estimated_dead_tuples == 0,
        "shadow maintenance clone must start each run with zero estimated dead tuples"
    );

    let mut expected_effect: Option<ChurnEffect> = None;
    for cycle in 0..config.churn_cycles {
        let baseline_first = (ordinal + cycle) % 2 == 1;
        let (baseline, shadow) =
            commit_churn_cycle(db, layout, config.churn_batch, baseline_first).await?;
        ensure!(
            baseline == shadow,
            "baseline/shadow maintenance churn effects diverged: baseline={baseline:?}, shadow={shadow:?}"
        );
        ensure!(
            baseline.entities > 0,
            "maintenance churn must update at least one entity"
        );
        if let Some(expected) = &expected_effect {
            ensure!(
                expected == &baseline,
                "maintenance churn effect changed between committed cycles"
            );
        } else {
            expected_effect = Some(baseline);
        }
    }
    ensure_clone_parity(db, layout).await?;

    force_stats_flush(db).await?;
    let baseline_before = side_stats(db, layout, MaintenanceSide::Baseline).await?;
    let shadow_before = side_stats(db, layout, MaintenanceSide::Shadow).await?;
    ensure!(
        baseline_before.estimated_dead_tuples > 0,
        "baseline maintenance churn did not produce positive dead-tuple evidence"
    );
    ensure!(
        shadow_before.estimated_dead_tuples > 0,
        "shadow maintenance churn did not produce positive dead-tuple evidence"
    );

    let (baseline_vacuum_ms, shadow_vacuum_ms) = if ordinal % 2 == 1 {
        let baseline = vacuum_side(db, layout, MaintenanceSide::Baseline).await?;
        let shadow = vacuum_side(db, layout, MaintenanceSide::Shadow).await?;
        (baseline, shadow)
    } else {
        let shadow = vacuum_side(db, layout, MaintenanceSide::Shadow).await?;
        let baseline = vacuum_side(db, layout, MaintenanceSide::Baseline).await?;
        (baseline, shadow)
    };

    force_stats_flush(db).await?;
    let baseline_after = side_stats(db, layout, MaintenanceSide::Baseline).await?;
    let shadow_after = side_stats(db, layout, MaintenanceSide::Shadow).await?;
    ensure!(
        baseline_after.estimated_dead_tuples == 0,
        "ordinary baseline VACUUM did not clear estimated dead tuples"
    );
    ensure!(
        shadow_after.estimated_dead_tuples == 0,
        "ordinary shadow VACUUM did not clear estimated dead tuples"
    );
    let (entities_after_vacuum, links_after_vacuum) = ensure_clone_parity(db, layout).await?;
    let effect = expected_effect.context("maintenance run produced no churn effect")?;

    Ok(PartitionMaintenanceRunEvidence {
        name: format!("maintenance-{ordinal:03}"),
        schema: layout.schema.clone(),
        churn_cycles: config.churn_cycles,
        churn_batch: config.churn_batch,
        affected_entities_per_cycle: effect.entities,
        affected_links_per_cycle: effect.links,
        baseline_vacuum_ms,
        shadow_vacuum_ms,
        baseline_dead_tuples: baseline_before.estimated_dead_tuples,
        shadow_dead_tuples: shadow_before.estimated_dead_tuples,
        baseline_after_vacuum_dead_tuples: baseline_after.estimated_dead_tuples,
        shadow_after_vacuum_dead_tuples: shadow_after.estimated_dead_tuples,
        baseline_before_vacuum: baseline_before,
        shadow_before_vacuum: shadow_before,
        baseline_after_vacuum: baseline_after,
        shadow_after_vacuum: shadow_after,
        entities_after_vacuum,
        links_after_vacuum,
    })
}

#[derive(Debug, Clone, Copy)]
enum MaintenanceSide {
    Baseline,
    Shadow,
}

async fn commit_churn_cycle(
    db: &DatabaseConnection,
    layout: &MaintenanceLayout,
    batch: i64,
    baseline_first: bool,
) -> Result<(ChurnEffect, ChurnEffect)> {
    let transaction = db
        .begin()
        .await
        .context("failed to start partition maintenance churn transaction")?;
    let result = async {
        if baseline_first {
            let baseline =
                apply_churn_side(&transaction, layout, MaintenanceSide::Baseline, batch).await?;
            let shadow =
                apply_churn_side(&transaction, layout, MaintenanceSide::Shadow, batch).await?;
            Ok::<_, anyhow::Error>((baseline, shadow))
        } else {
            let shadow =
                apply_churn_side(&transaction, layout, MaintenanceSide::Shadow, batch).await?;
            let baseline =
                apply_churn_side(&transaction, layout, MaintenanceSide::Baseline, batch).await?;
            Ok::<_, anyhow::Error>((baseline, shadow))
        }
    }
    .await;
    match result {
        Ok(value) => {
            transaction
                .commit()
                .await
                .context("failed to commit partition maintenance evidence churn")?;
            Ok(value)
        }
        Err(error) => {
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn apply_churn_side(
    transaction: &DatabaseTransaction,
    layout: &MaintenanceLayout,
    side: MaintenanceSide,
    batch: i64,
) -> Result<ChurnEffect> {
    let (entities, links) = match side {
        MaintenanceSide::Baseline => (
            qualified(&layout.schema, &layout.baseline_entities),
            qualified(&layout.schema, &layout.baseline_links),
        ),
        MaintenanceSide::Shadow => (
            qualified(&layout.schema, &layout.shadow_entities),
            qualified(&layout.schema, &layout.shadow_links),
        ),
    };
    let entity_sql = format!(
        concat!(
            "WITH targets AS (SELECT tenant_id, module_name, entity_name, schema_version, entity_id, locale_key ",
            "FROM {entities} ORDER BY tenant_id, module_name, entity_name, schema_version, locale_key, entity_id LIMIT $1) ",
            "UPDATE {entities} AS entity SET updated_at = entity.updated_at + INTERVAL '1 microsecond' ",
            "FROM targets WHERE entity.tenant_id = targets.tenant_id AND entity.module_name = targets.module_name ",
            "AND entity.entity_name = targets.entity_name AND entity.schema_version = targets.schema_version ",
            "AND entity.entity_id = targets.entity_id AND entity.locale_key = targets.locale_key"
        ),
        entities = entities
    );
    let entity_result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            entity_sql,
            vec![batch.into()],
        ))
        .await
        .context("failed to apply partition maintenance entity churn")?;

    let link_sql = format!(
        concat!(
            "WITH targets AS (SELECT tenant_id, source_module, source_entity, source_schema_version, ",
            "source_entity_id, source_locale_key, source_version, link_name, ordinal, target_module, ",
            "target_entity, target_schema_version, target_entity_id, target_locale_key, created_at ",
            "FROM {links} ORDER BY tenant_id, source_module, source_entity, source_schema_version, ",
            "source_locale_key, source_entity_id, source_version, link_name, ordinal LIMIT $1), ",
            "deleted AS (DELETE FROM {links} AS link USING targets WHERE link.tenant_id = targets.tenant_id ",
            "AND link.source_module = targets.source_module AND link.source_entity = targets.source_entity ",
            "AND link.source_schema_version = targets.source_schema_version ",
            "AND link.source_entity_id = targets.source_entity_id ",
            "AND link.source_locale_key = targets.source_locale_key ",
            "AND link.source_version = targets.source_version AND link.link_name = targets.link_name ",
            "AND link.ordinal = targets.ordinal RETURNING link.*) ",
            "INSERT INTO {links} (tenant_id, source_module, source_entity, source_schema_version, ",
            "source_entity_id, source_locale_key, source_version, link_name, ordinal, target_module, ",
            "target_entity, target_schema_version, target_entity_id, target_locale_key, created_at) ",
            "SELECT tenant_id, source_module, source_entity, source_schema_version, source_entity_id, ",
            "source_locale_key, source_version, link_name, ordinal, target_module, target_entity, ",
            "target_schema_version, target_entity_id, target_locale_key, created_at FROM deleted"
        ),
        links = links
    );
    let link_result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            link_sql,
            vec![batch.into()],
        ))
        .await
        .context("failed to apply partition maintenance link churn")?;

    Ok(ChurnEffect {
        entities: entity_result.rows_affected(),
        links: link_result.rows_affected(),
    })
}

async fn vacuum_side(
    db: &DatabaseConnection,
    layout: &MaintenanceLayout,
    side: MaintenanceSide,
) -> Result<f64> {
    let relations = match side {
        MaintenanceSide::Baseline => layout.baseline_physical_relations(),
        MaintenanceSide::Shadow => layout.shadow_physical_relations(),
    };
    let started = Instant::now();
    for relation in relations {
        let statement = format!("VACUUM (ANALYZE) {};", qualified(&layout.schema, &relation));
        db.execute_unprepared(&statement).await.with_context(|| {
            format!("failed to execute ordinary maintenance evidence VACUUM: {statement}")
        })?;
    }
    let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
    ensure!(
        elapsed.is_finite() && elapsed >= 0.0,
        "partition maintenance VACUUM duration must be finite and non-negative"
    );
    Ok(elapsed)
}

async fn create_maintenance_clones(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    layout: &MaintenanceLayout,
) -> Result<()> {
    let transaction = db
        .begin()
        .await
        .context("failed to start partition maintenance clone transaction")?;
    let result = create_maintenance_clones_in_transaction(&transaction, manifest, layout).await;
    match result {
        Ok(()) => transaction
            .commit()
            .await
            .context("failed to commit partition maintenance evidence clones"),
        Err(error) => {
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn create_maintenance_clones_in_transaction(
    transaction: &DatabaseTransaction,
    manifest: &PreparedManifest,
    layout: &MaintenanceLayout,
) -> Result<()> {
    transaction
        .execute_unprepared(&format!(
            "CREATE SCHEMA {};",
            quote_identifier(&layout.schema)
        ))
        .await
        .context("failed to create partition maintenance evidence schema")?;
    transaction
        .execute_unprepared(&format!(
            "COMMENT ON SCHEMA {} IS 'rustok-index-partition-maintenance:{}';",
            quote_identifier(&layout.schema),
            manifest.evidence_id
        ))
        .await
        .context("failed to bind maintenance schema to evidence identity")?;

    create_plain_clone(
        transaction,
        &layout.schema,
        &layout.baseline_entities,
        "index_entities",
    )
    .await?;
    create_plain_clone(
        transaction,
        &layout.schema,
        &layout.baseline_links,
        "index_links",
    )
    .await?;
    create_partitioned_clone(
        transaction,
        &layout.schema,
        &layout.shadow_entities,
        &layout.shadow_entity_partitions,
        &manifest.shadow_relations.entities.parent,
        manifest.modulus,
    )
    .await?;
    create_partitioned_clone(
        transaction,
        &layout.schema,
        &layout.shadow_links,
        &layout.shadow_link_partitions,
        &manifest.shadow_relations.links.parent,
        manifest.modulus,
    )
    .await?;

    for relation in layout
        .baseline_physical_relations()
        .into_iter()
        .chain(layout.shadow_physical_relations())
    {
        transaction
            .execute_unprepared(&format!(
                "ALTER TABLE {} SET (autovacuum_enabled = false);",
                qualified(&layout.schema, &relation)
            ))
            .await
            .with_context(|| {
                format!("failed to disable autovacuum for maintenance clone {relation}")
            })?;
    }

    copy_relation(
        transaction,
        "index_entities",
        &qualified(&layout.schema, &layout.baseline_entities),
    )
    .await?;
    copy_relation(
        transaction,
        "index_links",
        &qualified(&layout.schema, &layout.baseline_links),
    )
    .await?;
    copy_relation(
        transaction,
        &quote_identifier(&manifest.shadow_relations.entities.parent),
        &qualified(&layout.schema, &layout.shadow_entities),
    )
    .await?;
    copy_relation(
        transaction,
        &quote_identifier(&manifest.shadow_relations.links.parent),
        &qualified(&layout.schema, &layout.shadow_links),
    )
    .await?;
    Ok(())
}

async fn create_plain_clone(
    transaction: &DatabaseTransaction,
    schema: &str,
    relation: &str,
    source: &str,
) -> Result<()> {
    transaction
        .execute_unprepared(&format!(
            "CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS);",
            qualified(schema, relation),
            quote_identifier(source),
        ))
        .await
        .with_context(|| format!("failed to create maintenance baseline clone {relation}"))?;
    Ok(())
}

async fn create_partitioned_clone(
    transaction: &DatabaseTransaction,
    schema: &str,
    parent: &str,
    partitions: &[String],
    source: &str,
    modulus: u32,
) -> Result<()> {
    transaction
        .execute_unprepared(&format!(
            "CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS) PARTITION BY HASH (tenant_id);",
            qualified(schema, parent),
            quote_identifier(source),
        ))
        .await
        .with_context(|| format!("failed to create maintenance shadow clone {parent}"))?;
    for (remainder, partition) in partitions.iter().enumerate() {
        transaction
            .execute_unprepared(&format!(
                "CREATE TABLE {} PARTITION OF {} FOR VALUES WITH (MODULUS {modulus}, REMAINDER {remainder});",
                qualified(schema, partition),
                qualified(schema, parent),
            ))
            .await
            .with_context(|| format!("failed to create maintenance partition {partition}"))?;
    }
    Ok(())
}

async fn copy_relation(
    transaction: &DatabaseTransaction,
    source_sql: &str,
    target_sql: &str,
) -> Result<()> {
    transaction
        .execute_unprepared(&format!(
            "INSERT INTO {target_sql} SELECT * FROM {source_sql};"
        ))
        .await
        .with_context(|| format!("failed to copy maintenance evidence source {source_sql}"))?;
    Ok(())
}

async fn analyze_all_relations(db: &DatabaseConnection, layout: &MaintenanceLayout) -> Result<()> {
    let relations = layout
        .baseline_physical_relations()
        .into_iter()
        .chain(layout.shadow_physical_relations());
    for relation in relations {
        db.execute_unprepared(&format!(
            "ANALYZE {};",
            qualified(&layout.schema, &relation)
        ))
        .await
        .with_context(|| format!("failed to analyze maintenance clone {relation}"))?;
    }
    force_stats_flush(db).await
}

async fn force_stats_flush(db: &DatabaseConnection) -> Result<()> {
    db.execute_unprepared("SELECT pg_stat_force_next_flush(); SELECT pg_stat_clear_snapshot();")
        .await
        .context("failed to flush PostgreSQL maintenance statistics")?;
    Ok(())
}

async fn side_stats(
    db: &DatabaseConnection,
    layout: &MaintenanceLayout,
    side: MaintenanceSide,
) -> Result<MaintenanceSideStats> {
    let expected = match side {
        MaintenanceSide::Baseline => layout.baseline_physical_relations(),
        MaintenanceSide::Shadow => layout.shadow_physical_relations(),
    }
    .into_iter()
    .collect::<BTreeSet<_>>();
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT relname, n_live_tup::bigint AS n_live_tup, n_dead_tup::bigint AS n_dead_tup, ",
                "n_tup_ins::bigint AS n_tup_ins, n_tup_upd::bigint AS n_tup_upd, ",
                "n_tup_del::bigint AS n_tup_del, n_tup_hot_upd::bigint AS n_tup_hot_upd, ",
                "vacuum_count::bigint AS vacuum_count, autovacuum_count::bigint AS autovacuum_count, ",
                "analyze_count::bigint AS analyze_count, autoanalyze_count::bigint AS autoanalyze_count ",
                "FROM pg_stat_user_tables WHERE schemaname = $1 ORDER BY relname"
            ),
            vec![layout.schema.clone().into()],
        ))
        .await?;
    let mut tables = Vec::with_capacity(expected.len());
    for row in rows {
        let relation: String = row.try_get("", "relname")?;
        if !expected.contains(&relation) {
            continue;
        }
        tables.push(MaintenanceTableStats {
            relation,
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
        });
    }
    ensure!(
        tables.len() == expected.len(),
        "maintenance statistics did not contain every expected physical relation"
    );
    ensure!(
        tables
            .iter()
            .all(|table| expected.contains(&table.relation)),
        "maintenance statistics contained an unexpected relation"
    );
    ensure!(
        tables.iter().all(|table| table.autovacuum_count == 0),
        "autovacuum must remain disabled for partition maintenance evidence clones"
    );
    let estimated_dead_tuples = tables.iter().try_fold(0_i64, |total, table| {
        total
            .checked_add(table.estimated_dead_tuples)
            .context("maintenance dead-tuple total overflow")
    })?;
    Ok(MaintenanceSideStats {
        estimated_dead_tuples,
        tables,
    })
}

async fn source_snapshot(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<SourceSnapshot> {
    Ok(SourceSnapshot {
        canonical_entities: logical_relation(db, "index_entities", RelationKind::Entities).await?,
        canonical_links: logical_relation(db, "index_links", RelationKind::Links).await?,
        shadow_entities: logical_relation(
            db,
            &quote_identifier(&manifest.shadow_relations.entities.parent),
            RelationKind::Entities,
        )
        .await?,
        shadow_links: logical_relation(
            db,
            &quote_identifier(&manifest.shadow_relations.links.parent),
            RelationKind::Links,
        )
        .await?,
    })
}

fn ensure_source_parity(snapshot: &SourceSnapshot) -> Result<()> {
    ensure!(
        snapshot.canonical_entities == snapshot.shadow_entities,
        "canonical and retained shadow entities diverged before maintenance evidence"
    );
    ensure!(
        snapshot.canonical_links == snapshot.shadow_links,
        "canonical and retained shadow links diverged before maintenance evidence"
    );
    Ok(())
}

async fn ensure_source_unchanged(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    expected: &SourceSnapshot,
) -> Result<()> {
    let actual = source_snapshot(db, manifest).await?;
    ensure!(
        &actual == expected,
        "canonical or retained snapshot-shadow relations changed during maintenance evidence"
    );
    Ok(())
}

async fn ensure_clone_parity(
    db: &DatabaseConnection,
    layout: &MaintenanceLayout,
) -> Result<(LogicalRelationEvidence, LogicalRelationEvidence)> {
    let baseline_entities = logical_relation(
        db,
        &qualified(&layout.schema, &layout.baseline_entities),
        RelationKind::Entities,
    )
    .await?;
    let shadow_entities = logical_relation(
        db,
        &qualified(&layout.schema, &layout.shadow_entities),
        RelationKind::Entities,
    )
    .await?;
    ensure!(
        baseline_entities == shadow_entities,
        "baseline/shadow maintenance entity clones diverged"
    );
    let baseline_links = logical_relation(
        db,
        &qualified(&layout.schema, &layout.baseline_links),
        RelationKind::Links,
    )
    .await?;
    let shadow_links = logical_relation(
        db,
        &qualified(&layout.schema, &layout.shadow_links),
        RelationKind::Links,
    )
    .await?;
    ensure!(
        baseline_links == shadow_links,
        "baseline/shadow maintenance link clones diverged"
    );
    Ok((baseline_entities, baseline_links))
}

async fn logical_relation(
    db: &DatabaseConnection,
    relation_sql: &str,
    kind: RelationKind,
) -> Result<LogicalRelationEvidence> {
    let sql = format!(
        concat!(
            "SELECT count(*)::bigint AS rows, ",
            "COALESCE(md5(string_agg(md5(row_to_json(row_data)::text), '' ORDER BY {})), md5('')) AS digest_seed ",
            "FROM (SELECT * FROM {}) row_data"
        ),
        kind.digest_order(),
        relation_sql,
    );
    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .with_context(|| {
            format!("maintenance logical digest query returned no row for {relation_sql}")
        })?;
    let rows: i64 = row.try_get("", "rows")?;
    let digest_seed: String = row.try_get("", "digest_seed")?;
    let digest = sha256_hex(
        format!(
            "{RELATION_DIGEST_CONTRACT}\u{1f}{}\u{1f}{rows}\u{1f}{digest_seed}",
            kind.label()
        )
        .as_bytes(),
    );
    Ok(LogicalRelationEvidence { rows, digest })
}

fn read_manifest(path: &Path) -> Result<(PreparedManifest, JsonValue)> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect partition manifest at {path:?}"))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "partition manifest must be a regular non-symlink file"
    );
    let bytes =
        fs::read(path).with_context(|| format!("failed to read partition manifest at {path:?}"))?;
    let raw: JsonValue =
        serde_json::from_slice(&bytes).context("failed to parse partition manifest JSON")?;
    let manifest = serde_json::from_value(raw.clone())
        .context("failed to deserialize prepared partition manifest")?;
    Ok((manifest, raw))
}

fn validate_manifest(manifest: &PreparedManifest, raw: &JsonValue) -> Result<()> {
    ensure!(
        manifest.contract == MANIFEST_CONTRACT,
        "unexpected manifest contract"
    );
    ensure!(
        manifest.repository == "RusTokRs/RusTok",
        "unexpected manifest repository"
    );
    ensure!(
        is_lower_hex(&manifest.commit, 40),
        "manifest commit must be a lowercase full SHA-1"
    );
    ensure!(
        !manifest.run_key.is_empty() && manifest.run_key.len() <= 128,
        "manifest run_key must be non-empty and bounded"
    );
    ensure!(
        manifest.postgres_image == "postgres:16",
        "manifest must pin postgres:16"
    );
    ensure!(
        manifest.strategy == "tenant_hash",
        "manifest strategy must be tenant_hash"
    );
    ensure!(
        manifest.plan_digest_contract == PLAN_DIGEST_CONTRACT,
        "unexpected plan digest contract"
    );
    ensure!(
        manifest.modulus >= 2 && manifest.modulus <= 128 && manifest.modulus.is_power_of_two(),
        "manifest modulus must be a power of two between 2 and 128"
    );
    ensure!(
        !manifest.locales.is_empty()
            && manifest.locales.iter().all(|locale| !locale.is_empty())
            && manifest.locales.iter().collect::<BTreeSet<_>>().len() == manifest.locales.len(),
        "manifest locales must be unique and non-empty"
    );
    ensure!(
        manifest.repetitions.query > 0
            && manifest.repetitions.mutation > 0
            && manifest.repetitions.maintenance > 0
            && manifest.repetitions.cutover > 0,
        "manifest repetition counts must all be positive"
    );
    ensure!(
        manifest.thresholds.is_object(),
        "manifest thresholds must be an object"
    );
    ensure!(
        is_lower_hex(&manifest.evidence_id, 64),
        "invalid manifest evidence_id"
    );
    ensure!(
        manifest.shadow_plan_version == SHADOW_PLAN_VERSION,
        "unexpected shadow plan version"
    );

    let mut input = raw
        .as_object()
        .cloned()
        .context("prepared manifest must be a JSON object")?;
    for key in ["evidence_id", "shadow_plan_version", "shadow_relations"] {
        ensure!(
            input.remove(key).is_some(),
            "prepared manifest is missing {key}"
        );
    }
    let expected_evidence_id = sha256_hex(&canonical_json_bytes(&JsonValue::Object(input))?);
    ensure!(
        expected_evidence_id == manifest.evidence_id,
        "manifest evidence_id does not match canonical manifest input"
    );

    let modulus = manifest.modulus.to_string();
    let definition = [
        "rustok-index-partition",
        SHADOW_PLAN_VERSION,
        manifest.evidence_id.as_str(),
        "tenant_hash",
        modulus.as_str(),
    ]
    .join("\u{1f}");
    let definition_hash = sha256_hex(definition.as_bytes());
    ensure!(
        manifest.shadow_relations.definition_hash == definition_hash,
        "shadow definition hash does not match evidence identity"
    );
    let suffix = &definition_hash[..24];
    validate_relation_plan(
        &manifest.shadow_relations.entities,
        "index_entities",
        &format!("index_entities_shadow_{suffix}"),
        manifest.modulus,
    )?;
    validate_relation_plan(
        &manifest.shadow_relations.links,
        "index_links",
        &format!("index_links_shadow_{suffix}"),
        manifest.modulus,
    )?;
    Ok(())
}

fn validate_relation_plan(
    plan: &RelationPlan,
    expected_source: &str,
    expected_parent: &str,
    modulus: u32,
) -> Result<()> {
    ensure!(
        plan.source == expected_source,
        "unexpected shadow source relation"
    );
    ensure!(
        plan.parent == expected_parent,
        "unexpected shadow parent relation"
    );
    validate_identifier(&plan.parent)?;
    ensure!(
        plan.partitions.len() == modulus as usize,
        "shadow relation must contain exactly {modulus} child partitions"
    );
    for (remainder, partition) in plan.partitions.iter().enumerate() {
        validate_identifier(partition)?;
        ensure!(
            partition == &format!("{expected_parent}_p{remainder:03}"),
            "unexpected shadow child relation"
        );
    }
    Ok(())
}

async fn ensure_session_setting(
    db: &DatabaseConnection,
    setting: &str,
    expected: &str,
) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT current_setting($1) AS value",
            vec![setting.into()],
        ))
        .await?
        .context("partition maintenance session-setting query returned no row")?;
    let actual: String = row.try_get("", "value")?;
    ensure!(
        actual == expected,
        "requires {setting}={expected}, got {actual}"
    );
    Ok(())
}

async fn ensure_unpartitioned_source(db: &DatabaseConnection, relation: &str) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT c.relkind::text AS relkind, c.relispartition, ",
                "EXISTS (SELECT 1 FROM pg_partitioned_table p WHERE p.partrelid = c.oid) AS partitioned ",
                "FROM pg_class c WHERE c.oid = to_regclass($1)"
            ),
            vec![relation.into()],
        ))
        .await?
        .with_context(|| format!("canonical relation {relation} was not found"))?;
    let relkind: String = row.try_get("", "relkind")?;
    let relispartition: bool = row.try_get("", "relispartition")?;
    let partitioned: bool = row.try_get("", "partitioned")?;
    ensure!(
        relkind == "r" && !relispartition && !partitioned,
        "canonical relation {relation} must remain an ordinary unpartitioned table"
    );
    Ok(())
}

async fn validate_shadow_catalog(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<()> {
    validate_shadow_relation_catalog(db, manifest, &manifest.shadow_relations.entities).await?;
    validate_shadow_relation_catalog(db, manifest, &manifest.shadow_relations.links).await?;
    Ok(())
}

async fn validate_shadow_relation_catalog(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    plan: &RelationPlan,
) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT c.relkind::text AS relkind, obj_description(c.oid, 'pg_class') AS comment ",
                "FROM pg_class c WHERE c.oid = to_regclass($1)"
            ),
            vec![plan.parent.clone().into()],
        ))
        .await?
        .with_context(|| format!("shadow parent {} was not found", plan.parent))?;
    let relkind: String = row.try_get("", "relkind")?;
    let comment: Option<String> = row.try_get("", "comment")?;
    let expected_comment = format!("rustok-index-partition:{}", manifest.evidence_id);
    ensure!(
        relkind == "p",
        "shadow parent {} must be partitioned",
        plan.parent
    );
    ensure!(
        comment.as_deref() == Some(expected_comment.as_str()),
        "shadow parent {} is not bound to the evidence identity",
        plan.parent
    );

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT child.relname, child.relispartition, ",
                "pg_get_expr(child.relpartbound, child.oid) AS bound ",
                "FROM pg_inherits inheritance ",
                "JOIN pg_class child ON child.oid = inheritance.inhrelid ",
                "WHERE inheritance.inhparent = to_regclass($1) ORDER BY child.relname"
            ),
            vec![plan.parent.clone().into()],
        ))
        .await?;
    ensure!(
        rows.len() == plan.partitions.len(),
        "shadow parent {} has an unexpected child count",
        plan.parent
    );
    for (remainder, row) in rows.into_iter().enumerate() {
        let name: String = row.try_get("", "relname")?;
        let relispartition: bool = row.try_get("", "relispartition")?;
        let bound: String = row.try_get("", "bound")?;
        ensure!(relispartition, "shadow child {name} is not a partition");
        ensure!(
            name == plan.partitions[remainder],
            "unexpected shadow child {name}"
        );
        let bound = bound.to_ascii_lowercase();
        ensure!(
            bound.contains(&format!("modulus {}", manifest.modulus))
                && bound.contains(&format!("remainder {remainder}")),
            "shadow child {name} has an unexpected bound: {bound}"
        );
    }
    Ok(())
}

async fn ensure_schema_absent(db: &DatabaseConnection, schema: &str) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regnamespace($1)::text AS schema",
            vec![schema.into()],
        ))
        .await?
        .context("maintenance schema existence query returned no row")?;
    let existing: Option<String> = row.try_get("", "schema")?;
    ensure!(
        existing.is_none(),
        "maintenance evidence schema already exists: {schema}"
    );
    Ok(())
}

async fn acquire_maintenance_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        vec![format!("rustok-index-partition-maintenance:{evidence_id}").into()],
    ))
    .await?
    .context("partition maintenance advisory lock returned no row")?;
    Ok(())
}

async fn release_maintenance_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock(hashtextextended($1, 0)) AS unlocked",
            vec![format!("rustok-index-partition-maintenance:{evidence_id}").into()],
        ))
        .await?
        .context("partition maintenance advisory unlock returned no row")?;
    let unlocked: bool = row.try_get("", "unlocked")?;
    ensure!(unlocked, "partition maintenance advisory lock was not held");
    Ok(())
}

fn ensure_output_available(path: &Path) -> Result<()> {
    ensure!(!path.exists(), "refusing to overwrite {path:?}");
    Ok(())
}

fn publish_maintenance_artifact(
    path: &Path,
    runs: &[PartitionMaintenanceRunEvidence],
) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create maintenance evidence directory {parent:?}")
        })?;
    }
    ensure_output_available(path)?;
    let mut bytes = serde_json::to_vec_pretty(runs)
        .context("failed to serialize partition maintenance evidence")?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| {
            format!("failed to create temporary maintenance evidence file {temporary:?}")
        })?;
    file.write_all(&bytes).with_context(|| {
        format!("failed to write temporary maintenance evidence file {temporary:?}")
    })?;
    file.sync_all().with_context(|| {
        format!("failed to sync temporary maintenance evidence file {temporary:?}")
    })?;
    let publish = fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish maintenance evidence to {path:?}"));
    let _ = fs::remove_file(&temporary);
    publish
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(value)
}

fn canonical_json_bytes(value: &JsonValue) -> Result<Vec<u8>> {
    serde_json::to_vec(&canonical_json(value)).context("failed to serialize canonical JSON")
}

fn canonical_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(values) => JsonValue::Array(values.iter().map(canonical_json).collect()),
        JsonValue::Object(values) => {
            let mut sorted = JsonMap::new();
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), canonical_json(&values[key]));
            }
            JsonValue::Object(sorted)
        }
        _ => value.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 63,
        "invalid PostgreSQL identifier length"
    );
    ensure!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "PostgreSQL identifier contains unsupported characters"
    );
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn qualified(schema: &str, relation: &str) -> String {
    format!(
        "{}.{}",
        quote_identifier(schema),
        quote_identifier(relation)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_layout_is_bounded_and_deterministic() {
        let manifest = PreparedManifest {
            contract: MANIFEST_CONTRACT.to_owned(),
            repository: "RusTokRs/RusTok".to_owned(),
            commit: "1".repeat(40),
            run_key: "fixture".to_owned(),
            postgres_image: "postgres:16".to_owned(),
            strategy: "tenant_hash".to_owned(),
            plan_digest_contract: PLAN_DIGEST_CONTRACT.to_owned(),
            modulus: 4,
            locales: vec!["en-US".to_owned()],
            repetitions: EvidenceRepetitions {
                query: 1,
                mutation: 1,
                maintenance: 2,
                cutover: 1,
            },
            thresholds: serde_json::json!({}),
            evidence_id: "a".repeat(64),
            shadow_plan_version: SHADOW_PLAN_VERSION.to_owned(),
            shadow_relations: ShadowRelations {
                definition_hash: "b".repeat(64),
                entities: RelationPlan {
                    source: "index_entities".to_owned(),
                    parent: "index_entities_shadow_fixture".to_owned(),
                    partitions: Vec::new(),
                },
                links: RelationPlan {
                    source: "index_links".to_owned(),
                    parent: "index_links_shadow_fixture".to_owned(),
                    partitions: Vec::new(),
                },
            },
        };
        let layout = MaintenanceLayout::derive(&manifest).unwrap();
        assert_eq!(layout.schema, "index_pe_maintenance_aaaaaaaaaaaaaaaa");
        assert_eq!(layout.shadow_entity_partitions.len(), 4);
        assert!(layout.shadow_entity_partitions[3].ends_with("p003"));
    }

    #[test]
    fn maintenance_sql_uses_explicit_analyze_vacuum() {
        let statement = format!(
            "VACUUM (ANALYZE) {};",
            qualified("index_pe_maintenance_fixture", "baseline_entities")
        );
        assert!(statement.starts_with("VACUUM (ANALYZE)"));
        assert!(statement.ends_with(';'));
        assert!(!statement.contains("index_entities\""));
    }
}
