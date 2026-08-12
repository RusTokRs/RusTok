use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SeaValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use super::{
    connect_benchmark_database, ensure_database_metadata_stable,
    explain::parse_mutation_explain_metrics, read_database_metadata,
};

const MANIFEST_CONTRACT: &str = "index_partition_evidence_manifest_v1";
const SHADOW_PLAN_VERSION: &str = "tenant_hash_shadow_v1";
const PLAN_DIGEST_CONTRACT: &str = "normalized_partition_plan_v1";
const MUTATION_OPT_IN: &str = "INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE";
const DEFAULT_MUTATION_SAMPLES: usize = 7;
const MAX_MUTATION_SAMPLES: usize = 100;
const SAMPLE_SAVEPOINT: &str = "index_partition_mutation_sample";

#[derive(Debug, Clone)]
pub struct PartitionMutationConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub output_path: PathBuf,
    pub samples: usize,
}

impl PartitionMutationConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(MUTATION_OPT_IN).as_deref(), Ok("1")),
            "{MUTATION_OPT_IN}=1 is required because the runner executes rollback-only PostgreSQL mutations"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition mutation evidence")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let output_path = env::var("INDEX_PARTITION_MUTATION_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("INDEX_PARTITION_EVIDENCE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"))
                    .join("mutation.json")
            });
        let samples = env::var("INDEX_PARTITION_MUTATION_SAMPLES")
            .unwrap_or_else(|_| DEFAULT_MUTATION_SAMPLES.to_string())
            .parse::<usize>()
            .context("INDEX_PARTITION_MUTATION_SAMPLES must be an integer")?;
        ensure!(
            (3..=MAX_MUTATION_SAMPLES).contains(&samples),
            "INDEX_PARTITION_MUTATION_SAMPLES must be between 3 and {MAX_MUTATION_SAMPLES}"
        );
        ensure!(
            manifest_path != output_path,
            "manifest and mutation evidence output paths must be distinct"
        );
        Ok(Self {
            database_url,
            manifest_path,
            output_path,
            samples,
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

#[derive(Debug, Clone)]
struct EntityAnchor {
    tenant_id: String,
    module_name: String,
    entity_name: String,
    schema_version: i32,
    entity_id: String,
    locale_key: String,
}

#[derive(Debug, Clone)]
struct LinkAnchor {
    tenant_id: String,
    source_module: String,
    source_entity: String,
    source_schema_version: i32,
    source_entity_id: String,
    source_locale_key: String,
    source_version: String,
    link_name: String,
    ordinal: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutationTarget {
    Entities,
    Links,
}

#[derive(Debug, Clone)]
struct MutationCase {
    name: String,
    template: &'static str,
    baseline_sql: String,
    shadow_sql: String,
    values: Vec<SeaValue>,
    target: MutationTarget,
}

#[derive(Debug, Clone, Copy)]
enum MutationSide {
    Baseline,
    Shadow,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct MutationRelationReads {
    target_relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MutationExplainSample {
    sample: usize,
    execution_time_ms: f64,
    affected_rows: u64,
    maximum_node_wal_records: u64,
    maximum_node_wal_fpi: u64,
    maximum_node_wal_bytes: u64,
    shared_hit_blocks: u64,
    shared_read_blocks: u64,
    relation_reads: MutationRelationReads,
    explain: JsonValue,
}

#[derive(Debug, Clone, Serialize)]
struct PartitionMutationRunEvidence {
    name: String,
    template: String,
    sample_count: usize,
    affected_rows: u64,
    baseline_p95_ms: f64,
    shadow_p95_ms: f64,
    baseline_wal_bytes: u64,
    shadow_wal_bytes: u64,
    baseline_relation_reads: MutationRelationReads,
    shadow_relation_reads: MutationRelationReads,
    baseline_explain_samples: Vec<MutationExplainSample>,
    shadow_explain_samples: Vec<MutationExplainSample>,
}

#[derive(Debug, Clone)]
pub struct PartitionMutationCapture {
    pub evidence_id: String,
    pub output_path: PathBuf,
    pub runs: usize,
    pub samples_per_run: usize,
}

pub async fn capture_partition_mutation_evidence(
    config: &PartitionMutationConfig,
) -> Result<PartitionMutationCapture> {
    ensure_output_available(&config.output_path)?;
    let (manifest, raw_manifest) = read_manifest(&config.manifest_path)?;
    validate_manifest(&manifest, &raw_manifest)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared(
        "SET jit = off; SET lock_timeout = '5s'; SET statement_timeout = 0; SET enable_partition_pruning = on; SET synchronous_commit = on;",
    )
    .await
    .context("failed to pin partition mutation evidence session settings")?;
    let database_metadata = read_database_metadata(&db).await?;
    ensure!(
        database_metadata.server_version_num.starts_with("16"),
        "partition mutation evidence requires PostgreSQL 16, got {}",
        database_metadata.server_version_num
    );
    ensure!(
        database_metadata.jit == "off",
        "partition mutation evidence requires jit=off"
    );
    ensure_session_setting(&db, "enable_partition_pruning", "on").await?;
    ensure_session_setting(&db, "synchronous_commit", "on").await?;
    ensure_unpartitioned_source(&db, "index_entities").await?;
    ensure_unpartitioned_source(&db, "index_links").await?;
    validate_shadow_catalog(&db, &manifest).await?;

    acquire_mutation_lock(&db, &manifest.evidence_id).await?;
    let capture_result = capture_locked_mutations(&db, &manifest, config.samples).await;
    let release_result = release_mutation_lock(&db, &manifest.evidence_id).await;
    let runs = match capture_result {
        Ok(runs) => {
            release_result?;
            runs
        }
        Err(error) => {
            let _ = release_result;
            return Err(error);
        }
    };

    ensure_database_metadata_stable(&db, &database_metadata, "partition mutation evidence").await?;
    publish_mutation_artifact(&config.output_path, &runs)?;
    Ok(PartitionMutationCapture {
        evidence_id: manifest.evidence_id,
        output_path: config.output_path.clone(),
        runs: runs.len(),
        samples_per_run: config.samples,
    })
}

async fn capture_locked_mutations(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    samples: usize,
) -> Result<Vec<PartitionMutationRunEvidence>> {
    let transaction = db
        .begin()
        .await
        .context("failed to start partition mutation evidence transaction")?;
    transaction
        .execute_unprepared("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ;")
        .await
        .context("failed to pin repeatable-read mutation evidence snapshot")?;

    let capture_result = capture_mutations_in_transaction(&transaction, manifest, samples).await;
    let rollback_result = transaction.rollback().await;
    match capture_result {
        Ok(runs) => {
            rollback_result
                .context("failed to roll back partition mutation evidence transaction")?;
            Ok(runs)
        }
        Err(error) => {
            let _ = rollback_result;
            Err(error)
        }
    }
}

async fn capture_mutations_in_transaction(
    transaction: &DatabaseTransaction,
    manifest: &PreparedManifest,
    samples: usize,
) -> Result<Vec<PartitionMutationRunEvidence>> {
    ensure_relation_count_parity(
        transaction,
        "index_entities",
        &manifest.shadow_relations.entities.parent,
    )
    .await?;
    ensure_relation_count_parity(
        transaction,
        "index_links",
        &manifest.shadow_relations.links.parent,
    )
    .await?;

    let entity_anchors = load_entity_anchors(
        transaction,
        &manifest.shadow_relations.entities.parent,
        manifest.repetitions.mutation,
    )
    .await?;
    ensure!(
        !entity_anchors.is_empty(),
        "partition mutation evidence requires at least one matching canonical/shadow entity row"
    );
    let link_anchors = load_link_anchors(
        transaction,
        &manifest.shadow_relations.links.parent,
        manifest.repetitions.mutation,
    )
    .await?;
    let cases = build_mutation_cases(manifest, &entity_anchors, &link_anchors)?;
    ensure!(
        cases.len() == manifest.repetitions.mutation,
        "partition mutation runner did not build the exact manifest mutation run count"
    );

    let mut runs = Vec::with_capacity(cases.len());
    for case in &cases {
        runs.push(capture_mutation_case(transaction, manifest, case, samples).await?);
    }
    Ok(runs)
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
        .context("partition mutation session-setting query returned no row")?;
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
    let mut children = BTreeMap::new();
    for row in rows {
        let name: String = row.try_get("", "relname")?;
        let relispartition: bool = row.try_get("", "relispartition")?;
        let bound: String = row.try_get("", "bound")?;
        ensure!(relispartition, "shadow child {name} is not a partition");
        children.insert(name, bound.to_ascii_lowercase());
    }
    for (remainder, partition) in plan.partitions.iter().enumerate() {
        let bound = children
            .get(partition)
            .with_context(|| format!("shadow child {partition} is missing"))?;
        ensure!(
            bound.contains(&format!("modulus {}", manifest.modulus))
                && bound.contains(&format!("remainder {remainder}")),
            "shadow child {partition} has an unexpected bound: {bound}"
        );
    }
    Ok(())
}

async fn acquire_mutation_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        vec![format!("rustok-index-partition-mutation:{evidence_id}").into()],
    ))
    .await?
    .context("partition mutation advisory lock returned no row")?;
    Ok(())
}

async fn release_mutation_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock(hashtextextended($1, 0)) AS unlocked",
            vec![format!("rustok-index-partition-mutation:{evidence_id}").into()],
        ))
        .await?
        .context("partition mutation advisory unlock returned no row")?;
    let unlocked: bool = row.try_get("", "unlocked")?;
    ensure!(unlocked, "partition mutation advisory lock was not held");
    Ok(())
}

