use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use rustok_core::{ModuleRegistry, SecurityContext, UserRole};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    ForumSearchProjectionEvent,
};
use rustok_forum::{CategoryService, CreateCategoryInput, ForumModule};
use rustok_search::{
    ForumProjectionReconciler, SearchModule, SharedForumProjectionOwnerRevisionSourcePort,
    search_projection_source_registry_from_extensions,
};
use rustok_server::auth::AuthConfig;
use rustok_server::common::settings::RustokSettings;
use rustok_server::services::module_event_dispatcher::
    build_shared_runtime_extensions_with_host_providers;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::MigratorTrait;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const TYPED_EVENT_TYPE: &str = "forum.search_projection.invalidation_issued";
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_search_disabled_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-search-disabled-evidence.json";

const DISABLED_SEARCH_DOCUMENTS: &str = "forum_d9_search_documents_disabled";
const DISABLED_SEARCH_INBOX: &str = "forum_d9_search_projection_inbox_disabled";
const DISABLED_SEARCH_CHECKPOINTS: &str =
    "forum_d9_search_projection_owner_checkpoints_disabled";
const DISABLED_SEARCH_SCAN_CURSORS: &str =
    "forum_d9_search_projection_owner_scan_cursors_disabled";

struct PostgresSearchDisabledEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchDisabledEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search-disabled recovery proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_search_disabled_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let schema_url = database_url_in_schema(&database_url, &schema_name);
        let db = connect(&schema_url, 8).await?;
        if let Err(error) = rustok_migrations::Migrator::up(&db, None).await {
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

#[derive(Clone, Debug, Serialize)]
struct LedgerRow {
    owner_revision: i64,
    event_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
struct OutboxRow {
    id: Uuid,
    event_type: String,
    payload: JsonValue,
}

#[derive(Clone, Debug, Serialize)]
struct SearchDocumentRow {
    document_key: String,
    document_id: Uuid,
    entity_type: String,
    locale: String,
    status: String,
    is_public: bool,
    title: String,
    slug: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CheckpointRow {
    owner_revision: i64,
    event_id: Uuid,
    outcome: String,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct SearchDisabledEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    search_profile: &'static str,
    broker_used: bool,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn forum_owner_commit_survives_search_disable_and_reconciles_after_enable(
) -> TestResult<()> {
    let Some(evidence) = PostgresSearchDisabledEvidence::setup("owner_recovery").await? else {
        return Ok(());
    };

    let proof = run_search_disabled_recovery_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(SearchDisabledEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D9",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        search_profile: "disabled_then_enabled",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_search_disabled_recovery_proof(
    db: &DatabaseConnection,
) -> TestResult<ScenarioEvidence> {
    let tenant_id = Uuid::new_v4();
    insert_tenant(db, tenant_id).await?;

    if count_rows(db, "search_documents").await? != 0
        || count_rows(db, "search_projection_inbox").await? != 0
        || count_rows(db, "search_projection_owner_checkpoints").await? != 0
    {
        return Err(invalid_data(
            "isolated Search-owned storage was not empty before the disabled-profile command",
        )
        .into());
    }

    disable_search_storage(db).await?;

    let mut disabled_settings = RustokSettings::default();
    disabled_settings.search.enabled = false;
    let disabled_runtime = ServerRuntimeContext::new(db.clone(), disabled_settings);
    if disabled_runtime
        .shared_contains::<SharedForumProjectionOwnerRevisionSourcePort>()
    {
        return Err(invalid_data(
            "Search-disabled runtime unexpectedly contained the Forum owner revision adapter",
        )
        .into());
    }

    let category_name = "Search-disabled recovery category";
    let category_slug = format!("search-disabled-{}", Uuid::new_v4().simple());
    let created = CategoryService::new(db.clone())
        .create(
            tenant_id,
            SecurityContext::new(UserRole::Admin, None),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: category_name.to_string(),
                slug: category_slug.clone(),
                description: Some(
                    "Forum owner state committed while Search runtime was unavailable".to_string(),
                ),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let ledger = load_single_ledger_row(db, tenant_id).await?;
    ensure_owner_state(db, tenant_id, created.id, category_name, &category_slug).await?;
    ensure_disabled_search_storage_untouched(db).await?;

    let outbox = load_projection_outbox_rows(db).await?;
    ensure_outbox_identity(&outbox, tenant_id, &ledger).await?;

    enable_search_storage(db).await?;

    let mut enabled_settings = RustokSettings::default();
    enabled_settings.search.enabled = true;
    let runtime_ctx = ServerRuntimeContext::new(db.clone(), enabled_settings.clone());
    let registry = ModuleRegistry::new()
        .register(rustok_index::IndexModule)
        .register(ForumModule)
        .register(SearchModule);
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &enabled_settings,
        runtime_ctx,
        AuthConfig::new("forum-d9-test-secret-key-at-least-32-bytes".to_string()),
    )?;
    let source_registry =
        search_projection_source_registry_from_extensions(extensions.as_ref()).ok_or_else(|| {
            invalid_data("Search-enabled runtime did not register the projection source registry")
        })?;
    let forum_source = source_registry
        .build("forum", db.clone())
        .ok_or_else(|| invalid_data("Search-enabled runtime did not build the Forum source"))?;
    let owner_source = extensions
        .get::<SharedForumProjectionOwnerRevisionSourcePort>()
        .cloned()
        .ok_or_else(|| {
            invalid_data(
                "Search-enabled runtime did not compose the production Forum owner revision adapter",
            )
        })?;
    let reconciler =
        ForumProjectionReconciler::with_owner_revision_source(db.clone(), forum_source, owner_source);

    let recovered = reconciler.sweep_due(1, 8).await?;
    if recovered.owner_tenants_scanned != 1
        || recovered.owner_tenants_reconciled != 1
        || recovered.owner_tenants_failed != 0
        || recovered.owner_tenants_blocked != 0
        || recovered.owner_rebuilds != 1
        || recovered.owner_revisions_checkpointed != 1
    {
        return Err(invalid_data(format!(
            "Search re-enable produced an unexpected bounded recovery report: {recovered:?}"
        ))
        .into());
    }

    let document = load_search_document(db, tenant_id, created.id)
        .await?
        .ok_or_else(|| invalid_data("Search re-enable did not rebuild the Forum category document"))?;
    if document.document_key != format!("forum_category:{}:en", created.id)
        || document.document_id != created.id
        || document.entity_type != "forum_category"
        || document.locale != "en"
        || document.status != "public"
        || !document.is_public
        || document.title != category_name
        || document.slug.as_deref() != Some(category_slug.as_str())
    {
        return Err(invalid_data(format!(
            "Search re-enable rebuilt an unexpected Forum document: {document:?}"
        ))
        .into());
    }

    let checkpoint = load_checkpoint(db, tenant_id)
        .await?
        .ok_or_else(|| invalid_data("Search re-enable did not create the owner checkpoint"))?;
    if checkpoint.owner_revision != 1
        || checkpoint.event_id != ledger.event_id
        || checkpoint.outcome != "rebuild_repaired"
    {
        return Err(invalid_data(format!(
            "Search re-enable stored an unexpected owner checkpoint: {checkpoint:?}"
        ))
        .into());
    }
    if count_rows(db, "search_projection_inbox").await? != 0 {
        return Err(invalid_data(
            "owner-ledger recovery created a second Search inbox execution path",
        )
        .into());
    }

    let caught_up = reconciler.sweep_due(1, 8).await?;
    if caught_up.owner_rebuilds != 0
        || caught_up.owner_revisions_checkpointed != 0
        || load_forum_document_count(db, tenant_id).await? != 1
    {
        return Err(invalid_data(format!(
            "caught-up Search-enabled sweep repeated owner recovery work: {caught_up:?}"
        ))
        .into());
    }

    Ok(ScenarioEvidence {
        id: "search_disabled_profile",
        result: "passed",
        facts: json!({
            "tenant_id": tenant_id,
            "category_id": created.id,
            "search_enabled_during_owner_command": false,
            "search_owned_tables_unavailable_during_owner_command": [
                "search_documents",
                "search_projection_inbox",
                "search_projection_owner_checkpoints",
                "search_projection_owner_scan_cursors"
            ],
            "owner_state_committed": true,
            "root_event_type": ROOT_EVENT_TYPE,
            "typed_event_type": TYPED_EVENT_TYPE,
            "owner_revision": ledger.owner_revision,
            "root_event_id": ledger.event_id,
            "outbox_rows": outbox.len(),
            "disabled_search_documents": 0,
            "disabled_search_inbox_rows": 0,
            "disabled_search_checkpoints": 0,
            "recovery_owner_tenants_reconciled": recovered.owner_tenants_reconciled,
            "recovery_owner_rebuilds": recovered.owner_rebuilds,
            "recovery_revisions_checkpointed": recovered.owner_revisions_checkpointed,
            "recovered_document_key": document.document_key,
            "checkpoint_outcome": checkpoint.outcome,
            "caught_up_repeat_rebuilds": caught_up.owner_rebuilds,
            "search_inbox_rows_after_recovery": 0
        }),
    })
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO tenants (
            id, name, slug, domain, settings, default_locale, is_active
        ) VALUES ($1, $2, $3, NULL, '{}'::jsonb, 'en', TRUE)
        "#,
        vec![
            tenant_id.into(),
            "Forum D9 evidence tenant".to_string().into(),
            format!("forum-d9-{}", tenant_id.simple()).into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn disable_search_storage(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
        ALTER TABLE search_documents RENAME TO {DISABLED_SEARCH_DOCUMENTS};
        ALTER TABLE search_projection_inbox RENAME TO {DISABLED_SEARCH_INBOX};
        ALTER TABLE search_projection_owner_checkpoints RENAME TO {DISABLED_SEARCH_CHECKPOINTS};
        ALTER TABLE search_projection_owner_scan_cursors RENAME TO {DISABLED_SEARCH_SCAN_CURSORS};
        "#
    ))
    .await?;
    Ok(())
}

async fn enable_search_storage(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
        ALTER TABLE {DISABLED_SEARCH_DOCUMENTS} RENAME TO search_documents;
        ALTER TABLE {DISABLED_SEARCH_INBOX} RENAME TO search_projection_inbox;
        ALTER TABLE {DISABLED_SEARCH_CHECKPOINTS} RENAME TO search_projection_owner_checkpoints;
        ALTER TABLE {DISABLED_SEARCH_SCAN_CURSORS} RENAME TO search_projection_owner_scan_cursors;
        "#
    ))
    .await?;
    Ok(())
}

async fn ensure_disabled_search_storage_untouched(db: &DatabaseConnection) -> TestResult<()> {
    for table in [
        DISABLED_SEARCH_DOCUMENTS,
        DISABLED_SEARCH_INBOX,
        DISABLED_SEARCH_CHECKPOINTS,
        DISABLED_SEARCH_SCAN_CURSORS,
    ] {
        if count_rows(db, table).await? != 0 {
            return Err(invalid_data(format!(
                "Search-disabled owner command changed Search-owned table {table}"
            ))
            .into());
        }
    }
    Ok(())
}

async fn ensure_owner_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    expected_name: &str,
    expected_slug: &str,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT c.id, t.name, t.slug
            FROM forum_categories c
            JOIN forum_category_translations t
              ON t.tenant_id = c.tenant_id
             AND t.category_id = c.id
            WHERE c.tenant_id = $1
              AND c.id = $2
              AND t.locale = 'en'
            "#,
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?
        .ok_or_else(|| invalid_data("Forum owner category did not commit"))?;
    let stored_id: Uuid = row.try_get("", "id")?;
    let stored_name: String = row.try_get("", "name")?;
    let stored_slug: String = row.try_get("", "slug")?;
    if stored_id != category_id || stored_name != expected_name || stored_slug != expected_slug {
        return Err(invalid_data(format!(
            "Forum owner state changed during disabled-profile commit: id={stored_id}, name={stored_name}, slug={stored_slug}"
        ))
        .into());
    }
    Ok(())
}

async fn load_single_ledger_row(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<LedgerRow> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT revision, event_id, target_type, target_id
            FROM forum_projection_revision_ledger
            WHERE tenant_id = $1
            ORDER BY revision ASC
            "#,
            vec![tenant_id.into()],
        ))
        .await?;
    if rows.len() != 1 {
        return Err(invalid_data(format!(
            "Search-disabled owner command created {} ledger rows instead of one",
            rows.len()
        ))
        .into());
    }
    let row = &rows[0];
    let ledger = LedgerRow {
        owner_revision: row.try_get("", "revision")?,
        event_id: row.try_get("", "event_id")?,
        target_type: row.try_get("", "target_type")?,
        target_id: row.try_get("", "target_id")?,
    };
    if ledger.owner_revision != 1
        || ledger.event_id.is_nil()
        || ledger.target_type != "forum"
        || ledger.target_id.is_some()
    {
        return Err(invalid_data(format!(
            "Search-disabled owner command stored an unexpected ledger identity: {ledger:?}"
        ))
        .into());
    }
    Ok(ledger)
}

