use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::PortError;
use rustok_core::{Error as CoreError, MigrationSource, Result as CoreResult};
use rustok_search::{
    ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionRecord,
    ForumProjectionOwnerRevisionRequest, ForumProjectionOwnerRevisionSourcePort,
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    ForumProjectionReconciler, SearchModule, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use tokio::time::sleep;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const CHILD_DATABASE_ENV: &str = "RUSTOK_FORUM_D7_DATABASE_URL";
const CHILD_SCHEMA_ENV: &str = "RUSTOK_FORUM_D7_SCHEMA";
const CHILD_ROLE_ENV: &str = "RUSTOK_FORUM_D7_ROLE";
const CHILD_TEST_NAME: &str = "forum_multi_process_child";
const HOLDER_ROLE: &str = "holder";
const CONTENDER_ROLE: &str = "contender";
const NEXT_ROLE: &str = "next";
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_multi_process_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-multi-process-evidence.json";

struct PostgresMultiProcessEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresMultiProcessEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search multi-process proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_multi_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect_in_schema(&database_url, &schema_name, 8).await?;
        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            create_fixture_schema(&db).await
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
            database_url,
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

#[derive(Clone)]
struct DatabaseOwnerSource {
    db: DatabaseConnection,
}

#[async_trait]
impl ForumProjectionOwnerRevisionSourcePort for DatabaseOwnerSource {
    async fn list_owner_revisions(
        &self,
        request: ForumProjectionOwnerRevisionRequest,
    ) -> Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT owner_revision, event_id
                FROM forum_d7_owner_revisions
                WHERE tenant_id = $1
                  AND owner_revision > $2
                ORDER BY owner_revision ASC
                LIMIT $3
                "#,
                vec![
                    request.tenant_id.into(),
                    request.after_owner_revision.into(),
                    (request.limit as i64).into(),
                ],
            ))
            .await
            .map_err(|_| owner_source_unavailable())?;
        rows.into_iter()
            .map(|row| {
                Ok(ForumProjectionOwnerRevisionRecord {
                    owner_revision: row
                        .try_get("", "owner_revision")
                        .map_err(|_| owner_source_unavailable())?,
                    event_id: row
                        .try_get("", "event_id")
                        .map_err(|_| owner_source_unavailable())?,
                    event_type: "index.reindex_requested".to_string(),
                    impact: ForumProjectionOwnerRevisionImpact::FullRebuild,
                })
            })
            .collect()
    }

    async fn list_owner_revision_tenants(
        &self,
        request: ForumProjectionOwnerTenantPageRequest,
    ) -> Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
        let rows = match request.after_tenant_id {
            Some(after_tenant_id) => {
                self.db
                    .query_all_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        r#"
                        SELECT tenant_id, latest_owner_revision
                        FROM forum_d7_owner_tenants
                        WHERE tenant_id > $1
                        ORDER BY tenant_id ASC
                        LIMIT $2
                        "#,
                        vec![after_tenant_id.into(), (request.limit as i64).into()],
                    ))
                    .await
            }
            None => {
                self.db
                    .query_all_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        r#"
                        SELECT tenant_id, latest_owner_revision
                        FROM forum_d7_owner_tenants
                        ORDER BY tenant_id ASC
                        LIMIT $1
                        "#,
                        vec![(request.limit as i64).into()],
                    ))
                    .await
            }
        }
        .map_err(|_| owner_source_unavailable())?;

        rows.into_iter()
            .map(|row| {
                Ok(ForumProjectionOwnerTenantHead {
                    tenant_id: row
                        .try_get("", "tenant_id")
                        .map_err(|_| owner_source_unavailable())?,
                    latest_owner_revision: row
                        .try_get("", "latest_owner_revision")
                        .map_err(|_| owner_source_unavailable())?,
                })
            })
            .collect()
    }
}

fn owner_source_unavailable() -> PortError {
    PortError::unavailable(
        "forum.search_projection_owner_revision.multi_process_fixture_unavailable",
        "Forum owner revision fixture is temporarily unavailable",
    )
}

#[derive(Clone)]
struct DatabaseProjectionSource {
    db: DatabaseConnection,
    role: String,
}