async fn ensure_relation_count_parity(
    transaction: &DatabaseTransaction,
    canonical: &str,
    shadow: &str,
) -> Result<()> {
    let sql = format!(
        "SELECT (SELECT count(*)::bigint FROM {}) AS canonical_rows, (SELECT count(*)::bigint FROM {}) AS shadow_rows",
        quote_identifier(canonical),
        quote_identifier(shadow),
    );
    let row = transaction
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .context("partition mutation relation count query returned no row")?;
    let canonical_rows: i64 = row.try_get("", "canonical_rows")?;
    let shadow_rows: i64 = row.try_get("", "shadow_rows")?;
    ensure!(
        canonical_rows == shadow_rows,
        "partition mutation relation count parity failed for {canonical} and {shadow}: {canonical_rows} != {shadow_rows}"
    );
    Ok(())
}

async fn load_entity_anchors(
    transaction: &DatabaseTransaction,
    shadow: &str,
    requested: usize,
) -> Result<Vec<EntityAnchor>> {
    let limit = i64::try_from(requested.max(1)).context("mutation run count exceeds i64")?;
    let sql = format!(
        concat!(
            "SELECT c.tenant_id::text AS tenant_id, c.module_name, c.entity_name, ",
            "c.schema_version, c.entity_id::text AS entity_id, c.locale_key ",
            "FROM index_entities c JOIN {} s ON s.tenant_id = c.tenant_id ",
            "AND s.module_name = c.module_name AND s.entity_name = c.entity_name ",
            "AND s.schema_version = c.schema_version AND s.entity_id = c.entity_id ",
            "AND s.locale_key = c.locale_key WHERE to_jsonb(c) = to_jsonb(s) ",
            "ORDER BY c.tenant_id, c.module_name, c.entity_name, c.schema_version, ",
            "c.locale_key, c.entity_id LIMIT $1"
        ),
        quote_identifier(shadow),
    );
    let rows = transaction
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![limit.into()],
        ))
        .await
        .context("failed to load deterministic matching entity mutation anchors")?;
    rows.into_iter()
        .map(|row| {
            Ok(EntityAnchor {
                tenant_id: row.try_get("", "tenant_id")?,
                module_name: row.try_get("", "module_name")?,
                entity_name: row.try_get("", "entity_name")?,
                schema_version: row.try_get("", "schema_version")?,
                entity_id: row.try_get("", "entity_id")?,
                locale_key: row.try_get("", "locale_key")?,
            })
        })
        .collect()
}

