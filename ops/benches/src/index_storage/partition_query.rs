use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SeaValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use sha2::{Digest, Sha256};

use super::{connect_benchmark_database, ensure_database_metadata_stable, read_database_metadata};

const MANIFEST_CONTRACT: &str = "index_partition_evidence_manifest_v1";
const SHADOW_PLAN_VERSION: &str = "tenant_hash_shadow_v1";
const PLAN_DIGEST_CONTRACT: &str = "normalized_partition_plan_v1";
const QUERY_OPT_IN: &str = "INDEX_PARTITION_ALLOW_QUERY_EVIDENCE";
const DEFAULT_QUERY_SAMPLES: usize = 7;
const MAX_QUERY_SAMPLES: usize = 100;

#[derive(Debug, Clone)]
pub struct PartitionQueryConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub output_path: PathBuf,
    pub samples: usize,
}

impl PartitionQueryConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(QUERY_OPT_IN).as_deref(), Ok("1")),
            "{QUERY_OPT_IN}=1 is required because the runner executes measured PostgreSQL queries"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition query evidence")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let output_path = env::var("INDEX_PARTITION_QUERY_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("INDEX_PARTITION_EVIDENCE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"))
                    .join("query.json")
            });
        let samples = env::var("INDEX_PARTITION_QUERY_SAMPLES")
            .unwrap_or_else(|_| DEFAULT_QUERY_SAMPLES.to_string())
            .parse::<usize>()
            .context("INDEX_PARTITION_QUERY_SAMPLES must be an integer")?;
        ensure!(
            (3..=MAX_QUERY_SAMPLES).contains(&samples),
            "INDEX_PARTITION_QUERY_SAMPLES must be between 3 and {MAX_QUERY_SAMPLES}"
        );
        ensure!(
            manifest_path != output_path,
            "manifest and query evidence output paths must be distinct"
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
    source_locale_key: String,
    link_name: String,
}

