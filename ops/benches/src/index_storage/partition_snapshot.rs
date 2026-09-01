use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use super::{connect_benchmark_database, ensure_database_metadata_stable, read_database_metadata};

const MANIFEST_CONTRACT: &str = "index_partition_evidence_manifest_v1";
const SHADOW_PLAN_VERSION: &str = "tenant_hash_shadow_v1";
const QUERY_AUDIT_CONTRACT: &str = "index_partition_query_audit_v1";
const RELATION_DIGEST_CONTRACT: &str = "index_partition_relation_digest_v1";
const COPY_OPT_IN: &str = "INDEX_PARTITION_ALLOW_SHADOW_COPY";

#[derive(Debug, Clone)]
pub struct PartitionSnapshotConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub query_audit_path: PathBuf,
    pub output_root: PathBuf,
}

impl PartitionSnapshotConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(COPY_OPT_IN).as_deref(), Ok("1")),
            "{COPY_OPT_IN}=1 is required because the runner creates and fills shadow tables"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition snapshot capture")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let query_audit_path = env::var("INDEX_PARTITION_QUERY_AUDIT")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_QUERY_AUDIT is required")?;
        let output_root = env::var("INDEX_PARTITION_EVIDENCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"));
        ensure!(
            manifest_path != query_audit_path,
            "manifest and query-audit paths must be distinct"
        );
        Ok(Self {
            database_url,
            manifest_path,
            query_audit_path,
            output_root,
        })
    }

    fn baseline_path(&self) -> PathBuf {
        self.output_root.join("baseline.json")
    }

    fn shadow_path(&self) -> PathBuf {
        self.output_root.join("shadow.json")
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
    evidence_id: String,
    shadow_plan_version: String,
    shadow_relations: ShadowRelations,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TenantPredicateAudit {
    pub contract: String,
    pub total_templates: i64,
    pub tenant_scoped_templates: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RelationEvidence {
    pub rows: i64,
    pub bytes: i64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShadowRelationEvidence {
    pub rows: i64,
    pub bytes: i64,
    pub digest: String,
    pub partition_bytes: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BaselineSnapshot {
    pub generated_at: DateTime<Utc>,
    pub distinct_tenants: i64,
    pub tenant_predicate_audit: TenantPredicateAudit,
    pub entities: RelationEvidence,
    pub links: RelationEvidence,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShadowSnapshot {
    pub generated_at: DateTime<Utc>,
    pub caught_up: bool,
    pub foreign_keys_validated: bool,
    pub orphan_links: i64,
    pub entities: ShadowRelationEvidence,
    pub links: ShadowRelationEvidence,
}

#[derive(Debug, Clone)]
pub struct PartitionSnapshotCapture {
    pub evidence_id: String,
    pub baseline_path: PathBuf,
    pub shadow_path: PathBuf,
    pub baseline_rows: i64,
    pub shadow_rows: i64,
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
            Self::Entities => concat!(
                "row_data.tenant_id, row_data.module_name, row_data.entity_name, ",
                "row_data.schema_version, row_data.entity_id, row_data.locale_key"
            ),
            Self::Links => concat!(
                "row_data.tenant_id, row_data.source_module, row_data.source_entity, ",
                "row_data.source_schema_version, row_data.source_entity_id, ",
                "row_data.source_locale_key, row_data.source_version, ",
                "row_data.link_name, row_data.ordinal"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogicalRelation {
    rows: i64,
    digest: String,
}

pub async fn capture_partition_snapshot(
    config: &PartitionSnapshotConfig,
) -> Result<PartitionSnapshotCapture> {
    let manifest: PreparedManifest = read_regular_json(&config.manifest_path, "manifest")?;
    validate_manifest(&manifest)?;
    let audit: TenantPredicateAudit = read_regular_json(&config.query_audit_path, "query audit")?;
    validate_query_audit(&audit)?;

    let baseline_path = config.baseline_path();
    let shadow_path = config.shadow_path();
    ensure_outputs_available(&baseline_path, &shadow_path)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared("SET jit = off; SET lock_timeout = '5s'; SET statement_timeout = 0;")
        .await
        .context("failed to pin partition evidence session settings")?;
    let database_metadata = read_database_metadata(&db).await?;
    ensure!(
        database_metadata.server_version_num.starts_with("16"),
        "partition evidence requires PostgreSQL 16, got {}",
        database_metadata.server_version_num
    );
    ensure!(
        database_metadata.jit == "off",
        "partition evidence requires jit=off"
    );
    ensure_unpartitioned_source(&db, "index_entities").await?;
    ensure_unpartitioned_source(&db, "index_links").await?;

    acquire_capture_lock(&db, &manifest.evidence_id).await?;
    ensure_shadow_absent(&db, &manifest.shadow_relations.entities).await?;
    ensure_shadow_absent(&db, &manifest.shadow_relations.links).await?;
    db.execute_unprepared(&render_shadow_bootstrap(&manifest))
        .await
        .context("failed to create deterministic partition evidence shadow tables")?;

    let transaction = db
        .begin()
        .await
        .context("failed to start snapshot transaction")?;
    transaction
        .execute_unprepared("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ;")
        .await
        .context("failed to pin repeatable-read snapshot")?;
    let baseline_generated_at = Utc::now();
    let distinct_tenants = distinct_tenants(&transaction).await?;
    ensure!(
        distinct_tenants > 0,
        "partition evidence requires at least one tenant"
    );
    let baseline_entities =
        relation_evidence(&transaction, "index_entities", RelationKind::Entities).await?;
    let baseline_links =
        relation_evidence(&transaction, "index_links", RelationKind::Links).await?;
    copy_relation(
        &transaction,
        "index_entities",
        &manifest.shadow_relations.entities.parent,
    )
    .await?;
    copy_relation(
        &transaction,
        "index_links",
        &manifest.shadow_relations.links.parent,
    )
    .await?;
    transaction
        .commit()
        .await
        .context("failed to commit shadow snapshot copy")?;

    attach_shadow_source_integrity(&db, &manifest).await?;
    analyze_shadow(&db, &manifest.shadow_relations.entities.parent).await?;
    analyze_shadow(&db, &manifest.shadow_relations.links.parent).await?;

    let shadow_entities = shadow_relation_evidence(
        &db,
        &manifest.shadow_relations.entities,
        RelationKind::Entities,
    )
    .await?;
    let shadow_links =
        shadow_relation_evidence(&db, &manifest.shadow_relations.links, RelationKind::Links)
            .await?;
    let current_entities = logical_relation(&db, "index_entities", RelationKind::Entities).await?;
    let current_links = logical_relation(&db, "index_links", RelationKind::Links).await?;
    let caught_up = current_entities.rows == shadow_entities.rows
        && current_entities.digest == shadow_entities.digest
        && current_links.rows == shadow_links.rows
        && current_links.digest == shadow_links.digest;
    let orphan_links = orphan_link_count(&db, &manifest).await?;
    let foreign_keys_validated = shadow_foreign_key_validated(&db, &manifest).await?;

    let baseline = BaselineSnapshot {
        generated_at: baseline_generated_at,
        distinct_tenants,
        tenant_predicate_audit: audit,
        entities: baseline_entities,
        links: baseline_links,
    };
    let shadow = ShadowSnapshot {
        generated_at: Utc::now(),
        caught_up,
        foreign_keys_validated,
        orphan_links,
        entities: shadow_entities,
        links: shadow_links,
    };
    ensure_snapshot_parity(&baseline, &shadow)?;
    ensure!(
        orphan_links == 0,
        "shadow snapshot contains {orphan_links} orphan links"
    );
    ensure!(
        foreign_keys_validated,
        "shadow source foreign key is not validated"
    );
    ensure_database_metadata_stable(&db, &database_metadata, "partition snapshot capture").await?;

    publish_snapshot_pair(&baseline_path, &baseline, &shadow_path, &shadow)?;
    release_capture_lock(&db, &manifest.evidence_id).await?;
    Ok(PartitionSnapshotCapture {
        evidence_id: manifest.evidence_id,
        baseline_path,
        shadow_path,
        baseline_rows: checked_total_rows(&baseline)?,
        shadow_rows: checked_shadow_total_rows(&shadow)?,
    })
}

fn read_regular_json<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} at {path:?}"))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "{label} must be a regular non-symlink file"
    );
    let bytes = fs::read(path).with_context(|| format!("failed to read {label} at {path:?}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {label} JSON"))
}

fn validate_manifest(manifest: &PreparedManifest) -> Result<()> {
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
        "manifest commit must be lowercase SHA-1"
    );
    ensure!(
        !manifest.run_key.is_empty() && manifest.run_key.len() <= 128,
        "manifest run_key must be bounded and non-empty"
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
        manifest.plan_digest_contract == "normalized_partition_plan_v1",
        "unexpected plan digest contract"
    );
    ensure!(
        (2..=128).contains(&manifest.modulus) && manifest.modulus.is_power_of_two(),
        "manifest modulus must be a power of two between 2 and 128"
    );
    ensure!(
        is_lower_hex(&manifest.evidence_id, 64),
        "invalid evidence_id"
    );
    ensure!(
        manifest.shadow_plan_version == SHADOW_PLAN_VERSION,
        "unexpected shadow plan version"
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
        "manifest shadow definition hash does not match the evidence identity"
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
    ensure!(plan.source == expected_source, "unexpected relation source");
    ensure!(
        plan.parent == expected_parent,
        "unexpected shadow parent name"
    );
    validate_identifier(&plan.parent)?;
    ensure!(
        plan.partitions.len() == modulus as usize,
        "shadow relation must contain exactly {modulus} partitions"
    );
    for (remainder, partition) in plan.partitions.iter().enumerate() {
        validate_identifier(partition)?;
        ensure!(
            partition == &format!("{expected_parent}_p{remainder:03}"),
            "unexpected shadow partition name"
        );
    }
    Ok(())
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

fn validate_query_audit(audit: &TenantPredicateAudit) -> Result<()> {
    ensure!(
        audit.contract == QUERY_AUDIT_CONTRACT,
        "unexpected query audit contract"
    );
    ensure!(
        audit.total_templates > 0,
        "query audit must contain templates"
    );
    ensure!(
        audit.tenant_scoped_templates >= 0
            && audit.tenant_scoped_templates <= audit.total_templates,
        "tenant-scoped template count is outside the audited range"
    );
    Ok(())
}

fn ensure_outputs_available(baseline_path: &Path, shadow_path: &Path) -> Result<()> {
    ensure!(
        baseline_path != shadow_path,
        "baseline and shadow outputs must be distinct"
    );
    ensure!(
        !baseline_path.exists(),
        "refusing to overwrite {baseline_path:?}"
    );
    ensure!(
        !shadow_path.exists(),
        "refusing to overwrite {shadow_path:?}"
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
                "FROM pg_class c WHERE c.oid = $1::regclass"
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

async fn ensure_shadow_absent(db: &DatabaseConnection, plan: &RelationPlan) -> Result<()> {
    for relation in std::iter::once(&plan.parent).chain(plan.partitions.iter()) {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT to_regclass($1)::text AS relation",
                vec![relation.clone().into()],
            ))
            .await?
            .context("shadow existence query returned no row")?;
        let existing: Option<String> = row.try_get("", "relation")?;
        ensure!(
            existing.is_none(),
            "shadow relation already exists: {relation}"
        );
    }
    Ok(())
}

async fn acquire_capture_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock(hashtextextended($1, 0))",
        vec![format!("rustok-index-partition-snapshot:{evidence_id}").into()],
    ))
    .await?
    .context("partition capture advisory lock returned no row")?;
    Ok(())
}

async fn release_capture_lock(db: &DatabaseConnection, evidence_id: &str) -> Result<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock(hashtextextended($1, 0)) AS unlocked",
            vec![format!("rustok-index-partition-snapshot:{evidence_id}").into()],
        ))
        .await?
        .context("partition capture advisory unlock returned no row")?;
    let unlocked: bool = row.try_get("", "unlocked")?;
    ensure!(unlocked, "partition capture advisory lock was not held");
    Ok(())
}