async fn load_link_anchors(
    transaction: &DatabaseTransaction,
    shadow: &str,
    requested: usize,
) -> Result<Vec<LinkAnchor>> {
    let limit = i64::try_from(requested.max(1)).context("mutation run count exceeds i64")?;
    let sql = format!(
        concat!(
            "SELECT c.tenant_id::text AS tenant_id, c.source_module, c.source_entity, ",
            "c.source_schema_version, c.source_entity_id::text AS source_entity_id, ",
            "c.source_locale_key, c.source_version::text AS source_version, c.link_name, c.ordinal ",
            "FROM index_links c JOIN {} s ON s.tenant_id = c.tenant_id ",
            "AND s.source_module = c.source_module AND s.source_entity = c.source_entity ",
            "AND s.source_schema_version = c.source_schema_version ",
            "AND s.source_entity_id = c.source_entity_id ",
            "AND s.source_locale_key = c.source_locale_key ",
            "AND s.source_version = c.source_version AND s.link_name = c.link_name ",
            "AND s.ordinal = c.ordinal WHERE to_jsonb(c) = to_jsonb(s) ",
            "ORDER BY c.tenant_id, c.source_module, c.source_entity, c.source_schema_version, ",
            "c.source_locale_key, c.source_entity_id, c.source_version, c.link_name, c.ordinal LIMIT $1"
        ),
        quote_identifier(shadow),
    );
    let rows = transaction
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![limit.into()],
        ))
        .await
        .context("failed to load deterministic matching link mutation anchors")?;
    rows.into_iter()
        .map(|row| {
            Ok(LinkAnchor {
                tenant_id: row.try_get("", "tenant_id")?,
                source_module: row.try_get("", "source_module")?,
                source_entity: row.try_get("", "source_entity")?,
                source_schema_version: row.try_get("", "source_schema_version")?,
                source_entity_id: row.try_get("", "source_entity_id")?,
                source_locale_key: row.try_get("", "source_locale_key")?,
                source_version: row.try_get("", "source_version")?,
                link_name: row.try_get("", "link_name")?,
                ordinal: row.try_get("", "ordinal")?,
            })
        })
        .collect()
}