#[derive(Debug, Clone)]
struct QueryCase {
    name: String,
    template: &'static str,
    baseline_sql: String,
    shadow_sql: String,
    values: Vec<SeaValue>,
    logical_relations: Vec<&'static str>,
    logical_predicates: Vec<&'static str>,
    logical_ordering: Vec<&'static str>,
    uses_entities: bool,
    uses_links: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PlanRelationReads {
    entities: Vec<String>,
    links: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct QueryExplainSample {
    sample: usize,
    execution_time_ms: f64,
    plan_digest: String,
    relation_reads: PlanRelationReads,
    explain: JsonValue,
}

#[derive(Debug, Clone, Serialize)]
struct PartitionQueryRunEvidence {
    name: String,
    template: String,
    sample_count: usize,
    baseline_p95_ms: f64,
    shadow_p95_ms: f64,
    baseline_plan_digest: String,
    shadow_plan_digest: String,
    baseline_result_rows: i64,
    shadow_result_rows: i64,
    baseline_result_digest: String,
    shadow_result_digest: String,
    baseline_relation_reads: PlanRelationReads,
    shadow_relation_reads: PlanRelationReads,
    baseline_explain_samples: Vec<QueryExplainSample>,
    shadow_explain_samples: Vec<QueryExplainSample>,
}

#[derive(Debug, Clone)]
pub struct PartitionQueryCapture {
    pub evidence_id: String,
    pub output_path: PathBuf,
    pub runs: usize,
    pub samples_per_run: usize,
}

#[derive(Debug, Clone, Copy)]
enum QuerySide {
    Baseline,
    Shadow,
}

pub async fn capture_partition_query_evidence(
    config: &PartitionQueryConfig,
) -> Result<PartitionQueryCapture> {
    ensure_output_available(&config.output_path)?;
    let (manifest, raw_manifest) = read_manifest(&config.manifest_path)?;
    validate_manifest(&manifest, &raw_manifest)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared(
        "SET jit = off; SET lock_timeout = '5s'; SET statement_timeout = 0; SET enable_partition_pruning = on;",
    )
    .await
    .context("failed to pin partition query evidence session settings")?;
    let database_metadata = read_database_metadata(&db).await?;
    ensure!(
        database_metadata.server_version_num.starts_with("16"),
        "partition query evidence requires PostgreSQL 16, got {}",
        database_metadata.server_version_num
    );
    ensure!(
        database_metadata.jit == "off",
        "partition query evidence requires jit=off"
    );
    ensure_session_setting(&db, "enable_partition_pruning", "on").await?;
    ensure_unpartitioned_source(&db, "index_entities").await?;
    ensure_unpartitioned_source(&db, "index_links").await?;
    validate_shadow_catalog(&db, &manifest).await?;

    acquire_query_lock(&db, &manifest.evidence_id).await?;
    let capture_result = capture_locked_queries(&db, &manifest, config.samples).await;
    let release_result = release_query_lock(&db, &manifest.evidence_id).await;
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

    ensure_database_metadata_stable(&db, &database_metadata, "partition query evidence").await?;
    publish_query_artifact(&config.output_path, &runs)?;
    Ok(PartitionQueryCapture {
        evidence_id: manifest.evidence_id,
        output_path: config.output_path.clone(),
        runs: runs.len(),
        samples_per_run: config.samples,
    })
}

async fn capture_locked_queries(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    samples: usize,
) -> Result<Vec<PartitionQueryRunEvidence>> {
    let transaction = db
        .begin()
        .await
        .context("failed to start partition query evidence transaction")?;
    transaction
        .execute_unprepared("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY;")
        .await
        .context("failed to pin read-only repeatable-read query evidence snapshot")?;

    let entity_anchors = load_entity_anchors(&transaction, manifest.repetitions.query).await?;
    ensure!(
        !entity_anchors.is_empty(),
        "partition query evidence requires at least one canonical entity row"
    );
    let link_anchors = load_link_anchors(&transaction, manifest.repetitions.query).await?;
    let cases = build_query_cases(manifest, &entity_anchors, &link_anchors)?;
    ensure!(
        cases.len() == manifest.repetitions.query,
        "partition query runner did not build the exact manifest query run count"
    );

    let mut runs = Vec::with_capacity(cases.len());
    for case in &cases {
        runs.push(capture_query_case(&transaction, manifest, case, samples).await?);
    }
    transaction
        .commit()
        .await
        .context("failed to complete read-only partition query evidence transaction")?;
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
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT current_setting($1) AS value",
            vec![setting.into()],
        ))
        .await?
        .context("session-setting query returned no row")?;
    let actual: String = row.try_get("", "value")?;
    ensure!(
        actual == expected,
        "requires {setting}={expected}, got {actual}"
    );
    Ok(())
}

async fn ensure_unpartitioned_source(db: &DatabaseConnection, relation: &str) -> Result<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
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
        .query_one_raw(Statement::from_sql_and_values(
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
        .query_all_raw(Statement::from_sql_and_values(
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

async fn acquire_query_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        vec![format!("rustok-index-partition-query:{evidence_id}").into()],
    ))
    .await?
    .context("query advisory lock returned no row")?;
    Ok(())
}

async fn release_query_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock(hashtextextended($1, 0)) AS unlocked",
            vec![format!("rustok-index-partition-query:{evidence_id}").into()],
        ))
        .await?
        .context("query advisory unlock returned no row")?;
    let unlocked: bool = row.try_get("", "unlocked")?;
    ensure!(unlocked, "query advisory lock was not held");
    Ok(())
}

async fn load_entity_anchors<C: ConnectionTrait>(
    db: &C,
    requested: usize,
) -> Result<Vec<EntityAnchor>> {
    let limit = i64::try_from(requested.max(1)).context("query run count exceeds i64")?;
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT tenant_id::text AS tenant_id, module_name, entity_name, schema_version, ",
                "entity_id::text AS entity_id, locale_key FROM index_entities ",
                "ORDER BY tenant_id, module_name, entity_name, schema_version, locale_key, entity_id LIMIT $1"
            ),
            vec![limit.into()],
        ))
        .await
        .context("failed to load entity query anchors")?;
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