#[async_trait]
impl SearchProjectionSource for DatabaseProjectionSource {
    fn source_module(&self) -> &'static str {
        "forum"
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        _limit: usize,
    ) -> CoreResult<SearchProjectionPage> {
        if after.is_some() {
            return Err(CoreError::Validation(
                "multi-process proof received an unexpected projection cursor".to_string(),
            ));
        }
        if self.role == CONTENDER_ROLE {
            return Err(CoreError::External(
                "contender reached the projector despite advisory-lock exclusion".to_string(),
            ));
        }

        self.db
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE forum_d7_coordination
                SET rebuild_calls = rebuild_calls + 1,
                    holder_entered = holder_entered OR $2,
                    updated_at = CURRENT_TIMESTAMP
                WHERE tenant_id = $1
                "#,
                vec![tenant_id.into(), (self.role == HOLDER_ROLE).into()],
            ))
            .await
            .map_err(CoreError::Database)?;

        if self.role == HOLDER_ROLE && tenant_id == first_tenant_id() {
            wait_for_holder_release(&self.db, tenant_id).await?;
        }

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT document_id, title, slug
                FROM forum_d7_projection_documents
                WHERE tenant_id = $1
                "#,
                vec![tenant_id.into()],
            ))
            .await
            .map_err(CoreError::Database)?
            .ok_or_else(|| {
                CoreError::External(
                    "multi-process projection fixture returned no current document".to_string(),
                )
            })?;
        let document_id: Uuid = row
            .try_get("", "document_id")
            .map_err(CoreError::Database)?;
        let title: String = row.try_get("", "title").map_err(CoreError::Database)?;
        let slug: String = row.try_get("", "slug").map_err(CoreError::Database)?;
        let now = Utc::now();
        Ok(SearchProjectionPage {
            documents: vec![SearchProjectionDocument {
                document_key: format!("forum:{document_id}:en"),
                tenant_id,
                document_id,
                source_module: "forum".to_string(),
                entity_type: "forum_topic".to_string(),
                locale: "en".to_string(),
                status: "published".to_string(),
                is_public: true,
                title,
                subtitle: None,
                slug: Some(slug),
                handle: None,
                body: "Current multi-process owner state".to_string(),
                keywords_text: "forum multi process".to_string(),
                facets: json!({"multi_process": true}),
                payload: json!({"owner_state": "current"}),
                published_at: Some(now),
                created_at: now,
                updated_at: now,
            }],
            next_cursor: None,
        })
    }

    async fn load_public_entity(
        &self,
        _tenant_id: Uuid,
        _entity_type: &str,
        _entity_id: Uuid,
    ) -> CoreResult<Vec<SearchProjectionDocument>> {
        Ok(Vec::new())
    }
}