fn build_mutation_cases(
    manifest: &PreparedManifest,
    entities: &[EntityAnchor],
    links: &[LinkAnchor],
) -> Result<Vec<MutationCase>> {
    ensure!(
        !entities.is_empty(),
        "entity mutation anchors must not be empty"
    );
    let mut cases = Vec::with_capacity(manifest.repetitions.mutation);
    for index in 0..manifest.repetitions.mutation {
        let ordinal = index + 1;
        if index % 2 == 1 && !links.is_empty() {
            cases.push(link_delete_case(
                ordinal,
                &links[index % links.len()],
                &manifest.shadow_relations.links.parent,
            ));
        } else {
            cases.push(entity_touch_case(
                ordinal,
                &entities[index % entities.len()],
                &manifest.shadow_relations.entities.parent,
            ));
        }
    }
    let names = cases
        .iter()
        .map(|case| case.name.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        names.len() == cases.len(),
        "partition mutation cases contain duplicate names"
    );
    Ok(cases)
}

fn entity_touch_case(ordinal: usize, anchor: &EntityAnchor, shadow: &str) -> MutationCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "UPDATE {} e SET updated_at = e.updated_at + INTERVAL '1 microsecond' ",
                "WHERE e.tenant_id = $1::uuid AND e.module_name = $2 AND e.entity_name = $3 ",
                "AND e.schema_version = $4::integer AND e.entity_id = $5::uuid ",
                "AND e.locale_key = $6 RETURNING 1::bigint AS affected"
            ),
            quote_identifier(relation),
        )
    };
    MutationCase {
        name: format!("entity-touch-{ordinal:03}"),
        template: "entity_touch_v1",
        baseline_sql: render("index_entities"),
        shadow_sql: render(shadow),
        values: vec![
            anchor.tenant_id.clone().into(),
            anchor.module_name.clone().into(),
            anchor.entity_name.clone().into(),
            anchor.schema_version.into(),
            anchor.entity_id.clone().into(),
            anchor.locale_key.clone().into(),
        ],
        target: MutationTarget::Entities,
    }
}