async fn load_link_anchors<C: ConnectionTrait>(
    db: &C,
    requested: usize,
) -> Result<Vec<LinkAnchor>> {
    let limit = i64::try_from(requested.max(1)).context("query run count exceeds i64")?;
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT tenant_id::text AS tenant_id, source_module, source_entity, ",
                "source_schema_version, source_locale_key, link_name FROM index_links ",
                "ORDER BY tenant_id, source_module, source_entity, source_schema_version, ",
                "source_locale_key, link_name, source_entity_id, source_version, ordinal LIMIT $1"
            ),
            vec![limit.into()],
        ))
        .await
        .context("failed to load link query anchors")?;
    rows.into_iter()
        .map(|row| {
            Ok(LinkAnchor {
                tenant_id: row.try_get("", "tenant_id")?,
                source_module: row.try_get("", "source_module")?,
                source_entity: row.try_get("", "source_entity")?,
                source_schema_version: row.try_get("", "source_schema_version")?,
                source_locale_key: row.try_get("", "source_locale_key")?,
                link_name: row.try_get("", "link_name")?,
            })
        })
        .collect()
}

fn build_query_cases(
    manifest: &PreparedManifest,
    entities: &[EntityAnchor],
    links: &[LinkAnchor],
) -> Result<Vec<QueryCase>> {
    ensure!(
        !entities.is_empty(),
        "entity query anchors must not be empty"
    );
    let template_count = if links.is_empty() { 3 } else { 5 };
    let mut cases = Vec::with_capacity(manifest.repetitions.query);
    for index in 0..manifest.repetitions.query {
        let ordinal = index + 1;
        let case = match index % template_count {
            0 => entity_scope_case(
                ordinal,
                &entities[index % entities.len()],
                &manifest.shadow_relations.entities.parent,
            ),
            1 => entity_keyset_case(
                ordinal,
                &entities[index % entities.len()],
                &manifest.shadow_relations.entities.parent,
            ),
            2 => entity_count_case(
                ordinal,
                &entities[index % entities.len()],
                &manifest.shadow_relations.entities.parent,
            ),
            3 => link_scope_case(
                ordinal,
                &links[index % links.len()],
                &manifest.shadow_relations.links.parent,
            ),
            4 => source_link_join_case(
                ordinal,
                &links[index % links.len()],
                &manifest.shadow_relations.entities.parent,
                &manifest.shadow_relations.links.parent,
            ),
            _ => unreachable!("query template sequence is bounded"),
        };
        cases.push(case);
    }
    Ok(cases)
}

fn entity_scope_case(ordinal: usize, anchor: &EntityAnchor, shadow: &str) -> QueryCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "SELECT e.entity_id::text AS entity_id, e.source_version::text AS source_version, ",
                "e.payload, e.is_deleted FROM {} e WHERE e.tenant_id = $1::uuid ",
                "AND e.module_name = $2 AND e.entity_name = $3 AND e.schema_version = $4::integer ",
                "AND e.locale_key = $5 ORDER BY e.entity_id LIMIT 100"
            ),
            quote_identifier(relation),
        )
    };
    QueryCase {
        name: format!("entity-scope-page-{ordinal:03}"),
        template: "entity_scope_page_v1",
        baseline_sql: render("index_entities"),
        shadow_sql: render(shadow),
        values: entity_scope_values(anchor),
        logical_relations: vec!["entities"],
        logical_predicates: vec![
            "tenant_id = ?",
            "module_name = ?",
            "entity_name = ?",
            "schema_version = ?",
            "locale_key = ?",
        ],
        logical_ordering: vec!["entity_id ASC"],
        uses_entities: true,
        uses_links: false,
    }
}

fn entity_keyset_case(ordinal: usize, anchor: &EntityAnchor, shadow: &str) -> QueryCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "SELECT e.entity_id::text AS entity_id, e.source_version::text AS source_version, ",
                "e.payload, e.is_deleted FROM {} e WHERE e.tenant_id = $1::uuid ",
                "AND e.module_name = $2 AND e.entity_name = $3 AND e.schema_version = $4::integer ",
                "AND e.locale_key = $5 AND e.entity_id >= $6::uuid ",
                "ORDER BY e.entity_id LIMIT 100"
            ),
            quote_identifier(relation),
        )
    };
    let mut values = entity_scope_values(anchor);
    values.push(anchor.entity_id.clone().into());
    QueryCase {
        name: format!("entity-keyset-page-{ordinal:03}"),
        template: "entity_keyset_page_v1",
        baseline_sql: render("index_entities"),
        shadow_sql: render(shadow),
        values,
        logical_relations: vec!["entities"],
        logical_predicates: vec![
            "tenant_id = ?",
            "module_name = ?",
            "entity_name = ?",
            "schema_version = ?",
            "locale_key = ?",
            "entity_id >= ?",
        ],
        logical_ordering: vec!["entity_id ASC"],
        uses_entities: true,
        uses_links: false,
    }
}

