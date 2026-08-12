use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::PortError;
use rustok_core::{Error as CoreError, MigrationSource, Result as CoreResult};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_search::{
    ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionRecord,
    ForumProjectionOwnerRevisionRequest, ForumProjectionOwnerRevisionSourcePort,
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    ForumProjectionReconciler, SearchModule, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_missing_delivery_repair_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-missing-delivery-repair-evidence.json";

struct PostgresSearchEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search missing-delivery repair proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_repair_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let schema_url = database_url_in_schema(&database_url, &schema_name);
        let db = connect(&schema_url, 4).await?;
        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            create_checkpoint_audit(&db).await
        }
        .await;
        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

struct ControlledForumSource {
    tenant_id: Uuid,
    document_id: Uuid,
    fail_next_rebuild: AtomicBool,
    list_calls: AtomicUsize,
}

impl ControlledForumSource {
    fn new(tenant_id: Uuid, document_id: Uuid) -> Self {
        Self {
            tenant_id,
            document_id,
            fail_next_rebuild: AtomicBool::new(true),
            list_calls: AtomicUsize::new(0),
        }
    }

    fn list_calls(&self) -> usize {
        self.list_calls.load(Ordering::SeqCst)
    }

    fn current_document(&self) -> SearchProjectionDocument {
        SearchProjectionDocument {
            document_key: format!("forum:{}:en", self.document_id),
            tenant_id: self.tenant_id,
            document_id: self.document_id,
            source_module: "forum".to_string(),
            entity_type: "forum_topic".to_string(),
            locale: "en".to_string(),
            status: "published".to_string(),
            is_public: true,
            title: "Repaired forum document".to_string(),
            subtitle: None,
            slug: Some("repaired-forum-document".to_string()),
            handle: None,
            body: "Current owner state".to_string(),
            keywords_text: "forum repair".to_string(),
            facets: json!({"repair": true}),
            payload: json!({"owner_state": "current"}),
            published_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[async_trait]
impl SearchProjectionSource for ControlledForumSource {
    fn source_module(&self) -> &'static str {
        "forum"
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        _limit: usize,
    ) -> CoreResult<SearchProjectionPage> {
        self.list_calls.fetch_add(1, Ordering::SeqCst);
        if tenant_id != self.tenant_id || after.is_some() {
            return Err(CoreError::Validation(
                "missing-delivery proof received an unexpected projection page request".to_string(),
            ));
        }
        if self.fail_next_rebuild.swap(false, Ordering::SeqCst) {
            return Err(CoreError::External(
                "injected Forum projection rebuild failure".to_string(),
            ));
        }
        Ok(SearchProjectionPage {
            documents: vec![self.current_document()],
            next_cursor: None,
        })
    }

    async fn load_public_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> CoreResult<Vec<SearchProjectionDocument>> {
        if tenant_id == self.tenant_id
            && entity_type == "forum_topic"
            && entity_id == self.document_id
        {
            Ok(vec![self.current_document()])
        } else {
            Ok(Vec::new())
        }
    }
}

struct FixedOwnerRevisionSource {
    tenant_id: Uuid,
    revisions: Vec<ForumProjectionOwnerRevisionRecord>,
    tenant_page_calls: AtomicUsize,
    revision_page_calls: AtomicUsize,
}

impl FixedOwnerRevisionSource {
    fn new(tenant_id: Uuid, revisions: Vec<ForumProjectionOwnerRevisionRecord>) -> Self {
        Self {
            tenant_id,
            revisions,
            tenant_page_calls: AtomicUsize::new(0),
            revision_page_calls: AtomicUsize::new(0),
        }
    }

    fn tenant_page_calls(&self) -> usize {
        self.tenant_page_calls.load(Ordering::SeqCst)
    }

    fn revision_page_calls(&self) -> usize {
        self.revision_page_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ForumProjectionOwnerRevisionSourcePort for FixedOwnerRevisionSource {
    async fn list_owner_revisions(
        &self,
        request: ForumProjectionOwnerRevisionRequest,
    ) -> Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
        self.revision_page_calls.fetch_add(1, Ordering::SeqCst);
        if request.tenant_id != self.tenant_id {
            return Err(PortError::validation(
                "forum.search_projection_owner_revision.tenant_mismatch",
                "missing-delivery proof received a foreign tenant request",
            ));
        }
        Ok(self
            .revisions
            .iter()
            .filter(|revision| revision.owner_revision > request.after_owner_revision)
            .take(request.limit)
            .cloned()
            .collect())
    }

    async fn list_owner_revision_tenants(
        &self,
        request: ForumProjectionOwnerTenantPageRequest,
    ) -> Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
        self.tenant_page_calls.fetch_add(1, Ordering::SeqCst);
        if request.after_tenant_id.is_some() || request.limit == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForumProjectionOwnerTenantHead {
            tenant_id: self.tenant_id,
            latest_owner_revision: self
                .revisions
                .last()
                .map(|revision| revision.owner_revision)
                .unwrap_or(0),
        }])
    }
}