async fn wait_for_holder_release(db: &DatabaseConnection, tenant_id: Uuid) -> CoreResult<()> {
    let started = Instant::now();
    loop {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT release_holder FROM forum_d7_coordination WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await
            .map_err(CoreError::Database)?
            .ok_or_else(|| {
                CoreError::External("multi-process coordination row disappeared".to_string())
            })?;
        let released: bool = row
            .try_get("", "release_holder")
            .map_err(CoreError::Database)?;
        if released {
            return Ok(());
        }
        if started.elapsed() >= PROCESS_TIMEOUT {
            return Err(CoreError::External(
                "holder timed out waiting for the parent release".to_string(),
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProcessReport {
    role: String,
    process_id: i64,
    owner_tenants_scanned: i64,
    owner_tenants_reconciled: i64,
    owner_tenants_blocked: i64,
    owner_tenants_failed: i64,
    owner_revisions_checkpointed: i64,
    owner_rebuilds: i64,
}

#[derive(Clone, Debug, Serialize)]
struct CheckpointAuditRow {
    tenant_id: Uuid,
    owner_revision: i64,
    event_id: Uuid,
    outcome: String,
    observed_forum_documents: i64,
}

#[derive(Clone, Debug, Serialize)]
struct CursorAuditRow {
    previous_tenant_id: Option<Uuid>,
    next_tenant_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct MultiProcessEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    host_package: &'static str,
    os_processes: usize,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn multi_process_serialization_preserves_tenant_and_cursor_order() -> TestResult<()> {
    if env::var(CHILD_ROLE_ENV).is_ok() {
        return Ok(());
    }
    let Some(evidence) = PostgresMultiProcessEvidence::setup("serialization").await? else {
        return Ok(());
    };

    let proof = run_multi_process_proof(&evidence).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(MultiProcessEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D7",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        host_package: "rustok-server",
        os_processes: 3,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

#[tokio::test]
async fn forum_multi_process_child() -> TestResult<()> {
    let Ok(role) = env::var(CHILD_ROLE_ENV) else {
        return Ok(());
    };
    let database_url = env::var(CHILD_DATABASE_ENV)?;
    let schema_name = env::var(CHILD_SCHEMA_ENV)?;
    if !matches!(role.as_str(), HOLDER_ROLE | CONTENDER_ROLE | NEXT_ROLE) {
        return Err(invalid_data(format!("unsupported D7 child role `{role}`")).into());
    }

    let db = connect_in_schema(&database_url, &schema_name, 6).await?;
    let projection_source = Arc::new(DatabaseProjectionSource {
        db: db.clone(),
        role: role.clone(),
    });
    let owner_source = Arc::new(DatabaseOwnerSource { db: db.clone() });
    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        db.clone(),
        projection_source,
        owner_source,
    );
    let report = reconciler.sweep_due(1, 8).await?;
    let process_report = ProcessReport {
        role: role.clone(),
        process_id: i64::from(std::process::id()),
        owner_tenants_scanned: report.owner_tenants_scanned as i64,
        owner_tenants_reconciled: report.owner_tenants_reconciled as i64,
        owner_tenants_blocked: report.owner_tenants_blocked as i64,
        owner_tenants_failed: report.owner_tenants_failed as i64,
        owner_revisions_checkpointed: report.owner_revisions_checkpointed as i64,
        owner_rebuilds: report.owner_rebuilds as i64,
    };
    ensure_role_report(&process_report)?;
    store_process_report(&db, &process_report).await?;
    Ok(())
}

fn ensure_role_report(report: &ProcessReport) -> TestResult<()> {
    let valid = match report.role.as_str() {
        HOLDER_ROLE => {
            report.owner_tenants_scanned == 1
                && report.owner_tenants_reconciled == 1
                && report.owner_tenants_blocked == 0
                && report.owner_tenants_failed == 0
                && report.owner_revisions_checkpointed == 2
                && report.owner_rebuilds == 1
        }
        CONTENDER_ROLE => {
            report.owner_tenants_scanned == 1
                && report.owner_tenants_reconciled == 0
                && report.owner_tenants_blocked == 1
                && report.owner_tenants_failed == 0
                && report.owner_revisions_checkpointed == 0
                && report.owner_rebuilds == 0
        }
        NEXT_ROLE => {
            report.owner_tenants_scanned == 1
                && report.owner_tenants_reconciled == 1
                && report.owner_tenants_blocked == 0
                && report.owner_tenants_failed == 0
                && report.owner_revisions_checkpointed == 1
                && report.owner_rebuilds == 1
        }
        _ => false,
    };
    if !valid {
        return Err(
            invalid_data(format!("unexpected multi-process sweep report: {report:?}")).into(),
        );
    }
    Ok(())
}

async fn run_multi_process_proof(
    evidence: &PostgresMultiProcessEvidence,
) -> TestResult<ScenarioEvidence> {
    insert_fixture_rows(&evidence.db).await?;

    let mut holder = spawn_child(evidence, HOLDER_ROLE)?;
    wait_for_holder_entry(&evidence.db, &mut holder).await?;

    let contender = spawn_child(evidence, CONTENDER_ROLE)?;
    wait_child_success(contender, CONTENDER_ROLE).await?;
    let contender_report = require_process_report(&evidence.db, CONTENDER_ROLE).await?;
    ensure_role_report(&contender_report)?;

    let cursor_during_holder = load_scan_cursor(&evidence.db).await?;
    let cursor_audit_during_holder = load_cursor_audit(&evidence.db).await?;
    if cursor_during_holder != Some(first_tenant_id())
        || cursor_audit_during_holder.len() != 1
        || cursor_audit_during_holder[0].previous_tenant_id.is_some()
        || cursor_audit_during_holder[0].next_tenant_id != Some(first_tenant_id())
    {
        return Err(invalid_data(format!(
            "contender did not establish the first scan cursor exactly once: cursor={cursor_during_holder:?}, audit={cursor_audit_during_holder:?}"
        ))
        .into());
    }
    if count_checkpoint_audit(&evidence.db, first_tenant_id()).await? != 0 {
        return Err(invalid_data(
            "blocked contender advanced the first tenant checkpoint while holder was active",
        )
        .into());
    }

    release_holder(&evidence.db).await?;
    wait_child_success(holder, HOLDER_ROLE).await?;
    let holder_report = require_process_report(&evidence.db, HOLDER_ROLE).await?;
    ensure_role_report(&holder_report)?;

    let cursor_after_holder = load_scan_cursor(&evidence.db).await?;
    let cursor_audit_after_holder = load_cursor_audit(&evidence.db).await?;
    if cursor_after_holder != Some(first_tenant_id()) || cursor_audit_after_holder.len() != 1 {
        return Err(invalid_data(format!(
            "stale holder regressed or duplicated the contender-owned cursor CAS: cursor={cursor_after_holder:?}, audit={cursor_audit_after_holder:?}"
        ))
        .into());
    }

    let first_checkpoint_audit = load_checkpoint_audit(&evidence.db, first_tenant_id()).await?;
    let first_revisions = first_checkpoint_audit
        .iter()
        .map(|row| row.owner_revision)
        .collect::<Vec<_>>();
    if first_revisions != [1, 2]
        || first_checkpoint_audit
            .iter()
            .any(|row| row.outcome != "rebuild_repaired" || row.observed_forum_documents != 1)
        || first_checkpoint_audit[0].event_id != first_event_one_id()
        || first_checkpoint_audit[1].event_id != first_event_two_id()
        || rebuild_calls(&evidence.db, first_tenant_id()).await? != 1
    {
        return Err(invalid_data(format!(
            "first tenant serialization did not retain one rebuild and exact checkpoint order: {first_checkpoint_audit:?}"
        ))
        .into());
    }

    let next = spawn_child(evidence, NEXT_ROLE)?;
    wait_child_success(next, NEXT_ROLE).await?;
    let next_report = require_process_report(&evidence.db, NEXT_ROLE).await?;
    ensure_role_report(&next_report)?;

    let final_cursor = load_scan_cursor(&evidence.db).await?;
    let final_cursor_audit = load_cursor_audit(&evidence.db).await?;
    if final_cursor != Some(second_tenant_id())
        || final_cursor_audit.len() != 2
        || final_cursor_audit[0].next_tenant_id != Some(first_tenant_id())
        || final_cursor_audit[1].previous_tenant_id != Some(first_tenant_id())
        || final_cursor_audit[1].next_tenant_id != Some(second_tenant_id())
    {
        return Err(invalid_data(format!(
            "scan cursor skipped or regressed after the concurrent first tenant: cursor={final_cursor:?}, audit={final_cursor_audit:?}"
        ))
        .into());
    }

    let second_checkpoint_audit = load_checkpoint_audit(&evidence.db, second_tenant_id()).await?;
    if second_checkpoint_audit.len() != 1
        || second_checkpoint_audit[0].owner_revision != 1
        || second_checkpoint_audit[0].event_id != second_event_one_id()
        || second_checkpoint_audit[0].outcome != "rebuild_repaired"
        || second_checkpoint_audit[0].observed_forum_documents != 1
        || rebuild_calls(&evidence.db, second_tenant_id()).await? != 1
    {
        return Err(invalid_data(format!(
            "second tenant was skipped or reconciled more than once: {second_checkpoint_audit:?}"
        ))
        .into());
    }

    if !projection_replaced(&evidence.db, first_tenant_id(), first_current_document_id()).await?
        || !projection_replaced(
            &evidence.db,
            second_tenant_id(),
            second_current_document_id(),
        )
        .await?
    {
        return Err(invalid_data(
            "serialized rebuilds did not replace both tenants with current projection state",
        )
        .into());
    }

    Ok(ScenarioEvidence {
        id: "multi_process_serialization",
        result: "passed",
        facts: json!({
            "host_package": "rustok-server",
            "independent_os_process_roles": [HOLDER_ROLE, CONTENDER_ROLE, NEXT_ROLE],
            "holder_process_id": holder_report.process_id,
            "contender_process_id": contender_report.process_id,
            "next_process_id": next_report.process_id,
            "first_tenant_id": first_tenant_id(),
            "second_tenant_id": second_tenant_id(),
            "contender_blocked_by_advisory_lock": true,
            "first_tenant_successful_rebuilds": 1,
            "first_tenant_checkpoint_revisions": first_revisions,
            "second_tenant_successful_rebuilds": 1,
            "second_tenant_checkpoint_revisions": [1],
            "scan_cursor_path": [first_tenant_id(), second_tenant_id()],
            "stale_holder_cursor_cas_suppressed": true,
            "tenant_skip_observed": false,
            "cursor_regression_observed": false,
            "checkpoint_regression_or_skip_observed": false
        }),
    })
}

fn spawn_child(evidence: &PostgresMultiProcessEvidence, role: &str) -> TestResult<Child> {
    let executable = env::current_exe()?;
    Ok(Command::new(executable)
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_DATABASE_ENV, &evidence.database_url)
        .env(CHILD_SCHEMA_ENV, &evidence.schema_name)
        .env(CHILD_ROLE_ENV, role)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?)
}

async fn wait_for_holder_entry(db: &DatabaseConnection, child: &mut Child) -> TestResult<()> {
    let started = Instant::now();
    loop {
        if holder_entered(db).await? {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(invalid_data(format!(
                "holder child exited before acquiring the projection path: {status}"
            ))
            .into());
        }
        if started.elapsed() >= PROCESS_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid_data("timed out waiting for holder process entry").into());
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_child_success(mut child: Child, role: &str) -> TestResult<()> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(
                invalid_data(format!("multi-process child `{role}` failed with {status}")).into(),
            );
        }
        if started.elapsed() >= PROCESS_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid_data(format!("multi-process child `{role}` timed out")).into());
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn create_fixture_schema(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE forum_d7_owner_tenants (
            tenant_id UUID PRIMARY KEY,
            latest_owner_revision BIGINT NOT NULL
        );
        CREATE TABLE forum_d7_owner_revisions (
            tenant_id UUID NOT NULL,
            owner_revision BIGINT NOT NULL,
            event_id UUID NOT NULL,
            PRIMARY KEY (tenant_id, owner_revision)
        );
        CREATE TABLE forum_d7_projection_documents (
            tenant_id UUID PRIMARY KEY,
            document_id UUID NOT NULL,
            title TEXT NOT NULL,
            slug TEXT NOT NULL
        );
        CREATE TABLE forum_d7_coordination (
            tenant_id UUID PRIMARY KEY,
            holder_entered BOOLEAN NOT NULL DEFAULT FALSE,
            release_holder BOOLEAN NOT NULL DEFAULT FALSE,
            rebuild_calls BIGINT NOT NULL DEFAULT 0,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE forum_d7_process_reports (
            role TEXT PRIMARY KEY,
            process_id BIGINT NOT NULL,
            owner_tenants_scanned BIGINT NOT NULL,
            owner_tenants_reconciled BIGINT NOT NULL,
            owner_tenants_blocked BIGINT NOT NULL,
            owner_tenants_failed BIGINT NOT NULL,
            owner_revisions_checkpointed BIGINT NOT NULL,
            owner_rebuilds BIGINT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE forum_d7_checkpoint_audit (
            sequence BIGSERIAL PRIMARY KEY,
            tenant_id UUID NOT NULL,
            owner_revision BIGINT NOT NULL,
            event_id UUID NOT NULL,
            outcome TEXT NOT NULL,
            observed_forum_documents BIGINT NOT NULL
        );
        CREATE TABLE forum_d7_scan_cursor_audit (
            sequence BIGSERIAL PRIMARY KEY,
            previous_tenant_id UUID NULL,
            next_tenant_id UUID NULL
        );

        CREATE OR REPLACE FUNCTION forum_d7_capture_checkpoint()
        RETURNS trigger AS $$
        BEGIN
            INSERT INTO forum_d7_checkpoint_audit (
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
        CREATE TRIGGER forum_d7_checkpoint_audit
        AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints
        FOR EACH ROW EXECUTE FUNCTION forum_d7_capture_checkpoint();

        CREATE OR REPLACE FUNCTION forum_d7_capture_scan_cursor()
        RETURNS trigger AS $$
        BEGIN
            INSERT INTO forum_d7_scan_cursor_audit (
                previous_tenant_id, next_tenant_id
            ) VALUES (
                CASE WHEN TG_OP = 'INSERT' THEN NULL ELSE OLD.after_tenant_id END,
                NEW.after_tenant_id
            );
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        CREATE TRIGGER forum_d7_scan_cursor_audit
        AFTER INSERT OR UPDATE ON search_projection_owner_scan_cursors
        FOR EACH ROW EXECUTE FUNCTION forum_d7_capture_scan_cursor();
        "#,
    )
    .await?;
    Ok(())
}

async fn insert_fixture_rows(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_d7_owner_tenants (tenant_id, latest_owner_revision)
        VALUES
            ('{}', 2),
            ('{}', 1);
        INSERT INTO forum_d7_owner_revisions (tenant_id, owner_revision, event_id)
        VALUES
            ('{}', 1, '{}'),
            ('{}', 2, '{}'),
            ('{}', 1, '{}');
        INSERT INTO forum_d7_projection_documents (tenant_id, document_id, title, slug)
        VALUES
            ('{}', '{}', 'Current first tenant topic', 'current-first-topic'),
            ('{}', '{}', 'Current second tenant topic', 'current-second-topic');
        INSERT INTO forum_d7_coordination (tenant_id)
        VALUES ('{}'), ('{}');

        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, slug, body, keywords_text,
            facets, payload, created_at, updated_at, indexed_at
        ) VALUES
            ('forum:{}:en', '{}', '{}', 'forum', 'forum_topic',
             'en', 'published', TRUE, 'Stale first topic', 'stale-first-topic',
             '', '', '{{}}'::jsonb, '{{"owner_state":"stale"}}'::jsonb,
             NOW(), NOW(), NOW()),
            ('forum:{}:en', '{}', '{}', 'forum', 'forum_topic',
             'en', 'published', TRUE, 'Stale second topic', 'stale-second-topic',
             '', '', '{{}}'::jsonb, '{{"owner_state":"stale"}}'::jsonb,
             NOW(), NOW(), NOW());
        "#,
        first_tenant_id(),
        second_tenant_id(),
        first_tenant_id(),
        first_event_one_id(),
        first_tenant_id(),
        first_event_two_id(),
        second_tenant_id(),
        second_event_one_id(),
        first_tenant_id(),
        first_current_document_id(),
        second_tenant_id(),
        second_current_document_id(),
        first_tenant_id(),
        second_tenant_id(),
        first_stale_document_id(),
        first_tenant_id(),
        first_stale_document_id(),
        second_stale_document_id(),
        second_tenant_id(),
        second_stale_document_id(),
    ))
    .await?;
    Ok(())
}

async fn store_process_report(
    db: &DatabaseConnection,
    report: &ProcessReport,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO forum_d7_process_reports (
            role, process_id, owner_tenants_scanned,
            owner_tenants_reconciled, owner_tenants_blocked,
            owner_tenants_failed, owner_revisions_checkpointed,
            owner_rebuilds
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        vec![
            report.role.clone().into(),
            report.process_id.into(),
            report.owner_tenants_scanned.into(),
            report.owner_tenants_reconciled.into(),
            report.owner_tenants_blocked.into(),
            report.owner_tenants_failed.into(),
            report.owner_revisions_checkpointed.into(),
            report.owner_rebuilds.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn require_process_report(db: &DatabaseConnection, role: &str) -> TestResult<ProcessReport> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT role, process_id, owner_tenants_scanned,
                   owner_tenants_reconciled, owner_tenants_blocked,
                   owner_tenants_failed, owner_revisions_checkpointed,
                   owner_rebuilds
            FROM forum_d7_process_reports
            WHERE role = $1
            "#,
            vec![role.to_string().into()],
        ))
        .await?
        .ok_or_else(|| invalid_data(format!("missing process report for `{role}`")))?;
    Ok(ProcessReport {
        role: row.try_get("", "role")?,
        process_id: row.try_get("", "process_id")?,
        owner_tenants_scanned: row.try_get("", "owner_tenants_scanned")?,
        owner_tenants_reconciled: row.try_get("", "owner_tenants_reconciled")?,
        owner_tenants_blocked: row.try_get("", "owner_tenants_blocked")?,
        owner_tenants_failed: row.try_get("", "owner_tenants_failed")?,
        owner_revisions_checkpointed: row.try_get("", "owner_revisions_checkpointed")?,
        owner_rebuilds: row.try_get("", "owner_rebuilds")?,
    })
}

async fn holder_entered(db: &DatabaseConnection) -> Result<bool, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT holder_entered FROM forum_d7_coordination WHERE tenant_id = $1",
            vec![first_tenant_id().into()],
        ))
        .await?
        .expect("first tenant coordination row must exist");
    row.try_get("", "holder_entered")
}

async fn release_holder(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE forum_d7_coordination SET release_holder = TRUE, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1",
        vec![first_tenant_id().into()],
    ))
    .await?;
    Ok(())
}