fn render_shadow_bootstrap(manifest: &PreparedManifest) -> String {
    let mut statements = vec!["BEGIN;".to_owned()];
    for plan in [
        &manifest.shadow_relations.entities,
        &manifest.shadow_relations.links,
    ] {
        statements.push(format!(
            "CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS) PARTITION BY HASH (tenant_id);",
            quote_identifier(&plan.parent),
            quote_identifier(&plan.source),
        ));
        statements.push(format!(
            "COMMENT ON TABLE {} IS 'rustok-index-partition:{}';",
            quote_identifier(&plan.parent),
            manifest.evidence_id,
        ));
        for (remainder, partition) in plan.partitions.iter().enumerate() {
            statements.push(format!(
                "CREATE TABLE {} PARTITION OF {} FOR VALUES WITH (MODULUS {}, REMAINDER {});",
                quote_identifier(partition),
                quote_identifier(&plan.parent),
                manifest.modulus,
                remainder,
            ));
        }
    }
    statements.push("COMMIT;".to_owned());
    statements.join("\n")
}

async fn distinct_tenants<C: ConnectionTrait>(db: &C) -> Result<i64> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            concat!(
                "SELECT count(DISTINCT tenant_id)::bigint AS distinct_tenants FROM (",
                "SELECT tenant_id FROM index_entities UNION ALL ",
                "SELECT tenant_id FROM index_links) tenants"
            )
            .to_owned(),
        ))
        .await?
        .context("distinct tenant query returned no row")?;
    Ok(row.try_get("", "distinct_tenants")?)
}

