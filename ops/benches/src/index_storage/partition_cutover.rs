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
const RELATION_DIGEST_CONTRACT: &str = "index_partition_cutover_relation_v1";
const CUTOVER_OPT_IN: &str = "INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE";

#[derive(Debug, Clone)]
pub struct PartitionCutoverConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub output_path: PathBuf,
}

impl PartitionCutoverConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(CUTOVER_OPT_IN).as_deref(), Ok("1")),
            "{CUTOVER_OPT_IN}=1 is required because the runner acquires production-grade locks while keeping rename rehearsal inside evidence-only clones"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition cutover evidence")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let output_path = env::var("INDEX_PARTITION_CUTOVER_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("INDEX_PARTITION_EVIDENCE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"))
                    .join("cutover.json")
            });
        ensure!(
            manifest_path != output_path,
            "manifest and cutover evidence output paths must be distinct"
        );
        Ok(Self {
            database_url,
            manifest_path,
            output_path,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ChildCatalogEvidence {
    oid: i64,
    name: String,
    bound: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RelationCatalogEvidence {
    oid: i64,
    name: String,
    relkind: String,
    relispartition: bool,
    partitioned: bool,
    comment: Option<String>,
    children: Vec<ChildCatalogEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ProductionSnapshot {
    canonical_entities: LogicalRelationEvidence,
    canonical_links: LogicalRelationEvidence,
    shadow_entities: LogicalRelationEvidence,
    shadow_links: LogicalRelationEvidence,
    canonical_entities_catalog: RelationCatalogEvidence,
    canonical_links_catalog: RelationCatalogEvidence,
    shadow_entities_catalog: RelationCatalogEvidence,
    shadow_links_catalog: RelationCatalogEvidence,
}

#[derive(Debug, Clone)]
struct CutoverLayout {
    schema: String,
    canonical_entities: String,
    canonical_links: String,
    shadow_entities: String,
    shadow_links: String,
}

impl CutoverLayout {
    fn derive(manifest: &PreparedManifest) -> Result<Self> {
        let layout = Self {
            schema: format!("index_pe_cutover_{}", &manifest.evidence_id[..16]),
            canonical_entities: "canonical_entities".to_owned(),
            canonical_links: "canonical_links".to_owned(),
            shadow_entities: "shadow_entities".to_owned(),
            shadow_links: "shadow_links".to_owned(),
        };
        for identifier in [
            &layout.schema,
            &layout.canonical_entities,
            &layout.canonical_links,
            &layout.shadow_entities,
            &layout.shadow_links,
        ] {
            validate_identifier(identifier)?;
        }
        Ok(layout)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CloneIdentities {
    canonical_entities: i64,
    canonical_links: i64,
    shadow_entities: i64,
    shadow_links: i64,
}

#[derive(Debug, Clone, Serialize)]
struct PartitionCutoverRunEvidence {
    name: String,
    lock_ms: u64,
    rollback_verified: bool,
    production_relations_unchanged: bool,
    lock_mode: &'static str,
    locked_relations: Vec<String>,
    rehearsal_schema: String,
    clone_identities_before: CloneIdentities,
    clone_identities_during_swap: CloneIdentities,
    clone_identities_after_rollback: CloneIdentities,
}

#[derive(Debug, Clone)]
pub struct PartitionCutoverCapture {
    pub evidence_id: String,
    pub output_path: PathBuf,
    pub schema: String,
    pub runs: usize,
}

pub async fn capture_partition_cutover_evidence(
    config: &PartitionCutoverConfig,
) -> Result<PartitionCutoverCapture> {
    ensure_output_available(&config.output_path)?;
    let (manifest, raw_manifest) = read_manifest(&config.manifest_path)?;
    validate_manifest(&manifest, &raw_manifest)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared(
        "SET jit = off; SET lock_timeout = '5s'; SET statement_timeout = 0; SET enable_partition_pruning = on; SET synchronous_commit = on;",
    )
    .await
    .context("failed to pin partition cutover evidence session settings")?;
    let database_metadata = read_database_metadata(&db).await?;
    ensure!(
        database_metadata.server_version_num.starts_with("16"),
        "partition cutover evidence requires PostgreSQL 16, got {}",
        database_metadata.server_version_num
    );
    ensure!(
        database_metadata.jit == "off",
        "partition cutover evidence requires jit=off"
    );
    ensure_session_setting(&db, "enable_partition_pruning", "on").await?;
    ensure_session_setting(&db, "synchronous_commit", "on").await?;
    validate_source_catalog(&db, &manifest).await?;

    acquire_cutover_lock(&db, &manifest.evidence_id).await?;
    let capture_result = capture_locked_cutover(&db, &manifest).await;
    let release_result = release_cutover_lock(&db, &manifest.evidence_id).await;
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

    ensure_database_metadata_stable(&db, &database_metadata, "partition cutover evidence").await?;
    publish_cutover_artifact(&config.output_path, &runs)?;
    Ok(PartitionCutoverCapture {
        evidence_id: manifest.evidence_id,
        output_path: config.output_path.clone(),
        schema: layout.schema,
        runs: runs.len(),
    })
}

async fn capture_locked_cutover(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<(CutoverLayout, Vec<PartitionCutoverRunEvidence>)> {
    let source_before = production_snapshot(db, manifest).await?;
    ensure_source_parity(&source_before)?;

    let layout = CutoverLayout::derive(manifest)?;
    ensure_schema_absent(db, &layout.schema).await?;
    create_rehearsal_clones(db, manifest, &layout).await?;
    let clones_before = read_clone_identities(db, &layout).await?;
    ensure_source_unchanged(db, manifest, &source_before).await?;

    let mut runs = Vec::with_capacity(manifest.repetitions.cutover);
    for ordinal in 1..=manifest.repetitions.cutover {
        runs.push(
            capture_cutover_run(
                db,
                manifest,
                &layout,
                &source_before,
                &clones_before,
                ordinal,
            )
            .await?,
        );
    }
    ensure!(
        runs.len() == manifest.repetitions.cutover,
        "partition cutover runner did not produce the exact manifest cutover run count"
    );
    let names = runs
        .iter()
        .map(|run| run.name.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        names.len() == runs.len(),
        "partition cutover runs contain duplicate names"
    );
    ensure_source_unchanged(db, manifest, &source_before).await?;
    Ok((layout, runs))
}

async fn capture_cutover_run(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    layout: &CutoverLayout,
    source_before: &ProductionSnapshot,
    expected_clones: &CloneIdentities,
    ordinal: usize,
) -> Result<PartitionCutoverRunEvidence> {
    let transaction = db
        .begin()
        .await
        .context("failed to start partition cutover rehearsal transaction")?;
    let rehearsal =
        capture_transactional_rehearsal(&transaction, manifest, layout, expected_clones).await;
    let rollback = transaction
        .rollback()
        .await
        .context("failed to rollback partition cutover rehearsal transaction");
    let (lock_ms, during_swap) = match rehearsal {
        Ok(value) => {
            rollback?;
            value
        }
        Err(error) => {
            let _ = rollback;
            return Err(error);
        }
    };

    let after_rollback = read_clone_identities(db, layout).await?;
    let rollback_verified = &after_rollback == expected_clones;
    ensure!(
        rollback_verified,
        "partition cutover rehearsal rollback did not restore clone relation identities"
    );
    let production_relations_unchanged = &production_snapshot(db, manifest).await? == source_before;
    ensure!(
        production_relations_unchanged,
        "canonical or retained snapshot-shadow relations changed during cutover rehearsal"
    );

    Ok(PartitionCutoverRunEvidence {
        name: format!("cutover-{ordinal:03}"),
        lock_ms,
        rollback_verified,
        production_relations_unchanged,
        lock_mode: "ACCESS EXCLUSIVE",
        locked_relations: locked_relations(manifest),
        rehearsal_schema: layout.schema.clone(),
        clone_identities_before: expected_clones.clone(),
        clone_identities_during_swap: during_swap,
        clone_identities_after_rollback: after_rollback,
    })
}

async fn capture_transactional_rehearsal(
    transaction: &DatabaseTransaction,
    manifest: &PreparedManifest,
    layout: &CutoverLayout,
    expected_clones: &CloneIdentities,
) -> Result<(u64, CloneIdentities)> {
    let lock_sql = format!(
        "LOCK TABLE {} IN ACCESS EXCLUSIVE MODE;",
        locked_relations(manifest)
            .into_iter()
            .map(|relation| quote_identifier(&relation))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let started = Instant::now();
    transaction
        .execute_unprepared(&lock_sql)
        .await
        .context("failed to acquire ACCESS EXCLUSIVE cutover rehearsal locks")?;
    let elapsed_micros = started.elapsed().as_micros();
    let lock_ms = u64::try_from(elapsed_micros.div_ceil(1000))
        .context("cutover lock duration does not fit into u64 milliseconds")?;

    apply_clone_cutover_choreography(transaction, layout).await?;
    let during_swap = read_clone_identities(transaction, layout).await?;
    ensure!(
        during_swap.canonical_entities == expected_clones.shadow_entities
            && during_swap.shadow_entities == expected_clones.canonical_entities
            && during_swap.canonical_links == expected_clones.shadow_links
            && during_swap.shadow_links == expected_clones.canonical_links,
        "evidence-only cutover clone identities were not swapped inside the rehearsal transaction"
    );
    Ok((lock_ms, during_swap))
}

async fn apply_clone_cutover_choreography(
    transaction: &DatabaseTransaction,
    layout: &CutoverLayout,
) -> Result<()> {
    let previous_entities = "canonical_entities_previous";
    let previous_links = "canonical_links_previous";
    validate_identifier(previous_entities)?;
    validate_identifier(previous_links)?;
    for statement in [
        rename_statement(
            &layout.schema,
            &layout.canonical_entities,
            previous_entities,
        ),
        rename_statement(
            &layout.schema,
            &layout.shadow_entities,
            &layout.canonical_entities,
        ),
        rename_statement(&layout.schema, previous_entities, &layout.shadow_entities),
        rename_statement(&layout.schema, &layout.canonical_links, previous_links),
        rename_statement(
            &layout.schema,
            &layout.shadow_links,
            &layout.canonical_links,
        ),
        rename_statement(&layout.schema, previous_links, &layout.shadow_links),
    ] {
        transaction
            .execute_unprepared(&statement)
            .await
            .context("failed to apply evidence-only cutover rename choreography")?;
    }
    Ok(())
}

fn rename_statement(schema: &str, source: &str, target: &str) -> String {
    format!(
        "ALTER TABLE {} RENAME TO {};",
        qualified(schema, source),
        quote_identifier(target)
    )
}

async fn create_rehearsal_clones(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    layout: &CutoverLayout,
) -> Result<()> {
    for statement in [
        format!("CREATE SCHEMA {};", quote_identifier(&layout.schema)),
        clone_statement(&layout.schema, &layout.canonical_entities, "index_entities"),
        clone_statement(&layout.schema, &layout.canonical_links, "index_links"),
        clone_statement(
            &layout.schema,
            &layout.shadow_entities,
            &manifest.shadow_relations.entities.parent,
        ),
        clone_statement(
            &layout.schema,
            &layout.shadow_links,
            &manifest.shadow_relations.links.parent,
        ),
    ] {
        db.execute_unprepared(&statement)
            .await
            .context("failed to create evidence-only cutover rehearsal clones")?;
    }
    Ok(())
}

fn clone_statement(schema: &str, target: &str, source: &str) -> String {
    format!(
        "CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS);",
        qualified(schema, target),
        quote_identifier(source)
    )
}

fn locked_relations(manifest: &PreparedManifest) -> Vec<String> {
    vec![
        "index_entities".to_owned(),
        "index_links".to_owned(),
        manifest.shadow_relations.entities.parent.clone(),
        manifest.shadow_relations.links.parent.clone(),
    ]
}

async fn read_clone_identities<C>(db: &C, layout: &CutoverLayout) -> Result<CloneIdentities>
where
    C: ConnectionTrait,
{
    Ok(CloneIdentities {
        canonical_entities: relation_oid(db, &layout.schema, &layout.canonical_entities).await?,
        canonical_links: relation_oid(db, &layout.schema, &layout.canonical_links).await?,
        shadow_entities: relation_oid(db, &layout.schema, &layout.shadow_entities).await?,
        shadow_links: relation_oid(db, &layout.schema, &layout.shadow_links).await?,
    })
}

async fn relation_oid<C>(db: &C, schema: &str, relation: &str) -> Result<i64>
where
    C: ConnectionTrait,
{
    let name = format!("{schema}.{relation}");
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1)::oid::bigint AS oid",
            vec![name.clone().into()],
        ))
        .await?
        .with_context(|| format!("clone relation identity query returned no row for {name}"))?;
    let oid: Option<i64> = row.try_get("", "oid")?;
    oid.with_context(|| format!("clone relation was not found: {name}"))
}

async fn production_snapshot(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<ProductionSnapshot> {
    Ok(ProductionSnapshot {
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
        canonical_entities_catalog: relation_catalog(db, "index_entities").await?,
        canonical_links_catalog: relation_catalog(db, "index_links").await?,
        shadow_entities_catalog: relation_catalog(db, &manifest.shadow_relations.entities.parent)
            .await?,
        shadow_links_catalog: relation_catalog(db, &manifest.shadow_relations.links.parent).await?,
    })
}

fn ensure_source_parity(snapshot: &ProductionSnapshot) -> Result<()> {
    ensure!(
        snapshot.canonical_entities == snapshot.shadow_entities,
        "canonical and retained shadow entities diverged before cutover evidence"
    );
    ensure!(
        snapshot.canonical_links == snapshot.shadow_links,
        "canonical and retained shadow links diverged before cutover evidence"
    );
    Ok(())
}

async fn ensure_source_unchanged(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    expected: &ProductionSnapshot,
) -> Result<()> {
    ensure!(
        &production_snapshot(db, manifest).await? == expected,
        "canonical or retained snapshot-shadow relations changed during cutover rehearsal"
    );
    Ok(())
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
            format!("cutover logical digest query returned no row for {relation_sql}")
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

async fn relation_catalog(
    db: &DatabaseConnection,
    relation: &str,
) -> Result<RelationCatalogEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT c.oid::bigint AS oid, c.relname, c.relkind::text AS relkind, ",
                "c.relispartition, EXISTS (SELECT 1 FROM pg_partitioned_table p WHERE p.partrelid = c.oid) AS partitioned, ",
                "obj_description(c.oid, 'pg_class') AS comment FROM pg_class c WHERE c.oid = to_regclass($1)"
            ),
            vec![relation.into()],
        ))
        .await?
        .with_context(|| format!("relation catalog entry was not found: {relation}"))?;
    let oid: i64 = row.try_get("", "oid")?;
    let children = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT child.oid::bigint AS oid, child.relname, pg_get_expr(child.relpartbound, child.oid) AS bound ",
                "FROM pg_inherits inheritance JOIN pg_class child ON child.oid = inheritance.inhrelid ",
                "WHERE inheritance.inhparent = $1::oid ORDER BY child.relname"
            ),
            vec![oid.into()],
        ))
        .await?
        .into_iter()
        .map(|child| {
            Ok(ChildCatalogEvidence {
                oid: child.try_get("", "oid")?,
                name: child.try_get("", "relname")?,
                bound: child.try_get("", "bound")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RelationCatalogEvidence {
        oid,
        name: row.try_get("", "relname")?,
        relkind: row.try_get("", "relkind")?,
        relispartition: row.try_get("", "relispartition")?,
        partitioned: row.try_get("", "partitioned")?,
        comment: row.try_get("", "comment")?,
        children,
    })
}

async fn validate_source_catalog(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<()> {
    validate_canonical_catalog(db, "index_entities").await?;
    validate_canonical_catalog(db, "index_links").await?;
    validate_shadow_relation_catalog(db, manifest, &manifest.shadow_relations.entities).await?;
    validate_shadow_relation_catalog(db, manifest, &manifest.shadow_relations.links).await?;
    Ok(())
}

async fn validate_canonical_catalog(db: &DatabaseConnection, relation: &str) -> Result<()> {
    let catalog = relation_catalog(db, relation).await?;
    ensure!(
        catalog.relkind == "r" && !catalog.relispartition && !catalog.partitioned,
        "canonical relation {relation} must remain an ordinary unpartitioned table"
    );
    ensure!(
        catalog.children.is_empty(),
        "canonical relation {relation} must not have inherited children"
    );
    Ok(())
}

async fn validate_shadow_relation_catalog(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
    plan: &RelationPlan,
) -> Result<()> {
    let catalog = relation_catalog(db, &plan.parent).await?;
    ensure!(
        catalog.relkind == "p",
        "shadow parent {} must be partitioned",
        plan.parent
    );
    let expected_comment = format!("rustok-index-partition:{}", manifest.evidence_id);
    ensure!(
        catalog.comment.as_deref() == Some(expected_comment.as_str()),
        "shadow parent {} is not bound to the evidence identity",
        plan.parent
    );
    ensure!(
        catalog.children.len() == plan.partitions.len(),
        "shadow parent {} has an unexpected child count",
        plan.parent
    );
    for (remainder, child) in catalog.children.iter().enumerate() {
        ensure!(
            child.name == plan.partitions[remainder],
            "unexpected shadow child {}",
            child.name
        );
        let bound = child.bound.to_ascii_lowercase();
        ensure!(
            bound.contains(&format!("modulus {}", manifest.modulus))
                && bound.contains(&format!("remainder {remainder}")),
            "shadow child {} has an unexpected bound: {}",
            child.name,
            child.bound
        );
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<(PreparedManifest, JsonValue)> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect partition manifest at {path:?}"))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "partition manifest must be a regular non-symlink file"
    );
    let raw: JsonValue = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("failed to read partition manifest at {path:?}"))?,
    )
    .context("failed to parse partition manifest JSON")?;
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
    ensure!(
        sha256_hex(&canonical_json_bytes(&JsonValue::Object(input))?) == manifest.evidence_id,
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
        .context("partition cutover session-setting query returned no row")?;
    let actual: String = row.try_get("", "value")?;
    ensure!(
        actual == expected,
        "requires {setting}={expected}, got {actual}"
    );
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
        .context("cutover schema existence query returned no row")?;
    let existing: Option<String> = row.try_get("", "schema")?;
    ensure!(
        existing.is_none(),
        "cutover evidence schema already exists: {schema}"
    );
    Ok(())
}

async fn acquire_cutover_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        vec![format!("rustok-index-partition-cutover:{evidence_id}").into()],
    ))
    .await?
    .context("partition cutover advisory lock returned no row")?;
    Ok(())
}

async fn release_cutover_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock(hashtextextended($1, 0)) AS unlocked",
            vec![format!("rustok-index-partition-cutover:{evidence_id}").into()],
        ))
        .await?
        .context("partition cutover advisory unlock returned no row")?;
    let unlocked: bool = row.try_get("", "unlocked")?;
    ensure!(unlocked, "partition cutover advisory lock was not held");
    Ok(())
}

fn ensure_output_available(path: &Path) -> Result<()> {
    ensure!(!path.exists(), "refusing to overwrite {path:?}");
    Ok(())
}

fn publish_cutover_artifact(path: &Path, runs: &[PartitionCutoverRunEvidence]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cutover evidence directory {parent:?}"))?;
    }
    ensure_output_available(path)?;
    let mut bytes = serde_json::to_vec_pretty(runs)
        .context("failed to serialize partition cutover evidence")?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| {
            format!("failed to create temporary cutover evidence file {temporary:?}")
        })?;
    file.write_all(&bytes).with_context(|| {
        format!("failed to write temporary cutover evidence file {temporary:?}")
    })?;
    file.sync_all()
        .with_context(|| format!("failed to sync temporary cutover evidence file {temporary:?}"))?;
    let publish = fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish cutover evidence to {path:?}"));
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