async fn rebuild_calls(db: &DatabaseConnection, tenant_id: Uuid) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT rebuild_calls FROM forum_d7_coordination WHERE tenant_id = $1",
            vec![tenant_id.into()],
        ))
        .await?
        .expect("tenant coordination row must exist");
    row.try_get("", "rebuild_calls")
}

async fn load_scan_cursor(db: &DatabaseConnection) -> Result<Option<Uuid>, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT after_tenant_id FROM search_projection_owner_scan_cursors WHERE source_module = 'forum'"
                .to_string(),
        ))
        .await?;
    row.map(|row| row.try_get("", "after_tenant_id"))
        .transpose()
        .map(Option::flatten)
}

async fn load_cursor_audit(db: &DatabaseConnection) -> Result<Vec<CursorAuditRow>, sea_orm::DbErr> {
    db.query_all_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT previous_tenant_id, next_tenant_id FROM forum_d7_scan_cursor_audit ORDER BY sequence ASC"
            .to_string(),
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(CursorAuditRow {
            previous_tenant_id: row.try_get("", "previous_tenant_id")?,
            next_tenant_id: row.try_get("", "next_tenant_id")?,
        })
    })
    .collect()
}

async fn load_checkpoint_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<Vec<CheckpointAuditRow>, sea_orm::DbErr> {
    db.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT tenant_id, owner_revision, event_id, outcome,
               observed_forum_documents
        FROM forum_d7_checkpoint_audit
        WHERE tenant_id = $1
        ORDER BY sequence ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(CheckpointAuditRow {
            tenant_id: row.try_get("", "tenant_id")?,
            owner_revision: row.try_get("", "owner_revision")?,
            event_id: row.try_get("", "event_id")?,
            outcome: row.try_get("", "outcome")?,
            observed_forum_documents: row.try_get("", "observed_forum_documents")?,
        })
    })
    .collect()
}