fn link_delete_case(ordinal: usize, anchor: &LinkAnchor, shadow: &str) -> MutationCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "DELETE FROM {} l WHERE l.tenant_id = $1::uuid AND l.source_module = $2 ",
                "AND l.source_entity = $3 AND l.source_schema_version = $4::integer ",
                "AND l.source_entity_id = $5::uuid AND l.source_locale_key = $6 ",
                "AND l.source_version = $7::numeric AND l.link_name = $8 ",
                "AND l.ordinal = $9::integer RETURNING 1::bigint AS affected"
            ),
            quote_identifier(relation),
        )
    };
    MutationCase {
        name: format!("link-delete-{ordinal:03}"),
        template: "link_delete_v1",
        baseline_sql: render("index_links"),
        shadow_sql: render(shadow),
        values: vec![
            anchor.tenant_id.clone().into(),
            anchor.source_module.clone().into(),
            anchor.source_entity.clone().into(),
            anchor.source_schema_version.into(),
            anchor.source_entity_id.clone().into(),
            anchor.source_locale_key.clone().into(),
            anchor.source_version.clone().into(),
            anchor.link_name.clone().into(),
            anchor.ordinal.into(),
        ],
        target: MutationTarget::Links,
    }
}

async fn capture_mutation_case(
    transaction: &DatabaseTransaction,
    manifest: &PreparedManifest,
    case: &MutationCase,
    samples: usize,
) -> Result<PartitionMutationRunEvidence> {
    let baseline_affected =
        validate_mutation_side(transaction, case, MutationSide::Baseline).await?;
    let shadow_affected = validate_mutation_side(transaction, case, MutationSide::Shadow).await?;
    ensure!(
        baseline_affected == 1 && shadow_affected == baseline_affected,
        "mutation {} did not affect exactly one matching row on both sides",
        case.name
    );

    let mut baseline_samples = Vec::with_capacity(samples);
    let mut shadow_samples = Vec::with_capacity(samples);
    for sample in 1..=samples {
        if sample % 2 == 1 {
            baseline_samples.push(
                explain_mutation_sample(
                    transaction,
                    manifest,
                    case,
                    MutationSide::Baseline,
                    sample,
                )
                .await?,
            );
            shadow_samples.push(
                explain_mutation_sample(transaction, manifest, case, MutationSide::Shadow, sample)
                    .await?,
            );
        } else {
            shadow_samples.push(
                explain_mutation_sample(transaction, manifest, case, MutationSide::Shadow, sample)
                    .await?,
            );
            baseline_samples.push(
                explain_mutation_sample(
                    transaction,
                    manifest,
                    case,
                    MutationSide::Baseline,
                    sample,
                )
                .await?,
            );
        }
    }

    ensure_stable_affected_rows(&baseline_samples, &case.name, "baseline")?;
    ensure_stable_affected_rows(&shadow_samples, &case.name, "shadow")?;
    let baseline_relation_reads = stable_relation_reads(&baseline_samples, &case.name, "baseline")?;
    let shadow_relation_reads = stable_relation_reads(&shadow_samples, &case.name, "shadow")?;
    let baseline_p95_ms = percentile_95(
        &baseline_samples
            .iter()
            .map(|sample| sample.execution_time_ms)
            .collect::<Vec<_>>(),
    )?;
    let shadow_p95_ms = percentile_95(
        &shadow_samples
            .iter()
            .map(|sample| sample.execution_time_ms)
            .collect::<Vec<_>>(),
    )?;
    let baseline_wal_bytes = maximum_wal_bytes(&baseline_samples)?;
    let shadow_wal_bytes = maximum_wal_bytes(&shadow_samples)?;

    Ok(PartitionMutationRunEvidence {
        name: case.name.clone(),
        template: case.template.to_owned(),
        sample_count: samples,
        affected_rows: baseline_affected,
        baseline_p95_ms,
        shadow_p95_ms,
        baseline_wal_bytes,
        shadow_wal_bytes,
        baseline_relation_reads,
        shadow_relation_reads,
        baseline_explain_samples: baseline_samples,
        shadow_explain_samples: shadow_samples,
    })
}