#[derive(Clone, Debug, Serialize)]
struct CheckpointAuditRow {
    sequence: i64,
    owner_revision: i64,
    event_id: Uuid,
    outcome: String,
    observed_forum_documents: i64,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct RepairEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_used: bool,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn missing_owner_delivery_rebuilds_once_and_advances_checkpoint_contiguously()
-> TestResult<()> {
    let Some(evidence) = PostgresSearchEvidence::setup("missing_delivery").await? else {
        return Ok(());
    };

    let proof = run_missing_delivery_repair_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(RepairEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D6",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_missing_delivery_repair_proof(
    db: &DatabaseConnection,
) -> TestResult<ScenarioEvidence> {
    let tenant_id = Uuid::new_v4();
    let revision_one_event_id = Uuid::new_v4();
    let missing_revision_event_id = Uuid::new_v4();
    let revision_three_event_id = Uuid::new_v4();
    let stale_document_id = Uuid::new_v4();
    let repaired_document_id = Uuid::new_v4();

    insert_completed_delivery(db, tenant_id, revision_one_event_id).await?;
    insert_completed_delivery(db, tenant_id, revision_three_event_id).await?;
    insert_stale_forum_document(db, tenant_id, stale_document_id).await?;

    let revisions = vec![
        owner_revision(1, revision_one_event_id),
        owner_revision(2, missing_revision_event_id),
        owner_revision(3, revision_three_event_id),
    ];
    let projection_source = Arc::new(ControlledForumSource::new(tenant_id, repaired_document_id));
    let owner_source = Arc::new(FixedOwnerRevisionSource::new(tenant_id, revisions));
    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        db.clone(),
        projection_source.clone(),
        owner_source.clone(),
    );

    let failed = reconciler.sweep_due(8, 8).await?;
    if failed.owner_tenants_scanned != 1
        || failed.owner_tenants_failed != 1
        || failed.owner_rebuilds != 0
        || failed.owner_revisions_checkpointed != 0
        || projection_source.list_calls() != 1
    {
        return Err(invalid_data(format!(
            "injected rebuild failure produced an unexpected sweep report: {failed:?}"
        ))
        .into());
    }
    if load_checkpoint(db, tenant_id).await?.is_some()
        || !load_checkpoint_audit(db, tenant_id).await?.is_empty()
    {
        return Err(invalid_data(
            "checkpoint advanced before the injected rebuild failure was resolved",
        )
        .into());
    }
    if count_forum_document(db, tenant_id, stale_document_id).await? != 1
        || count_forum_document(db, tenant_id, repaired_document_id).await? != 0
    {
        return Err(
            invalid_data("failed rebuild changed Search projection state before commit").into(),
        );
    }

    let repaired = reconciler.sweep_due(8, 8).await?;
    if repaired.owner_tenants_scanned != 1
        || repaired.owner_tenants_reconciled != 1
        || repaired.owner_tenants_failed != 0
        || repaired.owner_rebuilds != 1
        || repaired.owner_revisions_checkpointed != 3
        || projection_source.list_calls() != 2
    {
        return Err(invalid_data(format!(
            "successful missing-delivery repair produced an unexpected report: {repaired:?}"
        ))
        .into());
    }

    let checkpoint = load_checkpoint(db, tenant_id)
        .await?
        .ok_or_else(|| invalid_data("successful repair did not create an owner checkpoint"))?;
    if checkpoint.owner_revision != 3
        || checkpoint.event_id != revision_three_event_id
        || checkpoint.outcome != "rebuild_repaired"
    {
        return Err(invalid_data(format!(
            "successful repair stored an unexpected final checkpoint: {checkpoint:?}"
        ))
        .into());
    }

    let audit = load_checkpoint_audit(db, tenant_id).await?;
    let audited_revisions = audit
        .iter()
        .map(|row| row.owner_revision)
        .collect::<Vec<_>>();
    if audited_revisions != [1, 2, 3]
        || audit
            .iter()
            .any(|row| row.outcome != "rebuild_repaired" || row.observed_forum_documents != 1)
        || audit[0].event_id != revision_one_event_id
        || audit[1].event_id != missing_revision_event_id
        || audit[2].event_id != revision_three_event_id
    {
        return Err(invalid_data(format!(
            "checkpoint audit did not retain exact contiguous repair ordering: {audit:?}"
        ))
        .into());
    }
    if count_forum_document(db, tenant_id, stale_document_id).await? != 0
        || count_forum_document(db, tenant_id, repaired_document_id).await? != 1
    {
        return Err(invalid_data(
            "current-state rebuild did not replace stale Forum Search projection state",
        )
        .into());
    }

    let caught_up = reconciler.sweep_due(8, 8).await?;
    if caught_up.owner_tenants_scanned != 1
        || caught_up.owner_tenants_reconciled != 0
        || caught_up.owner_rebuilds != 0
        || caught_up.owner_revisions_checkpointed != 0
        || projection_source.list_calls() != 2
        || load_checkpoint_audit(db, tenant_id).await?.len() != 3
    {
        return Err(invalid_data(format!(
            "caught-up owner head unexpectedly repeated repair work: {caught_up:?}"
        ))
        .into());
    }

    Ok(ScenarioEvidence {
        id: "missing_delivery_owner_repair",
        result: "passed",
        facts: json!({
            "tenant_id": tenant_id,
            "owner_head_revision": 3,
            "covered_revision_event_ids": [revision_one_event_id, revision_three_event_id],
            "missing_revision": 2,
            "missing_revision_event_id": missing_revision_event_id,
            "failed_rebuild_left_checkpoint_absent": true,
            "failed_rebuild_left_audit_empty": true,
            "successful_rebuild_count": 1,
            "projection_source_total_calls_including_injected_failure": projection_source.list_calls(),
            "checkpoint_audit_revisions": audited_revisions,
            "checkpoint_audit_outcomes": audit.iter().map(|row| row.outcome.clone()).collect::<Vec<_>>(),
            "checkpoint_audit_document_counts": audit.iter().map(|row| row.observed_forum_documents).collect::<Vec<_>>(),
            "final_checkpoint_revision": checkpoint.owner_revision,
            "final_checkpoint_event_id": checkpoint.event_id,
            "final_checkpoint_outcome": checkpoint.outcome,
            "stale_document_removed": true,
            "current_document_inserted": true,
            "caught_up_repeat_rebuilds": 0,
            "owner_tenant_page_calls": owner_source.tenant_page_calls(),
            "owner_revision_page_calls": owner_source.revision_page_calls()
        }),
    })
}

fn owner_revision(owner_revision: i64, event_id: Uuid) -> ForumProjectionOwnerRevisionRecord {
    ForumProjectionOwnerRevisionRecord {
        owner_revision,
        event_id,
        event_type: ROOT_EVENT_TYPE.to_string(),
        impact: ForumProjectionOwnerRevisionImpact::FullRebuild,
    }
}

async fn create_checkpoint_audit(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE forum_missing_delivery_checkpoint_audit (
            sequence BIGSERIAL PRIMARY KEY,
            tenant_id UUID NOT NULL,
            owner_revision BIGINT NOT NULL,
            event_id UUID NOT NULL,
            outcome VARCHAR(32) NOT NULL,
            observed_forum_documents BIGINT NOT NULL
        );

        CREATE OR REPLACE FUNCTION forum_capture_missing_delivery_checkpoint()
        RETURNS trigger AS $$
        BEGIN
            INSERT INTO forum_missing_delivery_checkpoint_audit (
                tenant_id, owner_revision, event_id, outcome,
                observed_forum_documents
            ) VALUES (
                NEW.tenant_id,
                NEW.owner_revision,
                NEW.event_id,
                NEW.outcome,
                (
                    SELECT COUNT(*)::BIGINT
                    FROM search_documents
                    WHERE tenant_id = NEW.tenant_id
                      AND source_module = 'forum'
                )
            );
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;

        CREATE TRIGGER forum_missing_delivery_checkpoint_audit
        AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints
        FOR EACH ROW EXECUTE FUNCTION forum_capture_missing_delivery_checkpoint();
        "#,
    )
    .await?;
    Ok(())
}