fn entity_count_case(ordinal: usize, anchor: &EntityAnchor, shadow: &str) -> QueryCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "SELECT count(*)::bigint AS result_count FROM {} e ",
                "WHERE e.tenant_id = $1::uuid AND e.module_name = $2 AND e.entity_name = $3 ",
                "AND e.schema_version = $4::integer AND e.locale_key = $5"
            ),
            quote_identifier(relation),
        )
    };
    QueryCase {
        name: format!("entity-exact-count-{ordinal:03}"),
        template: "entity_exact_count_v1",
        baseline_sql: render("index_entities"),
        shadow_sql: render(shadow),
        values: entity_scope_values(anchor),
        logical_relations: vec!["entities"],
        logical_predicates: vec![
            "tenant_id = ?",
            "module_name = ?",
            "entity_name = ?",
            "schema_version = ?",
            "locale_key = ?",
        ],
        logical_ordering: vec![],
        uses_entities: true,
        uses_links: false,
    }
}

fn link_scope_case(ordinal: usize, anchor: &LinkAnchor, shadow: &str) -> QueryCase {
    let render = |relation: &str| {
        format!(
            concat!(
                "SELECT l.source_entity_id::text AS source_entity_id, ",
                "l.source_version::text AS source_version, l.link_name, l.ordinal, ",
                "l.target_module, l.target_entity, l.target_schema_version, ",
                "l.target_entity_id::text AS target_entity_id, l.target_locale_key ",
                "FROM {} l WHERE l.tenant_id = $1::uuid AND l.source_module = $2 ",
                "AND l.source_entity = $3 AND l.source_schema_version = $4::integer ",
                "AND l.source_locale_key = $5 AND l.link_name = $6 ",
                "ORDER BY l.source_entity_id, l.source_version, l.ordinal LIMIT 100"
            ),
            quote_identifier(relation),
        )
    };
    QueryCase {
        name: format!("link-scope-page-{ordinal:03}"),
        template: "link_scope_page_v1",
        baseline_sql: render("index_links"),
        shadow_sql: render(shadow),
        values: link_scope_values(anchor),
        logical_relations: vec!["links"],
        logical_predicates: vec![
            "tenant_id = ?",
            "source_module = ?",
            "source_entity = ?",
            "source_schema_version = ?",
            "source_locale_key = ?",
            "link_name = ?",
        ],
        logical_ordering: vec!["source_entity_id ASC", "source_version ASC", "ordinal ASC"],
        uses_entities: false,
        uses_links: true,
    }
}

fn source_link_join_case(
    ordinal: usize,
    anchor: &LinkAnchor,
    entity_shadow: &str,
    link_shadow: &str,
) -> QueryCase {
    let render = |entities: &str, links: &str| {
        format!(
            concat!(
                "SELECT e.entity_id::text AS entity_id, e.source_version::text AS source_version, ",
                "l.link_name, l.ordinal, l.target_module, l.target_entity, ",
                "l.target_schema_version, l.target_entity_id::text AS target_entity_id, ",
                "l.target_locale_key FROM {} e JOIN {} l ON l.tenant_id = e.tenant_id ",
                "AND l.source_module = e.module_name AND l.source_entity = e.entity_name ",
                "AND l.source_schema_version = e.schema_version AND l.source_entity_id = e.entity_id ",
                "AND l.source_locale_key = e.locale_key AND l.source_version = e.source_version ",
                "WHERE e.tenant_id = $1::uuid AND e.module_name = $2 AND e.entity_name = $3 ",
                "AND e.schema_version = $4::integer AND e.locale_key = $5 AND l.link_name = $6 ",
                "ORDER BY e.entity_id, l.source_version, l.ordinal LIMIT 100"
            ),
            quote_identifier(entities),
            quote_identifier(links),
        )
    };
    QueryCase {
        name: format!("source-link-join-{ordinal:03}"),
        template: "source_link_join_v1",
        baseline_sql: render("index_entities", "index_links"),
        shadow_sql: render(entity_shadow, link_shadow),
        values: link_scope_values(anchor),
        logical_relations: vec!["entities", "links"],
        logical_predicates: vec![
            "tenant_id = ?",
            "module_name = source_module",
            "entity_name = source_entity",
            "schema_version = source_schema_version",
            "entity_id = source_entity_id",
            "locale_key = source_locale_key",
            "source_version = link_source_version",
            "link_name = ?",
        ],
        logical_ordering: vec!["entity_id ASC", "source_version ASC", "ordinal ASC"],
        uses_entities: true,
        uses_links: true,
    }
}

