use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::PortError;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{ContractEventEnvelope, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumEventService,
    ForumModule, ForumProjectionOwnerRevisionImpact as ForumOwnerRevisionImpact,
    ForumSearchProjectionSourceFactory, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_search::{
    ForumProjectionOwnerRevisionImpact as SearchOwnerRevisionImpact,
    ForumProjectionOwnerRevisionRecord, ForumProjectionOwnerRevisionRequest,
    ForumProjectionOwnerRevisionSourcePort, ForumProjectionOwnerTenantHead,
    ForumProjectionOwnerTenantPageRequest, ForumProjectionReconciler, SearchModule,
    SearchProjectionSourceFactory,
};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const FORUM_TEST_DATABASE_ENV: &str = "RUSTOK_FORUM_TEST_DATABASE_URL";
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const TYPED_EVENT_TYPE: &str = "forum.search_projection.invalidation_issued";
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json";
const TOPIC_ONE_MARKER: &str = "d9searchdisabledtopicone";
const TOPIC_TWO_MARKER: &str = "d9searchdisabledtopictwo";

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

        let db = connect_in_schema(&database_url, &schema_name, 8).await?;
        let setup_result = async {
            db.execute_unprepared(
                r#"
                CREATE TABLE users (
                    id UUID NOT NULL PRIMARY KEY,
                    tenant_id UUID NOT NULL
                )
                "#,
            )
            .await?;
            let manager = SchemaManager::new(&db);
            for migration in OutboxModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in TaxonomyModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in ForumModule.migrations() {
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

    async fn enable_search(&self) -> TestResult<()> {
        let manager = SchemaManager::new(&self.db);
        for migration in SearchModule.migrations() {
            migration.up(&manager).await?;
        }
        create_checkpoint_audit(&self.db).await?;
        Ok(())
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
struct RealForumOwnerRevisionSource {
    db: DatabaseConnection,
}

#[async_trait]
impl ForumProjectionOwnerRevisionSourcePort for RealForumOwnerRevisionSource {
    async fn list_owner_revisions(
        &self,
        request: ForumProjectionOwnerRevisionRequest,
    ) -> Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
        let revisions = ForumEventService::new(self.db.clone())
            .list_projection_owner_revisions(
                request.tenant_id,
                request.after_owner_revision,
                request.limit,
            )
            .await
            .map_err(map_forum_owner_error)?;

        Ok(revisions
            .into_iter()
            .map(|revision| ForumProjectionOwnerRevisionRecord {
                owner_revision: revision.owner_revision,
                event_id: revision.event_id,
                event_type: revision.event_type,
                impact: match revision.impact {
                    ForumOwnerRevisionImpact::FullRebuild => SearchOwnerRevisionImpact::FullRebuild,
                },
            })
            .collect())
    }

    async fn list_owner_revision_tenants(
        &self,
        request: ForumProjectionOwnerTenantPageRequest,
    ) -> Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
        let heads = ForumEventService::new(self.db.clone())
            .list_projection_owner_revision_tenants(request.after_tenant_id, request.limit)
            .await
            .map_err(map_forum_owner_error)?;

        Ok(heads
            .into_iter()
            .map(|head| ForumProjectionOwnerTenantHead {
                tenant_id: head.tenant_id,
                latest_owner_revision: head.latest_owner_revision,
            })
            .collect())
    }
}

fn map_forum_owner_error(_error: ForumError) -> PortError {
    PortError::unavailable(
        "forum.search_projection_owner_revision.search_disabled_recovery_unavailable",
        "Forum projection owner revision source is temporarily unavailable",
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OwnerRevisionRow {
    revision: i64,
    event_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct TopicOwnerRow {
    id: Uuid,
    category_id: Uuid,
    status: String,
    title: String,
    slug: String,
    body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ForumOwnerSnapshot {
    category_id: Uuid,
    category_name: String,
    category_slug: String,
    category_description: String,
    topics: Vec<TopicOwnerRow>,
    revisions: Vec<OwnerRevisionRow>,
}

#[derive(Clone, Debug, Serialize)]
struct SearchDocumentRow {
    document_id: Uuid,
    entity_type: String,
    status: String,
    title: String,
    body: String,
}

#[derive(Clone, Debug, Serialize)]
struct CheckpointAuditRow {
    sequence: i64,
    owner_revision: i64,
    event_id: Uuid,
    outcome: String,
    observed_forum_documents: i64,
}

#[derive(Clone, Debug, Serialize)]
struct CheckpointSnapshot {
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
    broker_used: bool,
    scenario_results: Vec<ScenarioEvidence>,
}

#[derive(Clone, Copy, Debug)]
struct ForumFixture {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_one_id: Uuid,
    topic_two_id: Uuid,
}

#[tokio::test]
async fn search_disabled_forum_commands_reconcile_after_late_search_enable() -> TestResult<()> {
    let Some(evidence) = PostgresSearchDisabledEvidence::setup("late_enable").await? else {
        return Ok(());
    };

    let proof = run_search_disabled_recovery_proof(&evidence).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(SearchDisabledEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D9",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_search_disabled_recovery_proof(
    evidence: &PostgresSearchDisabledEvidence,
) -> TestResult<ScenarioEvidence> {
    let db = &evidence.db;
    assert_search_storage_absent(db).await?;

    let fixture = create_forum_fixture(db).await?;
    assert_search_storage_absent(db).await?;

    let before_enable = load_owner_snapshot(db, fixture).await?;
    assert_disabled_owner_shape(&before_enable, fixture)?;

    let revision_event_ids = before_enable
        .revisions
        .iter()
        .map(|revision| revision.event_id)
        .collect::<BTreeSet<_>>();
    let root_event_ids = load_root_event_ids(db, fixture.tenant_id).await?;
    let typed_causation_ids = load_typed_causation_ids(db, fixture.tenant_id).await?;
    if root_event_ids != revision_event_ids || typed_causation_ids != revision_event_ids {
        return Err(test_error(format!(
            "Search-disabled owner events lost the shared root identity: revisions={revision_event_ids:?}, roots={root_event_ids:?}, typed_causation={typed_causation_ids:?}"
        )));
    }

    evidence.enable_search().await?;
    assert_search_storage_present(db).await?;
    if count_rows(db, "search_projection_inbox").await? != 0
        || count_rows(db, "search_projection_owner_checkpoints").await? != 0
        || count_forum_documents(db, fixture.tenant_id).await? != 0
    {
        return Err(test_error(
            "late Search enable started with unexpected inbox, checkpoint or projection state",
        ));
    }

    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let owner_source = Arc::new(RealForumOwnerRevisionSource { db: db.clone() });
    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        db.clone(),
        projection_source,
        owner_source,
    );

    let recovered = reconciler.sweep_due(8, 8).await?;
    if recovered.due_tenants != 0
        || recovered.claimed_events != 0
        || recovered.completed_events != 0
        || recovered.failed_events != 0
        || recovered.owner_tenants_scanned != 1
        || recovered.owner_tenants_reconciled != 1
        || recovered.owner_tenants_blocked != 0
        || recovered.owner_tenants_failed != 0
        || recovered.owner_rebuilds != 1
        || recovered.owner_revisions_checkpointed != 3
    {
        return Err(test_error(format!(
            "late Search owner-ledger recovery produced an unexpected report: {recovered:?}"
        )));
    }

    let checkpoint = load_checkpoint(db, fixture.tenant_id)
        .await?
        .ok_or_else(|| test_error("late Search recovery did not create an owner checkpoint"))?;
    let expected_head = before_enable
        .revisions
        .last()
        .ok_or_else(|| test_error("owner revision fixture is empty"))?;
    if checkpoint.owner_revision != expected_head.revision
        || checkpoint.event_id != expected_head.event_id
        || checkpoint.outcome != "rebuild_repaired"
    {
        return Err(test_error(format!(
            "late Search recovery stored an unexpected checkpoint: {checkpoint:?}"
        )));
    }

    let audit = load_checkpoint_audit(db, fixture.tenant_id).await?;
    if audit
        .iter()
        .map(|row| row.owner_revision)
        .collect::<Vec<_>>()
        != [1, 2, 3]
        || audit.iter().map(|row| row.event_id).collect::<Vec<_>>()
            != before_enable
                .revisions
                .iter()
                .map(|revision| revision.event_id)
                .collect::<Vec<_>>()
        || audit
            .iter()
            .any(|row| row.outcome != "rebuild_repaired" || row.observed_forum_documents != 3)
    {
        return Err(test_error(format!(
            "late Search checkpoint audit did not retain exact rebuild-before-checkpoint ordering: {audit:?}"
        )));
    }

    let documents = load_forum_documents(db, fixture.tenant_id).await?;
    assert_recovered_documents(&documents, fixture)?;

    let after_recovery = load_owner_snapshot(db, fixture).await?;
    if after_recovery != before_enable {
        return Err(test_error(format!(
            "late Search recovery changed Forum owner state: before={before_enable:?}, after={after_recovery:?}"
        )));
    }
    if load_root_event_ids(db, fixture.tenant_id).await? != root_event_ids
        || load_typed_causation_ids(db, fixture.tenant_id).await? != typed_causation_ids
        || count_rows(db, "search_projection_inbox").await? != 0
    {
        return Err(test_error(
            "late Search recovery mutated owner events or synthesized Search inbox deliveries",
        ));
    }

    let caught_up = reconciler.sweep_due(8, 8).await?;
    if caught_up.owner_tenants_scanned != 1
        || caught_up.owner_tenants_reconciled != 0
        || caught_up.owner_tenants_failed != 0
        || caught_up.owner_rebuilds != 0
        || caught_up.owner_revisions_checkpointed != 0
        || load_checkpoint_audit(db, fixture.tenant_id).await?.len() != 3
        || count_forum_documents(db, fixture.tenant_id).await? != 3
    {
        return Err(test_error(format!(
            "caught-up late Search sweep repeated recovery work: {caught_up:?}"
        )));
    }

    Ok(ScenarioEvidence {
        id: "search_disabled_profile",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "search_tables_absent_during_owner_commands": [
                "search_projection_inbox",
                "search_projection_owner_checkpoints",
                "search_documents"
            ],
            "owner_commands_committed": [
                "category_create",
                "topic_one_create",
                "topic_two_create"
            ],
            "owner_revision_rows": before_enable.revisions,
            "legacy_root_event_ids": root_event_ids,
            "typed_causation_ids": typed_causation_ids,
            "owner_state_before_enable": before_enable,
            "inbox_rows_before_recovery": 0,
            "successful_owner_rebuilds": recovered.owner_rebuilds,
            "checkpointed_revisions": recovered.owner_revisions_checkpointed,
            "checkpoint_audit_revisions": audit.iter().map(|row| row.owner_revision).collect::<Vec<_>>(),
            "checkpoint_audit_outcomes": audit.iter().map(|row| row.outcome.clone()).collect::<Vec<_>>(),
            "checkpoint_audit_document_counts": audit.iter().map(|row| row.observed_forum_documents).collect::<Vec<_>>(),
            "final_checkpoint_revision": checkpoint.owner_revision,
            "final_checkpoint_event_id": checkpoint.event_id,
            "recovered_search_documents": documents,
            "owner_state_unchanged_after_recovery": true,
            "synthetic_inbox_deliveries_created": false,
            "caught_up_repeat_rebuilds": caught_up.owner_rebuilds,
            "caught_up_repeat_checkpoint_advances": caught_up.owner_revisions_checkpointed
        }),
    })
}

async fn create_forum_fixture(db: &DatabaseConnection) -> TestResult<ForumFixture> {
    let tenant_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO users (id, tenant_id) VALUES ($1, $2)",
        vec![admin_id.into(), tenant_id.into()],
    ))
    .await?;

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin_security(admin_id),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "D9 Search-disabled category".to_string(),
                slug: "d9-search-disabled-category".to_string(),
                description: Some("Forum owner state committed without Search".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let bus = event_bus(db.clone());
    let topics = TopicService::new(db.clone(), bus);
    let topic_one = topics
        .create(
            tenant_id,
            customer_security(),
            topic_input(
                category.id,
                &format!("D9 topic one {TOPIC_ONE_MARKER}"),
                "d9-search-disabled-topic-one",
            ),
        )
        .await?;
    let topic_two = topics
        .create(
            tenant_id,
            customer_security(),
            topic_input(
                category.id,
                &format!("D9 topic two {TOPIC_TWO_MARKER}"),
                "d9-search-disabled-topic-two",
            ),
        )
        .await?;

    Ok(ForumFixture {
        tenant_id,
        category_id: category.id,
        topic_one_id: topic_one.id,
        topic_two_id: topic_two.id,
    })
}

fn topic_input(category_id: Uuid, title: &str, slug: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".to_string(),
        category_id,
        title: title.to_string(),
        slug: Some(slug.to_string()),
        body: rustok_api::RichTextDocument::single_paragraph(format!(
            "{title} body committed while Search is disabled"
        )),
        metadata: json!({}),
        tags: Vec::new(),
        channel_slugs: None,
    }
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn admin_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(user_id))
}

fn customer_security() -> SecurityContext {
    SecurityContext::new(UserRole::Customer, None)
}

fn assert_disabled_owner_shape(
    snapshot: &ForumOwnerSnapshot,
    fixture: ForumFixture,
) -> TestResult<()> {
    if snapshot.category_id != fixture.category_id
        || snapshot.category_name != "D9 Search-disabled category"
        || snapshot.category_slug != "d9-search-disabled-category"
        || snapshot.category_description != "Forum owner state committed without Search"
        || snapshot.topics.len() != 2
        || snapshot.revisions.len() != 3
    {
        return Err(test_error(format!(
            "Search-disabled Forum owner snapshot has an unexpected shape: {snapshot:?}"
        )));
    }
    if snapshot
        .revisions
        .iter()
        .map(|row| row.revision)
        .collect::<Vec<_>>()
        != [1, 2, 3]
        || snapshot.revisions[0].target_type != "forum"
        || snapshot.revisions[0].target_id.is_some()
        || snapshot.revisions[1].target_type != "forum_category"
        || snapshot.revisions[1].target_id != Some(fixture.category_id)
        || snapshot.revisions[2].target_type != "forum_category"
        || snapshot.revisions[2].target_id != Some(fixture.category_id)
    {
        return Err(test_error(format!(
            "Search-disabled Forum revisions are not the exact contiguous owner sequence: {:?}",
            snapshot.revisions
        )));
    }
    let topic_ids = snapshot
        .topics
        .iter()
        .map(|topic| topic.id)
        .collect::<BTreeSet<_>>();
    if topic_ids != BTreeSet::from([fixture.topic_one_id, fixture.topic_two_id])
        || snapshot
            .topics
            .iter()
            .any(|topic| topic.category_id != fixture.category_id || topic.status != "open")
    {
        return Err(test_error(format!(
            "Search-disabled Forum topics did not commit exactly: {:?}",
            snapshot.topics
        )));
    }
    Ok(())
}

fn assert_recovered_documents(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 3 {
        return Err(test_error(format!(
            "late Search rebuild produced {} Forum documents instead of three: {documents:?}",
            documents.len()
        )));
    }
    let category = documents
        .iter()
        .find(|document| document.document_id == fixture.category_id)
        .ok_or_else(|| test_error("late Search rebuild omitted the Forum category"))?;
    if category.entity_type != "forum_category"
        || category.status != "public"
        || category.title != "D9 Search-disabled category"
    {
        return Err(test_error(format!(
            "late Search rebuild projected an unexpected category: {category:?}"
        )));
    }
    for (topic_id, marker) in [
        (fixture.topic_one_id, TOPIC_ONE_MARKER),
        (fixture.topic_two_id, TOPIC_TWO_MARKER),
    ] {
        let topic = documents
            .iter()
            .find(|document| document.document_id == topic_id)
            .ok_or_else(|| test_error(format!("late Search rebuild omitted topic {topic_id}")))?;
        if topic.entity_type != "forum_topic"
            || topic.status != "open"
            || !topic.title.contains(marker)
            || !topic.body.contains("committed while Search is disabled")
        {
            return Err(test_error(format!(
                "late Search rebuild projected an unexpected topic: {topic:?}"
            )));
        }
    }
    Ok(())
}

async fn load_owner_snapshot(
    db: &DatabaseConnection,
    fixture: ForumFixture,
) -> TestResult<ForumOwnerSnapshot> {
    let category_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT translation.name, translation.slug,
                   COALESCE(translation.description, '') AS description
            FROM forum_categories AS category
            JOIN forum_category_translations AS translation
              ON translation.category_id = category.id
             AND translation.tenant_id = category.tenant_id
            WHERE category.tenant_id = $1
              AND category.id = $2
              AND translation.locale = 'en'
            "#,
            vec![fixture.tenant_id.into(), fixture.category_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error("Forum category disappeared from owner storage"))?;

    let topics = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT topic.id, topic.category_id, topic.status,
                   translation.title, COALESCE(translation.slug, '') AS slug,
                   translation.body
            FROM forum_topics AS topic
            JOIN forum_topic_translations AS translation
              ON translation.topic_id = topic.id
             AND translation.tenant_id = topic.tenant_id
            WHERE topic.tenant_id = $1
              AND translation.locale = 'en'
            ORDER BY topic.id ASC
            "#,
            vec![fixture.tenant_id.into()],
        ))
        .await?
        .into_iter()
        .map(|row| {
            Ok(TopicOwnerRow {
                id: row.try_get("", "id")?,
                category_id: row.try_get("", "category_id")?,
                status: row.try_get("", "status")?,
                title: row.try_get("", "title")?,
                slug: row.try_get("", "slug")?,
                body: row.try_get("", "body")?,
            })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;

    Ok(ForumOwnerSnapshot {
        category_id: fixture.category_id,
        category_name: category_row.try_get("", "name")?,
        category_slug: category_row.try_get("", "slug")?,
        category_description: category_row.try_get("", "description")?,
        topics,
        revisions: load_owner_revisions(db, fixture.tenant_id).await?,
    })
}

async fn load_owner_revisions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<OwnerRevisionRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT revision, event_id, target_type, target_id
        FROM forum_projection_revision_ledger
        WHERE tenant_id = $1
        ORDER BY revision ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(OwnerRevisionRow {
            revision: row.try_get("", "revision")?,
            event_id: row.try_get("", "event_id")?,
            target_type: row.try_get("", "target_type")?,
            target_id: row.try_get("", "target_id")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

async fn load_root_event_ids(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<BTreeSet<Uuid>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![ROOT_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id == tenant_id {
            ids.insert(envelope.id);
        }
    }
    Ok(ids)
}

async fn load_typed_causation_ids(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<BTreeSet<Uuid>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![TYPED_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: ContractEventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id() == tenant_id {
            let causation_id = envelope
                .causation_id()
                .ok_or_else(|| test_error("typed Forum invalidation lost causation identity"))?;
            ids.insert(causation_id);
        }
    }
    Ok(ids)
}

async fn assert_search_storage_absent(db: &DatabaseConnection) -> TestResult<()> {
    for table in [
        "search_projection_inbox",
        "search_projection_owner_checkpoints",
        "search_documents",
    ] {
        if table_exists(db, table).await? {
            return Err(test_error(format!(
                "Search-disabled profile unexpectedly contains `{table}`"
            )));
        }
    }
    Ok(())
}

async fn assert_search_storage_present(db: &DatabaseConnection) -> TestResult<()> {
    for table in [
        "search_projection_inbox",
        "search_projection_owner_checkpoints",
        "search_documents",
    ] {
        if !table_exists(db, table).await? {
            return Err(test_error(format!(
                "late Search enable did not create `{table}`"
            )));
        }
    }
    Ok(())
}

async fn table_exists(db: &DatabaseConnection, table: &str) -> TestResult<bool> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1::text)::TEXT AS value",
            vec![table.to_string().into()],
        ))
        .await?
        .ok_or_else(|| test_error("to_regclass returned no row"))?;
    let value: Option<String> = row.try_get("", "value")?;
    Ok(value.is_some())
}