async fn insert_completed_delivery(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event_id: Uuid,
) -> TestResult<()> {
    let envelope = root_envelope(tenant_id, event_id);
    envelope.validate_registered_schema()?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO search_projection_inbox (
            event_id, tenant_id, source_module, scope_key, event_type,
            revision_at, envelope_json, status, attempt_count,
            created_at, updated_at, completed_at
        ) VALUES (
            $1, $2, 'forum', 'forum', $3, $4, $5,
            'completed', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
        vec![
            event_id.into(),
            tenant_id.into(),
            ROOT_EVENT_TYPE.to_string().into(),
            envelope.timestamp.to_owned().into(),
            SqlValue::Json(Some(Box::new(serde_json::to_value(envelope)?))),
        ],
    ))
    .await?;
    Ok(())
}

fn root_envelope(tenant_id: Uuid, event_id: Uuid) -> EventEnvelope {
    EventEnvelope {
        id: event_id,
        event_type: ROOT_EVENT_TYPE.to_string(),
        schema_version: 1,
        correlation_id: event_id,
        causation_id: None,
        tenant_id,
        trace_id: None,
        timestamp: Utc::now(),
        actor_id: None,
        event: DomainEvent::ReindexRequested {
            target_type: "forum".to_string(),
            target_id: None,
        },
        retry_count: 0,
    }
}