async fn load_projection_outbox_rows(db: &DatabaseConnection) -> TestResult<Vec<OutboxRow>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, event_type, payload
            FROM sys_events
            WHERE event_type IN ($1, $2)
            ORDER BY event_type ASC
            "#,
            vec![
                ROOT_EVENT_TYPE.to_string().into(),
                TYPED_EVENT_TYPE.to_string().into(),
            ],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(OutboxRow {
                id: row.try_get("", "id")?,
                event_type: row.try_get("", "event_type")?,
                payload: row.try_get("", "payload")?,
            })
        })
        .collect()
}

async fn ensure_outbox_identity(
    rows: &[OutboxRow],
    tenant_id: Uuid,
    ledger: &LedgerRow,
) -> TestResult<()> {
    if rows.len() != 2 {
        return Err(invalid_data(format!(
            "Search-disabled owner command created {} projection outbox rows instead of two",
            rows.len()
        ))
        .into());
    }
    let root_row = rows
        .iter()
        .find(|row| row.event_type == ROOT_EVENT_TYPE)
        .ok_or_else(|| invalid_data("root projection invalidation outbox row is missing"))?;
    let typed_row = rows
        .iter()
        .find(|row| row.event_type == TYPED_EVENT_TYPE)
        .ok_or_else(|| invalid_data("typed projection invalidation outbox row is missing"))?;

    let root: EventEnvelope = serde_json::from_value(root_row.payload.clone())?;
    root.validate_registered_schema()?;
    if root_row.id != ledger.event_id
        || root.id != ledger.event_id
        || root.tenant_id != tenant_id
        || root.event_type != ROOT_EVENT_TYPE
    {
        return Err(invalid_data(format!(
            "root outbox identity did not match the owner ledger: {root:?}"
        ))
        .into());
    }
    match &root.event {
        DomainEvent::ReindexRequested {
            target_type,
            target_id,
        } if target_type == "forum" && target_id.is_none() => {}
        other => {
            return Err(invalid_data(format!(
                "root outbox payload was not the Forum scope invalidation: {other:?}"
            ))
            .into());
        }
    }

    let typed: ContractEventEnvelope = serde_json::from_value(typed_row.payload.clone())?;
    typed.validate_registered_schema()?;
    if typed_row.id != typed.id()
        || typed.tenant_id() != tenant_id
        || typed.causation_id() != Some(ledger.event_id)
        || typed.event_type() != TYPED_EVENT_TYPE
    {
        return Err(invalid_data(format!(
            "typed outbox identity did not retain exact owner causation: {typed:?}"
        ))
        .into());
    }
    match typed.payload()? {
        ContractEventPayload::ForumSearchProjection(
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision,
                target_type,
                target_id,
            },
        ) if *owner_revision == ledger.owner_revision
            && target_type == "forum"
            && target_id.is_none() => {}
        other => {
            return Err(invalid_data(format!(
                "typed outbox payload did not retain the owner revision: {other:?}"
            ))
            .into());
        }
    }
    Ok(())
}