async fn count_rows(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    if !matches!(
        table,
        "search_projection_inbox" | "search_projection_owner_checkpoints"
    ) {
        return Err(test_error("count_rows received an unsupported table"));
    }
    scalar_i64(
        db,
        Statement::from_string(
            DbBackend::Postgres,
            format!("SELECT COUNT(*)::BIGINT AS value FROM {table}"),
        ),
    )
    .await
}

async fn count_forum_documents(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum'",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn load_forum_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<SearchDocumentRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT document_id, entity_type, status, title, body
        FROM search_documents
        WHERE tenant_id = $1 AND source_module = 'forum'
        ORDER BY entity_type ASC, document_id ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(SearchDocumentRow {
            document_id: row.try_get("", "document_id")?,
            entity_type: row.try_get("", "entity_type")?,
            status: row.try_get("", "status")?,
            title: row.try_get("", "title")?,
            body: row.try_get("", "body")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

async fn create_checkpoint_audit(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE forum_search_disabled_checkpoint_audit (
            sequence BIGSERIAL PRIMARY KEY,
            tenant_id UUID NOT NULL,
            owner_revision BIGINT NOT NULL,
            event_id UUID NOT NULL,
            outcome VARCHAR(32) NOT NULL,
            observed_forum_documents BIGINT NOT NULL
        );

        CREATE OR REPLACE FUNCTION forum_capture_search_disabled_checkpoint()
        RETURNS trigger AS $$
        BEGIN
            INSERT INTO forum_search_disabled_checkpoint_audit (
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

        CREATE TRIGGER forum_search_disabled_checkpoint_audit
        AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints
        FOR EACH ROW EXECUTE FUNCTION forum_capture_search_disabled_checkpoint();
        "#,
    )
    .await?;
    Ok(())
}

async fn load_checkpoint(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Option<CheckpointSnapshot>> {
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
    row.map(
        |row| -> std::result::Result<CheckpointSnapshot, sea_orm::DbErr> {
            Ok(CheckpointSnapshot {
                owner_revision: row.try_get("", "owner_revision")?,
                event_id: row.try_get("", "event_id")?,
                outcome: row.try_get("", "outcome")?,
            })
        },
    )
    .transpose()
    .map_err(Into::into)
}

async fn load_checkpoint_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<CheckpointAuditRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT sequence, owner_revision, event_id, outcome, observed_forum_documents
        FROM forum_search_disabled_checkpoint_audit
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
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row = db
        .query_one(statement)
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn postgres_database_url() -> Option<String> {
    env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| env::var(FORUM_TEST_DATABASE_ENV))
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect_in_schema(
    database_url: &str,
    schema_name: &str,
    max_connections: u32,
) -> TestResult<DatabaseConnection> {
    connect(
        &database_url_in_schema(database_url, schema_name),
        max_connections,
    )
    .await
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
        .ok_or_else(|| test_error("evidence path has no parent directory"))?;
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
        return Err(test_error(
            "git rev-parse HEAD failed for evidence generation",
        ));
    }
    let value = String::from_utf8(output.stdout)?;
    let value = value.trim();
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(test_error(
            "git rev-parse HEAD returned an invalid commit SHA",
        ));
    }
    Ok(value.to_string())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(IoError::new(ErrorKind::InvalidData, message.into()))
}