async fn insert_stale_forum_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, slug, body, keywords_text,
            facets, payload, created_at, updated_at, indexed_at
        ) VALUES (
            'forum:{document_id}:en', '{tenant_id}', '{document_id}',
            'forum', 'forum_topic', 'en', 'published', TRUE,
            'Stale forum document', 'stale-forum-document', '', '',
            '{{}}'::jsonb, '{{"owner_state":"stale"}}'::jsonb,
            NOW(), NOW(), NOW()
        )
        "#
    ))
    .await?;
    Ok(())
}

#[derive(Debug)]
struct CheckpointSnapshot {
    owner_revision: i64,
    event_id: Uuid,
    outcome: String,
}

async fn load_checkpoint(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<Option<CheckpointSnapshot>, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT owner_revision, event_id, outcome
            FROM search_projection_owner_checkpoints
            WHERE tenant_id = $1 AND source_module = 'forum'
            "#,
            vec![tenant_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(CheckpointSnapshot {
            owner_revision: row.try_get("", "owner_revision")?,
            event_id: row.try_get("", "event_id")?,
            outcome: row.try_get("", "outcome")?,
        })
    })
    .transpose()
}

async fn load_checkpoint_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<Vec<CheckpointAuditRow>, sea_orm::DbErr> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT sequence, owner_revision, event_id, outcome,
               observed_forum_documents
        FROM forum_missing_delivery_checkpoint_audit
        WHERE tenant_id = $1
        ORDER BY sequence ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(CheckpointAuditRow {
            sequence: row.try_get("", "sequence")?,
            owner_revision: row.try_get("", "owner_revision")?,
            event_id: row.try_get("", "event_id")?,
            outcome: row.try_get("", "outcome")?,
            observed_forum_documents: row.try_get("", "observed_forum_documents")?,
        })
    })
    .collect()
}

async fn count_forum_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS value
            FROM search_documents
            WHERE tenant_id = $1
              AND document_id = $2
              AND source_module = 'forum'
            "#,
            vec![tenant_id.into(), document_id.into()],
        ))
        .await?
        .expect("Forum document count query must return one row");
    row.try_get("", "value")
}

fn postgres_database_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

fn database_url_in_schema(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-c%20search_path%3D{schema_name}%2Cpublic")
}

async fn connect(database_url: &str, max_connections: u32) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(max_connections)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}

fn write_evidence(artifact: RepairEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    Ok(())
}

fn source_commit() -> TestResult<String> {
    let output = Command::new("git")
        .current_dir(workspace_root())
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err(invalid_data("git rev-parse HEAD failed for evidence generation").into());
    }
    let value = String::from_utf8(output.stdout)?;
    let value = value.trim();
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_data("git rev-parse HEAD returned an invalid commit SHA").into());
    }
    Ok(value.to_string())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
