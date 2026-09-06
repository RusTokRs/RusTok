use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_search::{
    ForumSearchContractIngress, ForumSearchContractIngressError, ForumSearchContractIngressOutcome,
    SearchModule,
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
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_postgres_ingress_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-postgres-ingress-evidence.json";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum versioned invalidation ingress proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;
        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
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

#[derive(Clone, Debug, PartialEq, Serialize)]
struct InboxSnapshot {
    event_id: Uuid,
    tenant_id: Uuid,
    source_module: String,
    scope_key: String,
    event_type: String,
    ingest_sequence: i64,
    envelope_json: JsonValue,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct IngressEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_used: bool,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn versioned_forum_invalidation_converges_on_one_postgres_inbox_identity() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_versioned_ingress").await? else {
        return Ok(());
    };

    let proof = run_ingress_proof(&test_db.db).await;
    let cleanup = test_db.cleanup().await;
    let scenarios = proof?;
    cleanup?;

    write_evidence(IngressEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D2",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: scenarios,
    })?;

    Ok(())
}

async fn run_ingress_proof(db: &DatabaseConnection) -> TestResult<Vec<ScenarioEvidence>> {
    let mut scenarios = Vec::new();

    let typed_tenant_id = Uuid::new_v4();
    let typed_root_id = Uuid::new_v4();
    let typed_category_id = Uuid::new_v4();
    let typed = typed_invalidation(
        typed_tenant_id,
        typed_root_id,
        1,
        "forum_category",
        Some(typed_category_id),
    )?;
    let typed_envelope_id = typed.id();
    if typed.causation_id() != Some(typed_root_id) {
        return Err(test_error(
            "typed invalidation must retain the exact legacy root causation ID",
        ));
    }
    let outcome = ForumSearchContractIngress::new(db.clone())
        .ingest(&typed)
        .await?;
    ensure_durable_outcome(outcome, typed_root_id, 1)?;
    let typed_snapshot = load_snapshot(db, typed_root_id).await?;
    ensure_snapshot(
        &typed_snapshot,
        typed_tenant_id,
        typed_root_id,
        &format!("forum_category:{typed_category_id}"),
        "forum_category",
        Some(typed_category_id),
    )?;
    if typed_envelope_id == typed_root_id {
        return Err(test_error(
            "typed transport envelope ID must differ from the root projection identity",
        ));
    }
    if count_event_rows(db, typed_root_id).await? != 1 {
        return Err(test_error(
            "typed ingress must create exactly one durable inbox row",
        ));
    }
    scenarios.push(ScenarioEvidence {
        id: "typed_ingress_admission",
        result: "passed",
        facts: json!({
            "tenant_id": typed_tenant_id,
            "root_event_id": typed_root_id,
            "typed_envelope_id": typed_envelope_id,
            "owner_revision": 1,
            "scope_key": typed_snapshot.scope_key,
            "ingest_sequence": typed_snapshot.ingest_sequence,
            "inbox_rows": 1
        }),
    });

    let legacy_first_tenant_id = Uuid::new_v4();
    let legacy_first_root_id = Uuid::new_v4();
    let legacy_first_category_id = Uuid::new_v4();
    let legacy_first_root = root_envelope(
        legacy_first_tenant_id,
        legacy_first_root_id,
        "forum_category",
        Some(legacy_first_category_id),
    );
    insert_legacy_root(
        db,
        &legacy_first_root,
        &format!("forum_category:{legacy_first_category_id}"),
    )
    .await?;
    let legacy_first_before = load_snapshot(db, legacy_first_root_id).await?;
    ensure_snapshot(
        &legacy_first_before,
        legacy_first_tenant_id,
        legacy_first_root_id,
        &format!("forum_category:{legacy_first_category_id}"),
        "forum_category",
        Some(legacy_first_category_id),
    )?;
    let legacy_first_typed = typed_invalidation(
        legacy_first_tenant_id,
        legacy_first_root_id,
        2,
        "forum_category",
        Some(legacy_first_category_id),
    )?;
    let outcome = ForumSearchContractIngress::new(db.clone())
        .ingest(&legacy_first_typed)
        .await?;
    ensure_durable_outcome(outcome, legacy_first_root_id, 2)?;
    let legacy_first_after = load_snapshot(db, legacy_first_root_id).await?;
    if legacy_first_after != legacy_first_before {
        return Err(test_error(
            "typed duplicate must not replace the exact legacy-first durable row",
        ));
    }
    if count_event_rows(db, legacy_first_root_id).await? != 1 {
        return Err(test_error(
            "legacy-first duplicate must retain exactly one inbox row",
        ));
    }
    scenarios.push(ScenarioEvidence {
        id: "legacy_first_duplicate",
        result: "passed",
        facts: json!({
            "tenant_id": legacy_first_tenant_id,
            "root_event_id": legacy_first_root_id,
            "typed_envelope_id": legacy_first_typed.id(),
            "owner_revision": 2,
            "scope_key": legacy_first_after.scope_key,
            "ingest_sequence_before": legacy_first_before.ingest_sequence,
            "ingest_sequence_after": legacy_first_after.ingest_sequence,
            "inbox_rows": 1,
            "durable_root_preserved": true
        }),
    });

    let typed_first_tenant_id = Uuid::new_v4();
    let typed_first_root_id = Uuid::new_v4();
    let typed_first_category_id = Uuid::new_v4();
    let typed_first_typed = typed_invalidation(
        typed_first_tenant_id,
        typed_first_root_id,
        3,
        "forum_category",
        Some(typed_first_category_id),
    )?;
    let outcome = ForumSearchContractIngress::new(db.clone())
        .ingest(&typed_first_typed)
        .await?;
    ensure_durable_outcome(outcome, typed_first_root_id, 3)?;
    let typed_first_before = load_snapshot(db, typed_first_root_id).await?;
    ensure_snapshot(
        &typed_first_before,
        typed_first_tenant_id,
        typed_first_root_id,
        &format!("forum_category:{typed_first_category_id}"),
        "forum_category",
        Some(typed_first_category_id),
    )?;
    let typed_first_root = root_envelope(
        typed_first_tenant_id,
        typed_first_root_id,
        "forum_category",
        Some(typed_first_category_id),
    );
    insert_legacy_root(
        db,
        &typed_first_root,
        &format!("forum_category:{typed_first_category_id}"),
    )
    .await?;
    let typed_first_after = load_snapshot(db, typed_first_root_id).await?;
    if typed_first_after != typed_first_before {
        return Err(test_error(
            "legacy duplicate must not replace the typed-first durable row",
        ));
    }
    if count_event_rows(db, typed_first_root_id).await? != 1 {
        return Err(test_error(
            "typed-first duplicate must retain exactly one inbox row",
        ));
    }
    scenarios.push(ScenarioEvidence {
        id: "typed_first_duplicate",
        result: "passed",
        facts: json!({
            "tenant_id": typed_first_tenant_id,
            "root_event_id": typed_first_root_id,
            "typed_envelope_id": typed_first_typed.id(),
            "owner_revision": 3,
            "scope_key": typed_first_after.scope_key,
            "ingest_sequence_before": typed_first_before.ingest_sequence,
            "ingest_sequence_after": typed_first_after.ingest_sequence,
            "inbox_rows": 1,
            "typed_created_row_preserved": true
        }),
    });

    let expected_tenant_id = Uuid::new_v4();
    let conflicting_tenant_id = Uuid::new_v4();
    let conflict_root_id = Uuid::new_v4();
    let conflict_category_id = Uuid::new_v4();
    let conflicting_root = root_envelope(conflicting_tenant_id, conflict_root_id, "forum", None);
    insert_legacy_root(db, &conflicting_root, "forum").await?;
    let conflict_before = load_snapshot(db, conflict_root_id).await?;
    ensure_snapshot(
        &conflict_before,
        conflicting_tenant_id,
        conflict_root_id,
        "forum",
        "forum",
        None,
    )?;
    let conflict_typed = typed_invalidation(
        expected_tenant_id,
        conflict_root_id,
        4,
        "forum_category",
        Some(conflict_category_id),
    )?;
    let error = ForumSearchContractIngress::new(db.clone())
        .ingest(&conflict_typed)
        .await
        .expect_err("mismatched durable identity must fail closed");
    let stable_error_code = error.stable_code();
    if !matches!(
        error,
        ForumSearchContractIngressError::InboxIdentityConflict
    ) || stable_error_code != "forum.search_projection.contract_inbox_identity_conflict"
    {
        return Err(test_error(format!(
            "unexpected identity conflict classification: {stable_error_code}"
        )));
    }
    let conflict_after = load_snapshot(db, conflict_root_id).await?;
    if conflict_after != conflict_before || count_event_rows(db, conflict_root_id).await? != 1 {
        return Err(test_error(
            "semantic identity conflict must not replace or duplicate the durable row",
        ));
    }
    scenarios.push(ScenarioEvidence {
        id: "semantic_identity_conflict",
        result: "passed",
        facts: json!({
            "expected_tenant_id": expected_tenant_id,
            "durable_tenant_id": conflicting_tenant_id,
            "root_event_id": conflict_root_id,
            "typed_envelope_id": conflict_typed.id(),
            "owner_revision": 4,
            "durable_scope_key": conflict_after.scope_key,
            "requested_scope_key": format!("forum_category:{conflict_category_id}"),
            "ingest_sequence": conflict_after.ingest_sequence,
            "inbox_rows": 1,
            "stable_error_code": stable_error_code,
            "durable_row_preserved": true
        }),
    });

    Ok(scenarios)
}