async fn validate_mutation_side(
    transaction: &DatabaseTransaction,
    case: &MutationCase,
    side: MutationSide,
) -> Result<u64> {
    begin_sample_savepoint(transaction).await?;
    let sql = mutation_sql(case, side);
    let result = transaction
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql.to_owned(),
            case.values.clone(),
        ))
        .await
        .with_context(|| format!("failed to validate mutation {}", case.name));
    let cleanup = rollback_sample_savepoint(transaction).await;
    match result {
        Ok(rows) => {
            cleanup?;
            for row in &rows {
                let affected: i64 = row.try_get("", "affected")?;
                ensure!(
                    affected == 1,
                    "mutation {} returned an invalid affected marker",
                    case.name
                );
            }
            u64::try_from(rows.len()).context("mutation affected-row count exceeds u64")
        }
        Err(error) => {
            let _ = cleanup;
            Err(error)
        }
    }
}

async fn explain_mutation_sample(
    transaction: &DatabaseTransaction,
    manifest: &PreparedManifest,
    case: &MutationCase,
    side: MutationSide,
    sample: usize,
) -> Result<MutationExplainSample> {
    begin_sample_savepoint(transaction).await?;
    let sql = mutation_sql(case, side);
    let result = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON) {sql}"),
            case.values.clone(),
        ))
        .await
        .with_context(|| format!("failed to execute mutation EXPLAIN for {}", case.name));
    let cleanup = rollback_sample_savepoint(transaction).await;
    let row = match result {
        Ok(row) => {
            cleanup?;
            row.context("partition mutation EXPLAIN returned no row")?
        }
        Err(error) => {
            let _ = cleanup;
            return Err(error);
        }
    };
    let explain: JsonValue = row
        .try_get("", "QUERY PLAN")
        .context("partition mutation EXPLAIN did not contain QUERY PLAN JSON")?;
    let metrics = parse_mutation_explain_metrics(&explain)
        .context("partition mutation EXPLAIN did not satisfy the WAL evidence contract")?;
    ensure!(
        metrics.execution_time_ms > 0.0,
        "partition mutation execution time must be greater than zero"
    );
    ensure!(
        metrics.maximum_node_wal_bytes > 0,
        "partition mutation EXPLAIN must report positive WAL bytes"
    );
    let affected_rows = explain_affected_rows(&explain)?;
    ensure!(
        affected_rows == 1,
        "mutation {} EXPLAIN affected {affected_rows} rows",
        case.name
    );
    let relation_reads = validate_plan_relations(&explain, manifest, case, side)?;
    Ok(MutationExplainSample {
        sample,
        execution_time_ms: metrics.execution_time_ms,
        affected_rows,
        maximum_node_wal_records: metrics.maximum_node_wal_records,
        maximum_node_wal_fpi: metrics.maximum_node_wal_fpi,
        maximum_node_wal_bytes: metrics.maximum_node_wal_bytes,
        shared_hit_blocks: metrics.shared_hit_blocks,
        shared_read_blocks: metrics.shared_read_blocks,
        relation_reads,
        explain,
    })
}