async fn logical_relation<C: ConnectionTrait>(
    db: &C,
    relation: &str,
    kind: RelationKind,
) -> Result<LogicalRelation> {
    let sql = format!(
        concat!(
            "SELECT count(*)::bigint AS rows, ",
            "COALESCE(md5(string_agg(md5(row_to_json(row_data)::text), '' ORDER BY {})), md5('')) AS digest_seed ",
            "FROM (SELECT * FROM {}) row_data"
        ),
        kind.digest_order(),
        quote_identifier(relation),
    );
    let row = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .with_context(|| format!("relation digest query returned no row for {relation}"))?;
    let rows: i64 = row.try_get("", "rows")?;
    let digest_seed: String = row.try_get("", "digest_seed")?;
    let digest = sha256_hex(
        format!(
            "{RELATION_DIGEST_CONTRACT}\u{1f}{}\u{1f}{rows}\u{1f}{digest_seed}",
            kind.label()
        )
        .as_bytes(),
    );
    Ok(LogicalRelation { rows, digest })
}

async fn relation_evidence<C: ConnectionTrait>(
    db: &C,
    relation: &str,
    kind: RelationKind,
) -> Result<RelationEvidence> {
    let logical = logical_relation(db, relation, kind).await?;
    Ok(RelationEvidence {
        rows: logical.rows,
        bytes: relation_size(db, relation).await?,
        digest: logical.digest,
    })
}