fn entity_scope_values(anchor: &EntityAnchor) -> Vec<SeaValue> {
    vec![
        anchor.tenant_id.clone().into(),
        anchor.module_name.clone().into(),
        anchor.entity_name.clone().into(),
        anchor.schema_version.into(),
        anchor.locale_key.clone().into(),
    ]
}

fn link_scope_values(anchor: &LinkAnchor) -> Vec<SeaValue> {
    vec![
        anchor.tenant_id.clone().into(),
        anchor.source_module.clone().into(),
        anchor.source_entity.clone().into(),
        anchor.source_schema_version.into(),
        anchor.source_locale_key.clone().into(),
        anchor.link_name.clone().into(),
    ]
}

async fn capture_query_case<C: ConnectionTrait>(
    db: &C,
    manifest: &PreparedManifest,
    case: &QueryCase,
    samples: usize,
) -> Result<PartitionQueryRunEvidence> {
    let (baseline_result_rows, baseline_result_digest) =
        result_digest(db, &case.baseline_sql, &case.values).await?;
    let (shadow_result_rows, shadow_result_digest) =
        result_digest(db, &case.shadow_sql, &case.values).await?;
    ensure!(
        baseline_result_rows == shadow_result_rows
            && baseline_result_digest == shadow_result_digest,
        "query {} produced different baseline and shadow results",
        case.name
    );

    let mut baseline_samples = Vec::with_capacity(samples);
    let mut shadow_samples = Vec::with_capacity(samples);
    for sample in 1..=samples {
        if sample % 2 == 1 {
            baseline_samples
                .push(explain_sample(db, manifest, case, QuerySide::Baseline, sample).await?);
            shadow_samples
                .push(explain_sample(db, manifest, case, QuerySide::Shadow, sample).await?);
        } else {
            shadow_samples
                .push(explain_sample(db, manifest, case, QuerySide::Shadow, sample).await?);
            baseline_samples
                .push(explain_sample(db, manifest, case, QuerySide::Baseline, sample).await?);
        }
    }

    let baseline_plan_digest = stable_plan_digest(&baseline_samples, &case.name, "baseline")?;
    let shadow_plan_digest = stable_plan_digest(&shadow_samples, &case.name, "shadow")?;
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

    Ok(PartitionQueryRunEvidence {
        name: case.name.clone(),
        template: case.template.to_owned(),
        sample_count: samples,
        baseline_p95_ms,
        shadow_p95_ms,
        baseline_plan_digest,
        shadow_plan_digest,
        baseline_result_rows,
        shadow_result_rows,
        baseline_result_digest,
        shadow_result_digest,
        baseline_relation_reads,
        shadow_relation_reads,
        baseline_explain_samples: baseline_samples,
        shadow_explain_samples: shadow_samples,
    })
}

async fn result_digest<C: ConnectionTrait>(
    db: &C,
    sql: &str,
    values: &[SeaValue],
) -> Result<(i64, String)> {
    let wrapped = format!("SELECT row_to_json(result)::text AS result_json FROM ({sql}) AS result");
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            wrapped,
            values.to_vec(),
        ))
        .await
        .context("partition query result digest statement failed")?;
    let row_count = i64::try_from(rows.len()).context("query result row count exceeds i64")?;
    let mut payload = Vec::new();
    for row in rows {
        let value: String = row
            .try_get("", "result_json")
            .context("query result digest row did not contain result_json")?;
        payload.extend_from_slice(value.len().to_string().as_bytes());
        payload.push(b':');
        payload.extend_from_slice(value.as_bytes());
    }
    Ok((row_count, sha256_hex(&payload)))
}