fn mutation_sql(case: &MutationCase, side: MutationSide) -> &str {
    match side {
        MutationSide::Baseline => &case.baseline_sql,
        MutationSide::Shadow => &case.shadow_sql,
    }
}

async fn begin_sample_savepoint(transaction: &DatabaseTransaction) -> Result<()> {
    transaction
        .execute_unprepared(&format!("SAVEPOINT {SAMPLE_SAVEPOINT};"))
        .await
        .context("failed to create partition mutation sample savepoint")?;
    Ok(())
}

async fn rollback_sample_savepoint(transaction: &DatabaseTransaction) -> Result<()> {
    transaction
        .execute_unprepared(&format!("ROLLBACK TO SAVEPOINT {SAMPLE_SAVEPOINT};"))
        .await
        .context("failed to roll back partition mutation sample")?;
    transaction
        .execute_unprepared(&format!("RELEASE SAVEPOINT {SAMPLE_SAVEPOINT};"))
        .await
        .context("failed to release partition mutation sample savepoint")?;
    Ok(())
}

fn explain_affected_rows(explain: &JsonValue) -> Result<u64> {
    let root = explain_root(explain)?;
    let plan = root
        .get("Plan")
        .and_then(JsonValue::as_object)
        .context("partition mutation EXPLAIN is missing Plan")?;
    non_negative_integer(plan.get("Actual Rows"), "Plan.Actual Rows")
}

fn non_negative_integer(value: Option<&JsonValue>, label: &str) -> Result<u64> {
    let value = value.with_context(|| format!("partition mutation EXPLAIN is missing {label}"))?;
    if let Some(integer) = value.as_u64() {
        return Ok(integer);
    }
    let number = value
        .as_f64()
        .with_context(|| format!("partition mutation EXPLAIN {label} must be numeric"))?;
    ensure!(
        number.is_finite() && number >= 0.0 && number.fract() == 0.0,
        "partition mutation EXPLAIN {label} must be a non-negative integer"
    );
    ensure!(
        number <= u64::MAX as f64,
        "partition mutation EXPLAIN {label} exceeds u64"
    );
    Ok(number as u64)
}

fn validate_plan_relations(
    explain: &JsonValue,
    manifest: &PreparedManifest,
    case: &MutationCase,
    side: MutationSide,
) -> Result<MutationRelationReads> {
    let mut names = BTreeSet::new();
    collect_relation_names(explain, &mut names);
    let (canonical, plan) = match case.target {
        MutationTarget::Entities => ("index_entities", &manifest.shadow_relations.entities),
        MutationTarget::Links => ("index_links", &manifest.shadow_relations.links),
    };
    let all_shadow = manifest
        .shadow_relations
        .entities
        .partitions
        .iter()
        .chain(manifest.shadow_relations.links.partitions.iter())
        .cloned()
        .chain([
            manifest.shadow_relations.entities.parent.clone(),
            manifest.shadow_relations.links.parent.clone(),
        ])
        .collect::<BTreeSet<_>>();

    match side {
        MutationSide::Baseline => {
            ensure!(
                names.contains(canonical),
                "baseline mutation {} did not access {canonical}",
                case.name
            );
            ensure!(
                names.is_disjoint(&all_shadow),
                "baseline mutation {} accessed a shadow relation",
                case.name
            );
            ensure!(
                names.iter().all(|name| name == canonical),
                "baseline mutation {} accessed an unexpected relation: {:?}",
                case.name,
                names
            );
            Ok(MutationRelationReads {
                target_relations: vec![canonical.to_owned()],
            })
        }
        MutationSide::Shadow => {
            ensure!(
                !names.contains("index_entities") && !names.contains("index_links"),
                "shadow mutation {} accessed a canonical relation",
                case.name
            );
            let children = plan.partitions.iter().cloned().collect::<BTreeSet<_>>();
            let touched_children = names.intersection(&children).cloned().collect::<Vec<_>>();
            ensure!(
                touched_children.len() == 1,
                "shadow mutation {} must prune its target to exactly one child partition",
                case.name
            );
            let allowed = children
                .into_iter()
                .chain([plan.parent.clone()])
                .collect::<BTreeSet<_>>();
            ensure!(
                names.is_subset(&allowed),
                "shadow mutation {} accessed a relation outside its manifest shadow plan: {:?}",
                case.name,
                names.difference(&allowed).collect::<Vec<_>>()
            );
            Ok(MutationRelationReads {
                target_relations: names.into_iter().collect(),
            })
        }
    }
}