async fn shadow_relation_evidence(
    db: &DatabaseConnection,
    plan: &RelationPlan,
    kind: RelationKind,
) -> Result<ShadowRelationEvidence> {
    let logical = logical_relation(db, &plan.parent, kind).await?;
    let mut partition_bytes = Vec::with_capacity(plan.partitions.len());
    for partition in &plan.partitions {
        partition_bytes.push(relation_size(db, partition).await?);
    }
    let bytes = partition_bytes.iter().try_fold(0_i64, |total, value| {
        total
            .checked_add(*value)
            .context("shadow relation byte count overflow")
    })?;
    Ok(ShadowRelationEvidence {
        rows: logical.rows,
        bytes,
        digest: logical.digest,
        partition_bytes,
    })
}

async fn relation_size<C: ConnectionTrait>(db: &C, relation: &str) -> Result<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_total_relation_size($1::regclass)::bigint AS bytes",
            vec![relation.into()],
        ))
        .await?
        .with_context(|| format!("relation size query returned no row for {relation}"))?;
    let bytes: i64 = row.try_get("", "bytes")?;
    ensure!(
        bytes > 0,
        "relation {relation} has a non-positive physical size"
    );
    Ok(bytes)
}

async fn copy_relation<C: ConnectionTrait>(db: &C, source: &str, target: &str) -> Result<()> {
    db.execute_unprepared(&format!(
        "INSERT INTO {} SELECT * FROM {};",
        quote_identifier(target),
        quote_identifier(source),
    ))
    .await
    .with_context(|| format!("failed to copy {source} into {target}"))?;
    Ok(())
}

