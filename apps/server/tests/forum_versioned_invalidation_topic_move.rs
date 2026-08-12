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
use rustok_api::{PortError, RequestContext, RichTextDocument};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    ForumSearchProjectionEvent,
};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ForumSearchProjectionSourceFactory, ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, ForumTopicMoveResult, ForumTopicMoveService,
    MoveForumTopicInput, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_search::{
    ForumProjectionReconciler, ForumSearchContractIngress, ForumSearchContractIngressOutcome,
    ForumStorefrontSearchAttributeFilter, ForumStorefrontSearchRequest, SearchModule,
    SearchProjectionSourceFactory, SharedStorefrontSearchCategoryScopePort,
    SharedStorefrontSearchResultEligibilityPort, StorefrontSearchCategoryScopePort,
    StorefrontSearchCategoryScopeRequest, StorefrontSearchResultCandidate,
    StorefrontSearchResultCandidateKind, StorefrontSearchResultEligibilityPort,
    StorefrontSearchResultEligibilityRequest, StorefrontSearchTransport,
    execute_forum_storefront_search,
};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
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
const TOPIC_MARKER: &str = "d16topicmovemarker";
const REPLY_MARKER: &str = "d16replymovemarker";
const MOVE_REASON: &str = "Move the discussion to its canonical category";
const EVIDENCE_CONTRACT: &str = "forum_search_link_forum_03_topic_move_evidence_v1";
const EVIDENCE_PATH: &str = "target/forum-search-link-forum-03-topic-move-evidence.json";

struct PostgresTopicMoveEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresTopicMoveEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum topic-move proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_topic_move_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect_in_schema(&database_url, &schema_name, 10).await?;
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

#[derive(Clone, Copy, Debug)]
struct ForumFixture {
    tenant_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    admin_id: Uuid,
}

#[derive(Clone, Debug, Serialize)]
struct OwnerRevisionRow {
    revision: i64,
    event_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
struct DeliveryIdentityFact {
    owner_revision: i64,
    root_event_id: Uuid,
    typed_envelope_id: Uuid,
    ingest_sequence: i64,
    scope_key: String,
}

#[derive(Clone, Debug, Serialize)]
struct InboxRow {
    ingest_sequence: i64,
    event_id: Uuid,
    scope_key: String,
    status: String,
}

#[derive(Clone, Debug, Serialize)]
struct SearchDocumentRow {
    document_id: Uuid,
    entity_type: String,
    locale: String,
    status: String,
    title: String,
    body: String,
    facets: JsonValue,
    payload: JsonValue,
}

#[derive(Clone, Debug, Serialize)]
struct MoveOwnerFact {
    operation_id: Uuid,
    topic_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    actor_id: Uuid,
    reason: String,
    published_reply_count: i32,
    event_id: Uuid,
    event_payload: JsonValue,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct TopicMoveEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_used: bool,
    scenario_results: Vec<ScenarioEvidence>,
}

#[derive(Clone)]
struct ExactCategoryScopePort;

#[async_trait]
impl StorefrontSearchCategoryScopePort for ExactCategoryScopePort {
    async fn expand_forum_category_scope(
        &self,
        request: StorefrontSearchCategoryScopeRequest,
    ) -> Result<Vec<Uuid>, PortError> {
        Ok(request.category_ids)
    }
}

#[derive(Clone)]
struct RealForumPublicEligibilityPort {
    db: DatabaseConnection,
}

#[async_trait]
impl StorefrontSearchResultEligibilityPort for RealForumPublicEligibilityPort {
    async fn filter_forum_result_candidates(
        &self,
        request: StorefrontSearchResultEligibilityRequest,
    ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError> {
        let candidates = request
            .candidates
            .iter()
            .copied()
            .map(to_forum_candidate)
            .collect::<Vec<_>>();
        let channel_slug = request
            .request_context
            .as_ref()
            .and_then(|context| context.channel_slug.as_deref());
        let allowed = ForumSearchResultEligibilityService::new(self.db.clone())
            .filter_public_storefront_visible(request.tenant_id, channel_slug, &candidates)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "forum.search_projection.topic_move_owner_unavailable",
                    "Forum Search result eligibility is temporarily unavailable",
                )
            })?;
        Ok(allowed.into_iter().map(from_forum_candidate).collect())
    }
}