fn collect_relation_names(value: &JsonValue, names: &mut BTreeSet<String>) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                collect_relation_names(value, names);
            }
        }
        JsonValue::Object(values) => {
            if let Some(name) = values.get("Relation Name").and_then(JsonValue::as_str) {
                names.insert(name.to_owned());
            }
            for value in values.values() {
                collect_relation_names(value, names);
            }
        }
        _ => {}
    }
}

fn explain_root(explain: &JsonValue) -> Result<&JsonMap<String, JsonValue>> {
    let entries = explain
        .as_array()
        .context("partition mutation EXPLAIN must be a JSON array")?;
    ensure!(
        entries.len() == 1,
        "partition mutation EXPLAIN must contain exactly one root entry"
    );
    entries[0]
        .as_object()
        .context("partition mutation EXPLAIN root must be an object")
}

fn ensure_stable_affected_rows(
    samples: &[MutationExplainSample],
    mutation: &str,
    side: &str,
) -> Result<()> {
    ensure!(
        samples.iter().all(|sample| sample.affected_rows == 1),
        "mutation {mutation} had unstable {side} affected-row evidence"
    );
    Ok(())
}

fn stable_relation_reads(
    samples: &[MutationExplainSample],
    mutation: &str,
    side: &str,
) -> Result<MutationRelationReads> {
    let first = samples
        .first()
        .context("partition mutation evidence sample set must not be empty")?
        .relation_reads
        .clone();
    ensure!(
        samples.iter().all(|sample| sample.relation_reads == first),
        "mutation {mutation} had unstable {side} relation access across samples"
    );
    Ok(first)
}

fn percentile_95(values: &[f64]) -> Result<f64> {
    ensure!(!values.is_empty(), "p95 sample set must not be empty");
    let mut sorted = values.to_vec();
    ensure!(
        sorted.iter().all(|value| value.is_finite() && *value > 0.0),
        "p95 samples must be finite and greater than zero"
    );
    sorted.sort_by(|left, right| left.total_cmp(right));
    let rank = ((sorted.len() * 95).div_ceil(100)).max(1) - 1;
    Ok(sorted[rank])
}

fn maximum_wal_bytes(samples: &[MutationExplainSample]) -> Result<u64> {
    let maximum = samples
        .iter()
        .map(|sample| sample.maximum_node_wal_bytes)
        .max()
        .context("partition mutation WAL sample set must not be empty")?;
    ensure!(
        maximum > 0,
        "partition mutation WAL evidence must be positive"
    );
    Ok(maximum)
}

fn ensure_output_available(path: &Path) -> Result<()> {
    ensure!(!path.exists(), "refusing to overwrite {path:?}");
    Ok(())
}

fn publish_mutation_artifact(path: &Path, runs: &[PartitionMutationRunEvidence]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create mutation evidence directory {parent:?}"))?;
    }
    ensure_output_available(path)?;
    let mut bytes = serde_json::to_vec_pretty(runs)
        .context("failed to serialize partition mutation evidence")?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| {
            format!("failed to create temporary mutation evidence file {temporary:?}")
        })?;
    file.write_all(&bytes).with_context(|| {
        format!("failed to write temporary mutation evidence file {temporary:?}")
    })?;
    file.sync_all().with_context(|| {
        format!("failed to sync temporary mutation evidence file {temporary:?}")
    })?;
    let publish = fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish mutation evidence to {path:?}"));
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
        !value.is_empty()
            && value.len() <= 63
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "invalid PostgreSQL identifier {value}"
    );
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