async fn attach_shadow_source_integrity(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<()> {
    let suffix = &manifest.evidence_id[..16];
    let unique_index = format!("idx_pe_{suffix}_entity_source");
    let foreign_key = format!("fk_pe_{suffix}_link_source");
    validate_identifier(&unique_index)?;
    validate_identifier(&foreign_key)?;
    db.execute_unprepared(&format!(
        concat!(
            "CREATE UNIQUE INDEX {} ON {} (",
            "tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version);"
        ),
        quote_identifier(&unique_index),
        quote_identifier(&manifest.shadow_relations.entities.parent),
    ))
    .await
    .context("failed to create the shadow entity source-version uniqueness contract")?;
    db.execute_unprepared(&format!(
        concat!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY (",
            "tenant_id, source_module, source_entity, source_schema_version, source_entity_id, source_locale_key, source_version",
            ") REFERENCES {} (",
            "tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version",
            ") ON UPDATE RESTRICT ON DELETE CASCADE;"
        ),
        quote_identifier(&manifest.shadow_relations.links.parent),
        quote_identifier(&foreign_key),
        quote_identifier(&manifest.shadow_relations.entities.parent),
    ))
    .await
    .context("failed to add and validate the shadow source foreign key")?;
    Ok(())
}

async fn analyze_shadow(db: &DatabaseConnection, relation: &str) -> Result<()> {
    db.execute_unprepared(&format!("ANALYZE {};", quote_identifier(relation)))
        .await
        .with_context(|| format!("failed to analyze shadow relation {relation}"))?;
    Ok(())
}

async fn orphan_link_count(db: &DatabaseConnection, manifest: &PreparedManifest) -> Result<i64> {
    let links = quote_identifier(&manifest.shadow_relations.links.parent);
    let entities = quote_identifier(&manifest.shadow_relations.entities.parent);
    let sql = format!(
        concat!(
            "SELECT count(*)::bigint AS orphan_links FROM {links} l LEFT JOIN {entities} e ON ",
            "e.tenant_id = l.tenant_id AND e.module_name = l.source_module AND ",
            "e.entity_name = l.source_entity AND e.schema_version = l.source_schema_version AND ",
            "e.entity_id = l.source_entity_id AND e.locale_key = l.source_locale_key AND ",
            "e.source_version = l.source_version WHERE e.tenant_id IS NULL"
        ),
        links = links,
        entities = entities
    );
    let row = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .context("orphan-link query returned no row")?;
    Ok(row.try_get("", "orphan_links")?)
}

async fn shadow_foreign_key_validated(
    db: &DatabaseConnection,
    manifest: &PreparedManifest,
) -> Result<bool> {
    let name = format!("fk_pe_{}_link_source", &manifest.evidence_id[..16]);
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            concat!(
                "SELECT convalidated FROM pg_constraint ",
                "WHERE conrelid = $1::regclass AND conname = $2"
            ),
            vec![
                manifest.shadow_relations.links.parent.clone().into(),
                name.into(),
            ],
        ))
        .await?
        .context("shadow foreign-key catalog row was not found")?;
    Ok(row.try_get("", "convalidated")?)
}

fn ensure_snapshot_parity(baseline: &BaselineSnapshot, shadow: &ShadowSnapshot) -> Result<()> {
    ensure!(
        baseline.entities.rows == shadow.entities.rows
            && baseline.entities.digest == shadow.entities.digest,
        "entity shadow snapshot diverged from the repeatable-read baseline"
    );
    ensure!(
        baseline.links.rows == shadow.links.rows && baseline.links.digest == shadow.links.digest,
        "link shadow snapshot diverged from the repeatable-read baseline"
    );
    Ok(())
}

fn checked_total_rows(baseline: &BaselineSnapshot) -> Result<i64> {
    baseline
        .entities
        .rows
        .checked_add(baseline.links.rows)
        .context("baseline row count overflow")
}

fn checked_shadow_total_rows(shadow: &ShadowSnapshot) -> Result<i64> {
    shadow
        .entities
        .rows
        .checked_add(shadow.links.rows)
        .context("shadow row count overflow")
}