fn typed_invalidation(
    tenant_id: Uuid,
    root_event_id: Uuid,
    owner_revision: i64,
    target_type: &str,
    target_id: Option<Uuid>,
) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_caused_by(
        tenant_id,
        None,
        root_event_id,
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision,
            target_type: target_type.to_string(),
            target_id,
        },
    )?)
}

fn root_envelope(
    tenant_id: Uuid,
    root_event_id: Uuid,
    target_type: &str,
    target_id: Option<Uuid>,
) -> EventEnvelope {
    EventEnvelope {
        id: root_event_id,
        event_type: ROOT_EVENT_TYPE.to_string(),
        schema_version: 1,
        correlation_id: root_event_id,
        causation_id: None,
        tenant_id,
        trace_id: None,
        timestamp: Utc::now(),
        actor_id: None,
        event: DomainEvent::ReindexRequested {
            target_type: target_type.to_string(),
            target_id,
        },
        retry_count: 0,
    }
}

async fn insert_legacy_root(
    db: &DatabaseConnection,
    envelope: &EventEnvelope,
    scope_key: &str,
) -> TestResult<()> {
    envelope.validate_registered_schema()?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO search_projection_inbox (
            event_id, tenant_id, source_module, scope_key, event_type,
            revision_at, envelope_json, status, attempt_count, created_at, updated_at
        ) VALUES ($1, $2, 'forum', $3, $4, $5, $6, 'pending', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (event_id) DO NOTHING
        "#,
        vec![
            envelope.id.into(),
            envelope.tenant_id.into(),
            scope_key.to_string().into(),
            envelope.event_type.clone().into(),
            envelope.timestamp.to_owned().into(),
            SqlValue::Json(Some(Box::new(serde_json::to_value(envelope)?))),
        ],
    ))
    .await?;
    Ok(())
}