async fn count_checkpoint_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM forum_d7_checkpoint_audit WHERE tenant_id = $1",
            vec![tenant_id.into()],
        ))
        .await?
        .expect("checkpoint audit count must return one row");
    row.try_get("", "value")
}

async fn projection_replaced(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    current_document_id: Uuid,
) -> Result<bool, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS total,
                   COUNT(*) FILTER (WHERE document_id = $2)::BIGINT AS current_count
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'forum'
            "#,
            vec![tenant_id.into(), current_document_id.into()],
        ))
        .await?
        .expect("projection replacement query must return one row");
    let total: i64 = row.try_get("", "total")?;
    let current_count: i64 = row.try_get("", "current_count")?;
    Ok(total == 1 && current_count == 1)
}

fn first_tenant_id() -> Uuid {
    Uuid::from_u128(0x10000000000000000000000000000001)
}

fn second_tenant_id() -> Uuid {
    Uuid::from_u128(0x10000000000000000000000000000002)
}

fn first_event_one_id() -> Uuid {
    Uuid::from_u128(0x20000000000000000000000000000001)
}

fn first_event_two_id() -> Uuid {
    Uuid::from_u128(0x20000000000000000000000000000002)
}

fn second_event_one_id() -> Uuid {
    Uuid::from_u128(0x20000000000000000000000000000003)
}

fn first_stale_document_id() -> Uuid {
    Uuid::from_u128(0x30000000000000000000000000000001)
}

fn second_stale_document_id() -> Uuid {
    Uuid::from_u128(0x30000000000000000000000000000002)
}

fn first_current_document_id() -> Uuid {
    Uuid::from_u128(0x40000000000000000000000000000001)
}

fn second_current_document_id() -> Uuid {
    Uuid::from_u128(0x40000000000000000000000000000002)
}

fn postgres_database_url() -> Option<String> {
    env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect_in_schema(
    database_url: &str,
    schema_name: &str,
    max_connections: u32,
) -> TestResult<DatabaseConnection> {
    let schema_url = database_url_in_schema(database_url, schema_name);
    connect(&schema_url, max_connections).await
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

fn write_evidence(artifact: MultiProcessEvidenceArtifact) -> TestResult<()> {
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