async fn load_search_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
) -> TestResult<Option<SearchDocumentRow>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT document_key, document_id, entity_type, locale, status,
                   is_public, title, slug
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'forum'
              AND document_id = $2
            "#,
            vec![tenant_id.into(), document_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(SearchDocumentRow {
            document_key: row.try_get("", "document_key")?,
            document_id: row.try_get("", "document_id")?,
            entity_type: row.try_get("", "entity_type")?,
            locale: row.try_get("", "locale")?,
            status: row.try_get("", "status")?,
            is_public: row.try_get("", "is_public")?,
            title: row.try_get("", "title")?,
            slug: row.try_get("", "slug")?,
        })
    })
    .transpose()
}

async fn load_checkpoint(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Option<CheckpointRow>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT owner_revision, event_id, outcome
            FROM search_projection_owner_checkpoints
            WHERE tenant_id = $1
              AND source_module = 'forum'
            "#,
            vec![tenant_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(CheckpointRow {
            owner_revision: row.try_get("", "owner_revision")?,
            event_id: row.try_get("", "event_id")?,
            outcome: row.try_get("", "outcome")?,
        })
    })
    .transpose()
}

async fn load_forum_document_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS value
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'forum'
            "#,
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| invalid_data("Forum Search document count returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn count_rows(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    if !table
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_data("unsafe evidence table identifier").into());
    }
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("SELECT COUNT(*)::BIGINT AS value FROM {table}"),
            Vec::new(),
        ))
        .await?
        .ok_or_else(|| invalid_data(format!("count query returned no row for {table}")))?;
    Ok(row.try_get("", "value")?)
}

fn postgres_database_url() -> Option<String> {
    env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
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

fn write_evidence(artifact: SearchDisabledEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
    eprintln!(
        "wrote Forum Search-disabled recovery evidence to {}",
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
        return Err(invalid_data(format!(
            "git rev-parse HEAD failed with status {}",
            output.status
        ))
        .into());
    }
    let commit = String::from_utf8(output.stdout)?.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_data(format!(
            "git rev-parse HEAD returned an invalid source commit: {commit}"
        ))
        .into());
    }
    Ok(commit)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