async fn load_snapshot(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxSnapshot> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT event_id, tenant_id, source_module, scope_key, event_type,
                   ingest_sequence, envelope_json
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("inbox row {event_id} was not found")))?;

    Ok(InboxSnapshot {
        event_id: row.try_get("", "event_id")?,
        tenant_id: row.try_get("", "tenant_id")?,
        source_module: row.try_get("", "source_module")?,
        scope_key: row.try_get("", "scope_key")?,
        event_type: row.try_get("", "event_type")?,
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        envelope_json: row.try_get("", "envelope_json")?,
    })
}

async fn count_event_rows(db: &DatabaseConnection, event_id: Uuid) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_projection_inbox WHERE event_id = $1",
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error("inbox count query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn ensure_durable_outcome(
    outcome: ForumSearchContractIngressOutcome,
    expected_root_event_id: Uuid,
    expected_owner_revision: i64,
) -> TestResult<()> {
    match outcome {
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id,
            owner_revision,
        } if root_event_id == expected_root_event_id
            && owner_revision == expected_owner_revision =>
        {
            Ok(())
        }
        other => Err(test_error(format!(
            "unexpected durable ingress outcome: {other:?}"
        ))),
    }
}

fn ensure_snapshot(
    snapshot: &InboxSnapshot,
    expected_tenant_id: Uuid,
    expected_root_event_id: Uuid,
    expected_scope_key: &str,
    expected_target_type: &str,
    expected_target_id: Option<Uuid>,
) -> TestResult<()> {
    let stored_envelope: EventEnvelope = serde_json::from_value(snapshot.envelope_json.clone())?;
    let expected_event = DomainEvent::ReindexRequested {
        target_type: expected_target_type.to_string(),
        target_id: expected_target_id,
    };
    if snapshot.event_id != expected_root_event_id
        || snapshot.tenant_id != expected_tenant_id
        || snapshot.source_module != "forum"
        || snapshot.scope_key != expected_scope_key
        || snapshot.event_type != ROOT_EVENT_TYPE
        || snapshot.ingest_sequence <= 0
        || stored_envelope.id != expected_root_event_id
        || stored_envelope.tenant_id != expected_tenant_id
        || stored_envelope.event_type != ROOT_EVENT_TYPE
        || stored_envelope.schema_version != 1
        || stored_envelope.correlation_id != expected_root_event_id
        || stored_envelope.causation_id.is_some()
        || stored_envelope.event != expected_event
        || stored_envelope.validate_registered_schema().is_err()
    {
        return Err(test_error(format!(
            "unexpected durable inbox snapshot: {snapshot:?}"
        )));
    }
    Ok(())
}

fn write_evidence(artifact: IngressEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| test_error("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
    eprintln!(
        "wrote Forum Search PostgreSQL ingress evidence to {}",
        path.display()
    );
    Ok(())
}

fn source_commit() -> TestResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root())
        .output()?;
    if !output.status.success() {
        return Err(test_error(format!(
            "git rev-parse HEAD failed with status {}",
            output.status
        )));
    }
    let commit = String::from_utf8(output.stdout)?.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(test_error(format!(
            "git rev-parse HEAD returned an invalid source commit: {commit}"
        )));
    }
    Ok(commit)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn postgres_database_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
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

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}