async fn explain_sample<C: ConnectionTrait>(
    db: &C,
    manifest: &PreparedManifest,
    case: &QueryCase,
    side: QuerySide,
    sample: usize,
) -> Result<QueryExplainSample> {
    let sql = match side {
        QuerySide::Baseline => &case.baseline_sql,
        QuerySide::Shadow => &case.shadow_sql,
    };
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON) {sql}"),
            case.values.clone(),
        ))
        .await
        .with_context(|| format!("failed to execute EXPLAIN for query {}", case.name))?
        .context("partition query EXPLAIN returned no row")?;
    let explain: JsonValue = row
        .try_get("", "QUERY PLAN")
        .context("partition query EXPLAIN did not contain QUERY PLAN JSON")?;
    Ok(QueryExplainSample {
        sample,
        execution_time_ms: explain_execution_time(&explain)?,
        plan_digest: normalized_plan_digest(&explain, case)?,
        relation_reads: validate_plan_relations(&explain, manifest, case, side)?,
        explain,
    })
}

fn explain_execution_time(explain: &JsonValue) -> Result<f64> {
    let value = explain_root(explain)?
        .get("Execution Time")
        .and_then(JsonValue::as_f64)
        .context("EXPLAIN is missing numeric Execution Time")?;
    ensure!(
        value.is_finite() && value >= 0.0,
        "EXPLAIN Execution Time must be non-negative"
    );
    Ok(value)
}

fn normalized_plan_digest(explain: &JsonValue, case: &QueryCase) -> Result<String> {
    let plan = explain_root(explain)?
        .get("Plan")
        .context("EXPLAIN is missing Plan")?;
    let normalized = json!({
        "contract": PLAN_DIGEST_CONTRACT,
        "template": case.template,
        "tenant_hash_pruning_key": "tenant_id",
        "relations": &case.logical_relations,
        "predicates": &case.logical_predicates,
        "ordering": &case.logical_ordering,
        "plan": normalize_plan_node(plan)?,
    });
    Ok(sha256_hex(&canonical_json_bytes(&normalized)?))
}