fn publish_snapshot_pair(
    baseline_path: &Path,
    baseline: &BaselineSnapshot,
    shadow_path: &Path,
    shadow: &ShadowSnapshot,
) -> Result<()> {
    let baseline_bytes = json_bytes(baseline)?;
    let shadow_bytes = json_bytes(shadow)?;
    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create evidence directory {parent:?}"))?;
    }
    if let Some(parent) = shadow_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create evidence directory {parent:?}"))?;
    }
    ensure_outputs_available(baseline_path, shadow_path)?;
    let baseline_temp = temporary_path(baseline_path);
    let shadow_temp = temporary_path(shadow_path);
    write_new_file(&baseline_temp, &baseline_bytes)?;
    if let Err(error) = write_new_file(&shadow_temp, &shadow_bytes) {
        let _ = fs::remove_file(&baseline_temp);
        return Err(error);
    }
    let result = (|| -> Result<()> {
        fs::hard_link(&baseline_temp, baseline_path)
            .with_context(|| format!("failed to publish {baseline_path:?}"))?;
        if let Err(error) = fs::hard_link(&shadow_temp, shadow_path) {
            let _ = fs::remove_file(baseline_path);
            return Err(error).with_context(|| format!("failed to publish {shadow_path:?}"));
        }
        Ok(())
    })();
    let _ = fs::remove_file(&baseline_temp);
    let _ = fs::remove_file(&shadow_temp);
    result
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec_pretty(value).context("failed to serialize evidence JSON")?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(value)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to create temporary evidence file {path:?}"))?;
    file.write_all(bytes)
        .with_context(|| format!("failed to write temporary evidence file {path:?}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync temporary evidence file {path:?}"))?;
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
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

    fn manifest() -> PreparedManifest {
        let evidence_id = "a".repeat(64);
        let definition = [
            "rustok-index-partition",
            SHADOW_PLAN_VERSION,
            evidence_id.as_str(),
            "tenant_hash",
            "4",
        ]
        .join("\u{1f}");
        let definition_hash = sha256_hex(definition.as_bytes());
        let suffix = &definition_hash[..24];
        let plan = |source: &str| {
            let parent = format!("{source}_shadow_{suffix}");
            RelationPlan {
                source: source.to_owned(),
                partitions: (0..4)
                    .map(|remainder| format!("{parent}_p{remainder:03}"))
                    .collect(),
                parent,
            }
        };
        let entities = plan("index_entities");
        let links = plan("index_links");
        PreparedManifest {
            contract: MANIFEST_CONTRACT.to_owned(),
            repository: "RusTokRs/RusTok".to_owned(),
            commit: "1".repeat(40),
            run_key: "fixture-run-1".to_owned(),
            postgres_image: "postgres:16".to_owned(),
            strategy: "tenant_hash".to_owned(),
            plan_digest_contract: "normalized_partition_plan_v1".to_owned(),
            modulus: 4,
            evidence_id,
            shadow_plan_version: SHADOW_PLAN_VERSION.to_owned(),
            shadow_relations: ShadowRelations {
                definition_hash,
                entities,
                links,
            },
        }
    }

    #[test]
    fn deterministic_manifest_and_bootstrap_remain_shadow_only() {
        let manifest = manifest();
        validate_manifest(&manifest).unwrap();
        let sql = render_shadow_bootstrap(&manifest);
        assert!(sql.contains("PARTITION BY HASH (tenant_id)"));
        assert!(sql.contains("MODULUS 4, REMAINDER 3"));
        for forbidden in [
            "ALTER TABLE \"index_entities\"",
            "ALTER TABLE \"index_links\"",
            "DROP TABLE",
            "RENAME TO",
            "VACUUM FULL",
        ] {
            assert!(!sql.contains(forbidden), "bootstrap contains {forbidden}");
        }
    }

    #[test]
    fn query_audit_and_relation_digest_contract_fail_closed() {
        validate_query_audit(&TenantPredicateAudit {
            contract: QUERY_AUDIT_CONTRACT.to_owned(),
            total_templates: 2,
            tenant_scoped_templates: 2,
        })
        .unwrap();
        assert!(
            validate_query_audit(&TenantPredicateAudit {
                contract: QUERY_AUDIT_CONTRACT.to_owned(),
                total_templates: 1,
                tenant_scoped_templates: 2,
            })
            .is_err()
        );
        let digest = sha256_hex(
            format!("{RELATION_DIGEST_CONTRACT}\u{1f}entities\u{1f}1\u{1f}seed").as_bytes(),
        );
        assert_eq!(digest.len(), 64);
    }
}