fn to_forum_candidate(candidate: StorefrontSearchResultCandidate) -> ForumSearchResultCandidate {
    ForumSearchResultCandidate {
        document_id: candidate.document_id,
        kind: match candidate.kind {
            StorefrontSearchResultCandidateKind::ForumTopic => {
                ForumSearchResultCandidateKind::Topic
            }
            StorefrontSearchResultCandidateKind::ForumReply => {
                ForumSearchResultCandidateKind::Reply
            }
        },
    }
}

fn from_forum_candidate(candidate: ForumSearchResultCandidate) -> StorefrontSearchResultCandidate {
    StorefrontSearchResultCandidate {
        document_id: candidate.document_id,
        kind: match candidate.kind {
            ForumSearchResultCandidateKind::Topic => {
                StorefrontSearchResultCandidateKind::ForumTopic
            }
            ForumSearchResultCandidateKind::Reply => {
                StorefrontSearchResultCandidateKind::ForumReply
            }
        },
    }
}

#[tokio::test]
async fn topic_move_reassigns_search_category_scope_without_identity_drift() -> TestResult<()> {
    let Some(evidence) = PostgresTopicMoveEvidence::setup("link").await? else {
        return Ok(());
    };

    let proof = run_topic_move_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(TopicMoveEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D16",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_topic_move_proof(db: &DatabaseConnection) -> TestResult<ScenarioEvidence> {
    let fixture = create_forum_fixture(db).await?;
    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let reconciler = ForumProjectionReconciler::new(db.clone(), projection_source);

    let baseline_revisions = load_owner_revisions_after(db, fixture.tenant_id, 0).await?;
    ensure_revision_shape(
        &baseline_revisions,
        1,
        &[
            ("forum", None),
            ("forum", None),
            ("forum_category", Some(fixture.source_category_id)),
            ("forum_category", Some(fixture.source_category_id)),
        ],
        "baseline",
    )?;
    let baseline_deliveries =
        ingest_exact_typed_revisions(db, fixture, &baseline_revisions).await?;
    let baseline_report = reconciler.sweep_due(1, 16).await?;
    if baseline_report.claimed_events != 4
        || baseline_report.completed_events != 4
        || baseline_report.failed_events != 0
    {
        return Err(test_error(format!(
            "D16 baseline Forum projection did not complete exactly four events: {baseline_report:?}"
        )));
    }

    let baseline_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_document_scope(
        &baseline_documents,
        fixture,
        fixture.source_category_id,
        1,
        1,
    )?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.source_category_id,
        TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.target_category_id,
        TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        0,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.source_category_id,
        REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.target_category_id,
        REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        0,
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let input = MoveForumTopicInput {
        operation_id,
        target_category_id: fixture.target_category_id,
        reason: MOVE_REASON.to_string(),
    };
    let bus = event_bus(db.clone());
    let move_service = ForumTopicMoveService::new(db.clone(), bus);
    let moved = move_service
        .move_topic(
            fixture.tenant_id,
            fixture.topic_id,
            admin_security(fixture.admin_id),
            input.clone(),
        )
        .await?;
    ensure_move_result(moved.clone(), fixture, operation_id)?;
    let move_owner_fact = load_move_owner_fact(db, fixture, operation_id).await?;

    let move_revisions = load_owner_revisions_after(db, fixture.tenant_id, 4).await?;
    ensure_revision_shape(
        &move_revisions,
        5,
        &[
            ("forum_topic", Some(fixture.topic_id)),
            ("forum_category", Some(fixture.source_category_id)),
            ("forum_category", Some(fixture.target_category_id)),
        ],
        "move",
    )?;
    let move_deliveries = ingest_exact_typed_revisions(db, fixture, &move_revisions).await?;
    let move_report = reconciler.sweep_due(1, 16).await?;
    if move_report.claimed_events != 3
        || move_report.completed_events != 3
        || move_report.failed_events != 0
    {
        return Err(test_error(format!(
            "D16 move projection did not complete exactly three events: {move_report:?}"
        )));
    }

    let moved_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_document_scope(&moved_documents, fixture, fixture.target_category_id, 0, 0)?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.source_category_id,
        TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        0,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.target_category_id,
        TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.source_category_id,
        REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        0,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        fixture.target_category_id,
        REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        1,
    )
    .await?;

    let roots_before_replay = load_root_event_ids(db, fixture.tenant_id).await?;
    let typed_before_replay = load_typed_event_ids(db, fixture.tenant_id).await?;
    let max_ingest_before_replay = max_ingest_sequence(db).await?;
    let replay = move_service
        .move_topic(
            fixture.tenant_id,
            fixture.topic_id,
            admin_security(fixture.admin_id),
            input,
        )
        .await?;
    if replay != moved {
        return Err(test_error(format!(
            "D16 exact replay did not return the original move result: first={moved:?}, replay={replay:?}"
        )));
    }
    if !load_owner_revisions_after(db, fixture.tenant_id, 7)
        .await?
        .is_empty()
        || load_root_event_ids(db, fixture.tenant_id).await? != roots_before_replay
        || load_typed_event_ids(db, fixture.tenant_id).await? != typed_before_replay
        || max_ingest_sequence(db).await? != max_ingest_before_replay
        || count_move_receipts(db, fixture.tenant_id, operation_id).await? != 1
        || count_move_semantic_events(db, fixture.tenant_id, operation_id).await? != 1
    {
        return Err(test_error(
            "D16 exact replay created duplicate owner, transport, inbox or semantic state",
        ));
    }

    let all_revisions = load_owner_revisions_after(db, fixture.tenant_id, 0).await?;
    if all_revisions.len() != 7
        || all_revisions
            .windows(2)
            .any(|pair| pair[1].revision != pair[0].revision + 1)
    {
        return Err(test_error(format!(
            "D16 owner revisions are not exactly contiguous 1 through 7: {all_revisions:?}"
        )));
    }
    let revision_ids = all_revisions
        .iter()
        .map(|revision| revision.event_id)
        .collect::<BTreeSet<_>>();
    if roots_before_replay != revision_ids {
        return Err(test_error(format!(
            "D16 ledger/root identities diverged: revisions={revision_ids:?}, roots={roots_before_replay:?}"
        )));
    }
    for revision in &all_revisions {
        if count_inbox_rows(db, revision.event_id).await? != 1 {
            return Err(test_error(format!(
                "D16 root {} did not retain exactly one Search inbox row",
                revision.event_id
            )));
        }
    }

    let caught_up = reconciler.sweep_due(1, 16).await?;
    if caught_up.claimed_events != 0
        || caught_up.completed_events != 0
        || caught_up.failed_events != 0
    {
        return Err(test_error(format!(
            "caught-up D16 repeat performed duplicate work: {caught_up:?}"
        )));
    }

    Ok(ScenarioEvidence {
        id: "topic_move_category_scope",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "source_category_id": fixture.source_category_id,
            "target_category_id": fixture.target_category_id,
            "topic_id": fixture.topic_id,
            "reply_id": fixture.reply_id,
            "move_owner_fact": move_owner_fact,
            "owner_revision_rows": all_revisions,
            "baseline_deliveries": baseline_deliveries,
            "move_deliveries": move_deliveries,
            "baseline_documents": baseline_documents,
            "moved_documents": moved_documents,
            "topic_identity_retained": true,
            "reply_identity_retained": true,
            "source_category_scope_empty_after_move": true,
            "target_category_scope_contains_topic_and_reply_after_move": true,
            "exact_replay_created_new_owner_revision": false,
            "exact_replay_created_new_transport_event": false,
            "exact_replay_created_new_inbox_row": false,
            "owner_revision_compared_to_ingest_sequence": false,
            "caught_up_repeat_performed_work": false
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

    let admin = admin_security(admin_id);
    let source = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "D16 source category".to_string(),
                slug: "d16-source-category".to_string(),
                description: Some("D16 topic move source".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;
    let target = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "D16 target category".to_string(),
                slug: "d16-target-category".to_string(),
                description: Some("D16 topic move target".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(1),
                moderated: false,
            },
        )
        .await?;
    let bus = event_bus(db.clone());
    let topic = TopicService::new(db.clone(), bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: source.id,
                title: format!("D16 topic {TOPIC_MARKER}"),
                slug: Some("d16-topic-move".to_string()),
                body: RichTextDocument::single_paragraph(format!("D16 topic body {TOPIC_MARKER}")),
                metadata: json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?;
    let reply = ReplyService::new(db.clone(), bus)
        .create(
            tenant_id,
            admin,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph(format!(
                    "D16 approved reply {REPLY_MARKER}"
                )),
                parent_reply_id: None,
            },
        )
        .await?;
    if reply.status != "approved" {
        return Err(test_error(format!(
            "D16 unmoderated category produced reply status `{}` instead of approved",
            reply.status
        )));
    }

    Ok(ForumFixture {
        tenant_id,
        source_category_id: source.id,
        target_category_id: target.id,
        topic_id: topic.id,
        reply_id: reply.id,
        admin_id,
    })
}

fn ensure_move_result(
    moved: ForumTopicMoveResult,
    fixture: ForumFixture,
    operation_id: Uuid,
) -> TestResult<()> {
    if moved.operation_id != operation_id
        || moved.event_id != operation_id
        || moved.topic_id != fixture.topic_id
        || moved.source_category_id != fixture.source_category_id
        || moved.target_category_id != fixture.target_category_id
        || moved.actor_id != fixture.admin_id
        || moved.reason != MOVE_REASON
        || moved.published_reply_count != 1
    {
        return Err(test_error(format!(
            "D16 owner move result drifted: {moved:?}"
        )));
    }
    Ok(())
}

async fn load_move_owner_fact(
    db: &DatabaseConnection,
    fixture: ForumFixture,
    operation_id: Uuid,
) -> TestResult<MoveOwnerFact> {
    let receipt = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT operation_id, topic_id, source_category_id, target_category_id,
                   actor_id, reason, published_reply_count, event_id
            FROM forum_topic_move_operations
            WHERE tenant_id = $1 AND operation_id = $2
            "#,
            vec![fixture.tenant_id.into(), operation_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error("D16 topic move receipt was not found"))?;
    let journal = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT event_id, aggregate_type, aggregate_id, event_type,
                   schema_version, actor_id, payload
            FROM forum_domain_events
            WHERE tenant_id = $1 AND event_id = $2
            "#,
            vec![fixture.tenant_id.into(), operation_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error("D16 topic move semantic event was not found"))?;

    let event_id: Uuid = journal.try_get("", "event_id")?;
    let aggregate_type: String = journal.try_get("", "aggregate_type")?;
    let aggregate_id: Uuid = journal.try_get("", "aggregate_id")?;
    let event_type: String = journal.try_get("", "event_type")?;
    let schema_version: i16 = journal.try_get("", "schema_version")?;
    let event_actor_id: Option<Uuid> = journal.try_get("", "actor_id")?;
    let event_payload: JsonValue = journal.try_get("", "payload")?;
    if event_id != operation_id
        || aggregate_type != "forum_topic"
        || aggregate_id != fixture.topic_id
        || event_type != "forum.topic.moved"
        || schema_version != 1
        || event_actor_id != Some(fixture.admin_id)
        || event_payload["operation_id"] != operation_id.to_string()
        || event_payload["topic_id"] != fixture.topic_id.to_string()
        || event_payload["source_category_id"] != fixture.source_category_id.to_string()
        || event_payload["target_category_id"] != fixture.target_category_id.to_string()
        || event_payload["published_reply_count"] != 1
        || event_payload["reason"] != MOVE_REASON
    {
        return Err(test_error(format!(
            "D16 topic move semantic event drifted: {event_payload:?}"
        )));
    }

    let fact = MoveOwnerFact {
        operation_id: receipt.try_get("", "operation_id")?,
        topic_id: receipt.try_get("", "topic_id")?,
        source_category_id: receipt.try_get("", "source_category_id")?,
        target_category_id: receipt.try_get("", "target_category_id")?,
        actor_id: receipt.try_get("", "actor_id")?,
        reason: receipt.try_get("", "reason")?,
        published_reply_count: receipt.try_get("", "published_reply_count")?,
        event_id: receipt.try_get("", "event_id")?,
        event_payload,
    };
    if fact.operation_id != operation_id
        || fact.event_id != operation_id
        || fact.topic_id != fixture.topic_id
        || fact.source_category_id != fixture.source_category_id
        || fact.target_category_id != fixture.target_category_id
        || fact.actor_id != fixture.admin_id
        || fact.reason != MOVE_REASON
        || fact.published_reply_count != 1
    {
        return Err(test_error(format!(
            "D16 immutable topic move receipt drifted: {fact:?}"
        )));
    }
    Ok(fact)
}

async fn ingest_exact_typed_revisions(
    db: &DatabaseConnection,
    fixture: ForumFixture,
    revisions: &[OwnerRevisionRow],
) -> TestResult<Vec<DeliveryIdentityFact>> {
    let mut facts = Vec::with_capacity(revisions.len());
    for revision in revisions {
        let root = load_root_envelope(db, fixture.tenant_id, revision.event_id).await?;
        root.validate_registered_schema()?;
        if root.causation_id.is_some()
            || !matches!(
                &root.event,
                DomainEvent::ReindexRequested {
                    target_type,
                    target_id,
                } if target_type == &revision.target_type && *target_id == revision.target_id
            )
        {
            return Err(test_error(format!(
                "D16 root envelope does not match owner revision: root={root:?}, revision={revision:?}"
            )));
        }
        let typed = load_typed_envelope(db, fixture.tenant_id, revision.event_id).await?;
        typed.validate_registered_schema()?;
        if typed.id() == revision.event_id
            || typed.causation_id() != Some(revision.event_id)
            || typed.event_type() != TYPED_EVENT_TYPE
            || typed.schema_version() != 1
        {
            return Err(test_error(format!(
                "D16 typed envelope lost transport/root identity: {typed:?}"
            )));
        }
        match typed.payload()? {
            ContractEventPayload::ForumSearchProjection(
                ForumSearchProjectionEvent::InvalidationIssued {
                    owner_revision,
                    target_type,
                    target_id,
                },
            ) if *owner_revision == revision.revision
                && target_type == &revision.target_type
                && *target_id == revision.target_id => {}
            payload => {
                return Err(test_error(format!(
                    "D16 typed payload does not match owner revision: {payload:?}"
                )));
            }
        }
        match ForumSearchContractIngress::new(db.clone())
            .ingest(&typed)
            .await?
        {
            ForumSearchContractIngressOutcome::DurablyAccepted {
                root_event_id,
                owner_revision,
            } if root_event_id == revision.event_id && owner_revision == revision.revision => {}
            outcome => {
                return Err(test_error(format!(
                    "D16 typed ingress returned unexpected outcome: {outcome:?}"
                )));
            }
        }
        let inbox = load_inbox_row(db, revision.event_id).await?;
        if inbox.status != "pending" || inbox.ingest_sequence <= 0 {
            return Err(test_error(format!(
                "D16 typed ingress did not create a pending durable inbox row: {inbox:?}"
            )));
        }
        facts.push(DeliveryIdentityFact {
            owner_revision: revision.revision,
            root_event_id: revision.event_id,
            typed_envelope_id: typed.id(),
            ingest_sequence: inbox.ingest_sequence,
            scope_key: inbox.scope_key,
        });
    }
    if facts
        .windows(2)
        .any(|pair| pair[1].ingest_sequence <= pair[0].ingest_sequence)
    {
        return Err(test_error(format!(
            "D16 typed inbox sequences did not increase within phase: {facts:?}"
        )));
    }
    Ok(facts)
}

fn ensure_revision_shape(
    revisions: &[OwnerRevisionRow],
    first_revision: i64,
    expected: &[(&str, Option<Uuid>)],
    phase: &str,
) -> TestResult<()> {
    if revisions.len() != expected.len() {
        return Err(test_error(format!(
            "D16 {phase} expected {} owner revisions, received {revisions:?}",
            expected.len()
        )));
    }
    for (index, (actual, (target_type, target_id))) in
        revisions.iter().zip(expected.iter()).enumerate()
    {
        if actual.revision != first_revision + index as i64
            || actual.target_type != *target_type
            || actual.target_id != *target_id
        {
            return Err(test_error(format!(
                "D16 {phase} owner revision shape drifted: {revisions:?}"
            )));
        }
    }
    Ok(())
}

async fn load_owner_revisions_after(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    after_revision: i64,
) -> TestResult<Vec<OwnerRevisionRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT revision, event_id, target_type, target_id
        FROM forum_projection_revision_ledger
        WHERE tenant_id = $1 AND revision > $2
        ORDER BY revision ASC
        "#,
        vec![tenant_id.into(), after_revision.into()],
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

async fn load_root_envelope(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event_id: Uuid,
) -> TestResult<EventEnvelope> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![ROOT_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut matches = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id == tenant_id && envelope.id == event_id {
            matches.push(envelope);
        }
    }
    if matches.len() != 1 {
        return Err(test_error(format!(
            "expected one D16 root envelope {event_id}, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
}

async fn load_typed_envelope(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    root_event_id: Uuid,
) -> TestResult<ContractEventEnvelope> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![TYPED_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut matches = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: ContractEventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id() == tenant_id && envelope.causation_id() == Some(root_event_id) {
            matches.push(envelope);
        }
    }
    if matches.len() != 1 {
        return Err(test_error(format!(
            "expected one D16 typed envelope caused by {root_event_id}, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
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

async fn load_typed_event_ids(
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
            ids.insert(envelope.id());
        }
    }
    Ok(ids)
}

async fn load_inbox_row(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxRow> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT ingest_sequence, event_id, scope_key, status
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("D16 Search inbox row {event_id} was not found")))?;
    Ok(InboxRow {
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        event_id: row.try_get("", "event_id")?,
        scope_key: row.try_get("", "scope_key")?,
        status: row.try_get("", "status")?,
    })
}

async fn load_forum_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<SearchDocumentRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT document_id, entity_type, locale, status, title, body, facets, payload
        FROM search_documents
        WHERE tenant_id = $1 AND source_module = 'forum'
        ORDER BY entity_type ASC, document_id ASC, locale ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(SearchDocumentRow {
            document_id: row.try_get("", "document_id")?,
            entity_type: row.try_get("", "entity_type")?,
            locale: row.try_get("", "locale")?,
            status: row.try_get("", "status")?,
            title: row.try_get("", "title")?,
            body: row.try_get("", "body")?,
            facets: row.try_get("", "facets")?,
            payload: row.try_get("", "payload")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

fn ensure_document_scope(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
    expected_category_id: Uuid,
    expected_source_topics: i64,
    expected_source_replies: i64,
) -> TestResult<()> {
    if documents.len() != 4 {
        return Err(test_error(format!(
            "D16 projected {} documents instead of four: {documents:?}",
            documents.len()
        )));
    }
    let source = find_document(
        documents,
        fixture.source_category_id,
        "forum_category",
        "en",
    )?;
    let target = find_document(
        documents,
        fixture.target_category_id,
        "forum_category",
        "en",
    )?;
    let topic = find_document(documents, fixture.topic_id, "forum_topic", "en")?;
    let reply = find_document(documents, fixture.reply_id, "forum_reply", "en")?;

    let expected_target_topics = 1 - expected_source_topics;
    let expected_target_replies = 1 - expected_source_replies;
    if source.payload["topic_count"].as_i64() != Some(expected_source_topics)
        || source.payload["reply_count"].as_i64() != Some(expected_source_replies)
        || target.payload["topic_count"].as_i64() != Some(expected_target_topics)
        || target.payload["reply_count"].as_i64() != Some(expected_target_replies)
        || topic.status != "open"
        || reply.status != "approved"
        || !topic.title.contains(TOPIC_MARKER)
        || !topic.body.contains(TOPIC_MARKER)
        || !reply.body.contains(REPLY_MARKER)
        || document_category_id(topic)? != expected_category_id
        || document_category_id(reply)? != expected_category_id
        || topic.payload["category_id"] != expected_category_id.to_string()
        || reply.payload["category_id"] != expected_category_id.to_string()
        || reply.payload["topic_id"] != fixture.topic_id.to_string()
    {
        return Err(test_error(format!(
            "D16 Search document category scope drifted: {documents:?}"
        )));
    }
    Ok(())
}

fn document_category_id(document: &SearchDocumentRow) -> TestResult<Uuid> {
    document
        .facets
        .get("category_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| test_error("D16 Forum document facet has no category_id"))?
        .parse()
        .map_err(Into::into)
}

fn find_document<'a>(
    documents: &'a [SearchDocumentRow],
    document_id: Uuid,
    entity_type: &str,
    locale: &str,
) -> TestResult<&'a SearchDocumentRow> {
    let matches = documents
        .iter()
        .filter(|document| {
            document.document_id == document_id
                && document.entity_type == entity_type
                && document.locale == locale
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(test_error(format!(
            "D16 expected one {entity_type}:{document_id}:{locale}, found {}",
            matches.len()
        )));
    }
    Ok(matches[0])
}

async fn assert_storefront_exact(
    db: &DatabaseConnection,
    fixture: ForumFixture,
    category_id: Uuid,
    marker: &str,
    entity_type: &str,
    status: &str,
    expected_id: Uuid,
    expected_total: u64,
) -> TestResult<()> {
    let category_scope: SharedStorefrontSearchCategoryScopePort = Arc::new(ExactCategoryScopePort);
    let eligibility: SharedStorefrontSearchResultEligibilityPort =
        Arc::new(RealForumPublicEligibilityPort { db: db.clone() });
    let execution = execute_forum_storefront_search(
        db,
        Some(category_scope),
        Some(eligibility),
        ForumStorefrontSearchRequest {
            tenant_id: fixture.tenant_id,
            query: marker.to_string(),
            locale: Some("en".to_string()),
            fallback_locale: "en".to_string(),
            channel_id: None,
            current_channel_only: None,
            limit: Some(10),
            offset: Some(0),
            ranking_profile: None,
            preset_key: None,
            entity_types: vec![entity_type.to_string()],
            source_modules: vec!["forum".to_string()],
            statuses: vec![status.to_string()],
            category_ids: vec![category_id.to_string()],
            author_ids: Vec::new(),
            tags: Vec::new(),
            solved: None,
            published_from: None,
            published_to: None,
            attribute_filters: Vec::<ForumStorefrontSearchAttributeFilter>::new(),
            sort_attribute_code: None,
            sort_desc: false,
            auth: None,
            request_context: Some(RequestContext {
                tenant_id: fixture.tenant_id,
                user_id: None,
                channel_id: None,
                channel_slug: None,
                channel_resolution_source: None,
                locale: "en".to_string(),
            }),
            transport: StorefrontSearchTransport::Graphql,
        },
    )
    .await?;
    if execution.result.total != expected_total {
        return Err(test_error(format!(
            "D16 storefront marker `{marker}` in category {category_id} returned total {}, expected {expected_total}",
            execution.result.total
        )));
    }
    if expected_total == 1 {
        if execution.result.items.len() != 1 || execution.result.items[0].id != expected_id {
            return Err(test_error(format!(
                "D16 storefront marker `{marker}` did not return exact owner object {expected_id}: {:?}",
                execution.result.items
            )));
        }
    } else if !execution.result.items.is_empty()
        || execution
            .result
            .facets
            .iter()
            .any(|facet| !facet.buckets.is_empty())
    {
        return Err(test_error(format!(
            "D16 storefront marker `{marker}` leaked items or visible facets"
        )));
    }
    Ok(())
}

async fn count_move_receipts(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM forum_topic_move_operations WHERE tenant_id = $1 AND operation_id = $2",
            vec![tenant_id.into(), operation_id.into()],
        ),
    )
    .await
}

async fn count_move_semantic_events(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM forum_domain_events WHERE tenant_id = $1 AND event_id = $2 AND event_type = 'forum.topic.moved'",
            vec![tenant_id.into(), operation_id.into()],
        ),
    )
    .await
}

async fn count_inbox_rows(db: &DatabaseConnection, event_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_projection_inbox WHERE event_id = $1",
            vec![event_id.into()],
        ),
    )
    .await
}

async fn max_ingest_sequence(db: &DatabaseConnection) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_string(
            DbBackend::Postgres,
            "SELECT COALESCE(MAX(ingest_sequence), 0)::BIGINT AS value FROM search_projection_inbox"
                .to_string(),
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row: QueryResult = db
        .query_one(statement)
        .await?
        .ok_or_else(|| test_error("D16 scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn admin_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(user_id))
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

fn write_evidence(artifact: TopicMoveEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| test_error("D16 evidence path has no parent directory"))?;
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
            "git rev-parse HEAD failed for D16 evidence generation",
        ));
    }
    let value = String::from_utf8(output.stdout)?;
    let value = value.trim();
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(test_error(
            "git rev-parse HEAD returned an invalid D16 commit SHA",
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