fn normalize_plan_node(value: &JsonValue) -> Result<JsonValue> {
    let node = value.as_object().context("plan node must be an object")?;
    let node_type = node
        .get("Node Type")
        .and_then(JsonValue::as_str)
        .context("plan node is missing Node Type")?;
    let children = node
        .get("Plans")
        .map(|plans| {
            plans
                .as_array()
                .context("plan Plans must be an array")?
                .iter()
                .map(normalize_plan_node)
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    if node_type == "Subquery Scan" && children.len() == 1 {
        return Ok(children.into_iter().next().expect("one child was checked"));
    }
    if is_scan_node(node_type) {
        return Ok(json!({ "operator": "scan" }));
    }
    if node_type.ends_with("Append") {
        let mut unique = BTreeMap::new();
        for child in children {
            let key = String::from_utf8(canonical_json_bytes(&child)?)
                .context("canonical normalized plan was not UTF-8")?;
            unique.insert(key, child);
        }
        let mut children = unique.into_values().collect::<Vec<_>>();
        if children.len() == 1 {
            return Ok(children.remove(0));
        }
        return Ok(json!({ "operator": "append", "children": children }));
    }

    let mut normalized = JsonMap::new();
    normalized.insert(
        "operator".to_owned(),
        JsonValue::String(normalize_operator(node_type)),
    );
    for (source, target) in [
        ("Join Type", "join_type"),
        ("Strategy", "strategy"),
        ("Partial Mode", "partial_mode"),
    ] {
        if let Some(value) = node.get(source).and_then(JsonValue::as_str) {
            normalized.insert(
                target.to_owned(),
                JsonValue::String(value.to_ascii_lowercase()),
            );
        }
    }
    if !children.is_empty() {
        normalized.insert("children".to_owned(), JsonValue::Array(children));
    }
    Ok(JsonValue::Object(normalized))
}

fn is_scan_node(node_type: &str) -> bool {
    node_type.contains("Scan") || matches!(node_type, "BitmapAnd" | "BitmapOr")
}

fn normalize_operator(node_type: &str) -> String {
    node_type
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn validate_plan_relations(
    explain: &JsonValue,
    manifest: &PreparedManifest,
    case: &QueryCase,
    side: QuerySide,
) -> Result<PlanRelationReads> {
    let mut names = BTreeSet::new();
    collect_relation_names(explain, &mut names);
    let entity_children = manifest
        .shadow_relations
        .entities
        .partitions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let link_children = manifest
        .shadow_relations
        .links
        .partitions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let all_shadow = entity_children
        .union(&link_children)
        .cloned()
        .chain([
            manifest.shadow_relations.entities.parent.clone(),
            manifest.shadow_relations.links.parent.clone(),
        ])
        .collect::<BTreeSet<_>>();

    match side {
        QuerySide::Baseline => {
            ensure!(
                names.is_disjoint(&all_shadow),
                "baseline query {} accessed a shadow relation",
                case.name
            );
            ensure!(
                !case.uses_entities || names.contains("index_entities"),
                "baseline query {} did not access index_entities",
                case.name
            );
            ensure!(
                !case.uses_links || names.contains("index_links"),
                "baseline query {} did not access index_links",
                case.name
            );
            Ok(PlanRelationReads {
                entities: names
                    .contains("index_entities")
                    .then(|| "index_entities".to_owned())
                    .into_iter()
                    .collect(),
                links: names
                    .contains("index_links")
                    .then(|| "index_links".to_owned())
                    .into_iter()
                    .collect(),
            })
        }
        QuerySide::Shadow => {
            ensure!(
                !names.contains("index_entities") && !names.contains("index_links"),
                "shadow query {} accessed a canonical relation",
                case.name
            );
            let entities = names
                .intersection(&entity_children)
                .cloned()
                .collect::<Vec<_>>();
            let links = names
                .intersection(&link_children)
                .cloned()
                .collect::<Vec<_>>();
            let expected_entities = if case.uses_entities { 1 } else { 0 };
            let expected_links = if case.uses_links { 1 } else { 0 };
            ensure!(
                entities.len() == expected_entities,
                "shadow query {} must prune entities to exactly one child when used",
                case.name
            );
            ensure!(
                links.len() == expected_links,
                "shadow query {} must prune links to exactly one child when used",
                case.name
            );
            let allowed = entity_children
                .union(&link_children)
                .cloned()
                .collect::<BTreeSet<_>>();
            ensure!(
                names.is_subset(&allowed),
                "shadow query {} accessed a relation outside the evidence plan: {:?}",
                case.name,
                names.difference(&allowed).collect::<Vec<_>>()
            );
            Ok(PlanRelationReads { entities, links })
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
    let entries = explain.as_array().context("EXPLAIN must be a JSON array")?;
    ensure!(
        entries.len() == 1,
        "EXPLAIN must contain exactly one root entry"
    );
    entries[0]
        .as_object()
        .context("EXPLAIN root must be an object")
}

fn stable_plan_digest(samples: &[QueryExplainSample], query: &str, side: &str) -> Result<String> {
    let digests = samples
        .iter()
        .map(|sample| sample.plan_digest.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        digests.len() == 1,
        "query {query} had unstable {side} normalized plans across samples"
    );
    Ok(digests.into_iter().next().expect("one digest was checked"))
}

fn stable_relation_reads(
    samples: &[QueryExplainSample],
    query: &str,
    side: &str,
) -> Result<PlanRelationReads> {
    let first = samples
        .first()
        .context("query evidence sample set must not be empty")?
        .relation_reads
        .clone();
    ensure!(
        samples.iter().all(|sample| sample.relation_reads == first),
        "query {query} had unstable {side} relation reads across samples"
    );
    Ok(first)
}

fn percentile_95(values: &[f64]) -> Result<f64> {
    ensure!(!values.is_empty(), "p95 sample set must not be empty");
    let mut sorted = values.to_vec();
    ensure!(
        sorted
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0),
        "p95 samples must be finite and non-negative"
    );
    sorted.sort_by(|left, right| left.total_cmp(right));
    let rank = ((sorted.len() * 95).div_ceil(100)).max(1) - 1;
    let value = sorted[rank];
    ensure!(value > 0.0, "measured p95 must be greater than zero");
    Ok(value)
}

fn ensure_output_available(path: &Path) -> Result<()> {
    ensure!(!path.exists(), "refusing to overwrite {path:?}");
    Ok(())
}

fn publish_query_artifact(path: &Path, runs: &[PartitionQueryRunEvidence]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create query evidence directory {parent:?}"))?;
    }
    ensure_output_available(path)?;
    let mut bytes =
        serde_json::to_vec_pretty(runs).context("failed to serialize partition query evidence")?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("failed to create temporary query evidence file {temporary:?}"))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write temporary query evidence file {temporary:?}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync temporary query evidence file {temporary:?}"))?;
    let publish = fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish query evidence to {path:?}"));
    let _ = fs::remove_file(&temporary);
    publish
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(value)
}

fn canonical_json_bytes(value: &JsonValue) -> Result<Vec<u8>> {
    serde_json::to_vec(&canonicalize_json(value))
        .context("failed to serialize canonical partition query JSON")
}

fn canonicalize_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(values) => {
            JsonValue::Array(values.iter().map(canonicalize_json).collect())
        }
        JsonValue::Object(values) => {
            let mut sorted = JsonMap::new();
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), canonicalize_json(&values[key]));
            }
            JsonValue::Object(sorted)
        }
        _ => value.clone(),
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
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

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn case() -> QueryCase {
        QueryCase {
            name: "fixture-001".to_owned(),
            template: "entity_scope_page_v1",
            baseline_sql: String::new(),
            shadow_sql: String::new(),
            values: vec![],
            logical_relations: vec!["entities"],
            logical_predicates: vec!["tenant_id = ?"],
            logical_ordering: vec!["entity_id ASC"],
            uses_entities: true,
            uses_links: false,
        }
    }

    #[test]
    fn partition_append_and_unpartitioned_scan_share_logical_digest() {
        let baseline = json!([{
            "Execution Time": 1.0,
            "Plan": {"Node Type": "Index Scan", "Relation Name": "index_entities"}
        }]);
        let shadow = json!([{
            "Execution Time": 1.1,
            "Plan": {
                "Node Type": "Append",
                "Plans": [{"Node Type": "Seq Scan", "Relation Name": "shadow_p003"}]
            }
        }]);
        assert_eq!(
            normalized_plan_digest(&baseline, &case()).unwrap(),
            normalized_plan_digest(&shadow, &case()).unwrap()
        );
    }

    #[test]
    fn join_algorithm_change_changes_normalized_digest() {
        let nested = json!([{
            "Execution Time": 1.0,
            "Plan": {
                "Node Type": "Nested Loop",
                "Join Type": "Inner",
                "Plans": [{"Node Type": "Seq Scan"}, {"Node Type": "Index Scan"}]
            }
        }]);
        let hash = json!([{
            "Execution Time": 1.0,
            "Plan": {
                "Node Type": "Hash Join",
                "Join Type": "Inner",
                "Plans": [
                    {"Node Type": "Seq Scan"},
                    {"Node Type": "Hash", "Plans": [{"Node Type": "Seq Scan"}]}
                ]
            }
        }]);
        assert_ne!(
            normalized_plan_digest(&nested, &case()).unwrap(),
            normalized_plan_digest(&hash, &case()).unwrap()
        );
    }

    #[test]
    fn p95_uses_nearest_rank_and_requires_positive_measurement() {
        assert_eq!(percentile_95(&[1.0, 2.0, 3.0]).unwrap(), 3.0);
        assert!(percentile_95(&[0.0, 0.0, 0.0]).is_err());
    }

    #[test]
    fn generated_queries_remain_tenant_scoped_and_read_only() {
        let anchor = EntityAnchor {
            tenant_id: "00000000-0000-0000-0000-000000000001".to_owned(),
            module_name: "catalog".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: 1,
            entity_id: "00000000-0000-0000-0000-000000000002".to_owned(),
            locale_key: "en-US".to_owned(),
        };
        let query = entity_scope_case(1, &anchor, "index_entities_shadow_fixture");
        for sql in [&query.baseline_sql, &query.shadow_sql] {
            assert!(sql.contains("tenant_id = $1::uuid"));
            assert!(sql.contains("ORDER BY e.entity_id LIMIT 100"));
            for forbidden in ["INSERT", "UPDATE", "DELETE", "ALTER", "DROP", "RENAME"] {
                assert!(!sql.contains(forbidden), "query contains {forbidden}");
            }
        }
    }
}
