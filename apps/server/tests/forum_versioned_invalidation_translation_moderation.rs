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
    ForumSearchResultEligibilityService, ModerationService, ReplyService, TopicService,
    UpdateCategoryInput, UpdateTopicInput,
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
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    Value as SqlValue,
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
const ENGLISH_TOPIC_MARKER: &str = "d14englishtopicmarker";
const FRENCH_CATEGORY_MARKER: &str = "d14frenchcategorymarker";
const FRENCH_TOPIC_MARKER: &str = "d14frenchtopicmarker";
const APPROVED_REPLY_MARKER: &str = "d14approvedreplymarker";
const EVIDENCE_CONTRACT: &str = "forum_search_link_forum_03_translation_moderation_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-link-forum-03-translation-moderation-evidence.json";

struct PostgresTranslationModerationEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresTranslationModerationEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum translation/moderation proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_translation_moderation_{}_{}",
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
    category_id: Uuid,
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
struct InboxOrderRow {
    ingest_sequence: i64,
    event_id: Uuid,
    scope_key: String,
    event_type: String,
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

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct TranslationModerationEvidenceArtifact {
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
                    "forum.search_projection.translation_moderation_owner_unavailable",
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
async fn translation_and_moderation_approval_reach_storefront_search() -> TestResult<()> {
    let Some(evidence) = PostgresTranslationModerationEvidence::setup("link").await? else {
        return Ok(());
    };

    let proof = run_translation_moderation_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(TranslationModerationEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D14",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_translation_moderation_proof(db: &DatabaseConnection) -> TestResult<ScenarioEvidence> {
    let fixture = create_forum_fixture(db).await?;
    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let reconciler = ForumProjectionReconciler::new(db.clone(), projection_source);

    let baseline_revisions = load_owner_revisions_after(db, fixture.tenant_id, 0).await?;
    ensure_revision_shape(
        &baseline_revisions,
        1,
        &[
            ("forum", None),
            ("forum_category", Some(fixture.category_id)),
        ],
        "baseline",
    )?;
    let baseline_deliveries =
        ingest_exact_typed_revisions(db, fixture, &baseline_revisions).await?;
    let baseline_report = reconciler.sweep_due(1, 16).await?;
    if baseline_report.claimed_events != 2
        || baseline_report.completed_events != 2
        || baseline_report.failed_events != 0
    {
        return Err(test_error(format!(
            "baseline Forum projection did not complete exactly two events: {baseline_report:?}"
        )));
    }
    let baseline_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_baseline_documents(&baseline_documents, fixture)?;
    assert_storefront_exact(
        db,
        fixture,
        "en",
        ENGLISH_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        "en",
        APPROVED_REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        0,
    )
    .await?;

    let category_service = CategoryService::new(db.clone());
    category_service
        .update(
            fixture.tenant_id,
            fixture.category_id,
            admin_security(fixture.admin_id),
            UpdateCategoryInput {
                locale: "fr".to_string(),
                name: Some(format!("Catégorie D14 {FRENCH_CATEGORY_MARKER}")),
                slug: Some("d14-categorie-francaise".to_string()),
                description: Some(format!("Description {FRENCH_CATEGORY_MARKER}")),
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;
    let bus = event_bus(db.clone());
    TopicService::new(db.clone(), bus.clone())
        .update(
            fixture.tenant_id,
            fixture.topic_id,
            admin_security(fixture.admin_id),
            UpdateTopicInput {
                locale: "fr".to_string(),
                title: Some(format!("Sujet D14 {FRENCH_TOPIC_MARKER}")),
                body: Some(RichTextDocument::single_paragraph(format!(
                    "Corps français D14 {FRENCH_TOPIC_MARKER}"
                ))),
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await?;

    let translation_revisions = load_owner_revisions_after(db, fixture.tenant_id, 2).await?;
    ensure_revision_shape(
        &translation_revisions,
        3,
        &[("forum", None), ("forum_topic", Some(fixture.topic_id))],
        "translation",
    )?;
    let translation_deliveries =
        ingest_exact_typed_revisions(db, fixture, &translation_revisions).await?;
    let translation_report = reconciler.sweep_due(1, 16).await?;
    if translation_report.claimed_events != 2
        || translation_report.completed_events != 2
        || translation_report.failed_events != 0
    {
        return Err(test_error(format!(
            "translation projection did not complete exactly two events: {translation_report:?}"
        )));
    }
    let translated_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_translated_documents(&translated_documents, fixture)?;
    assert_storefront_exact(
        db,
        fixture,
        "fr",
        FRENCH_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        "en",
        ENGLISH_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        "en",
        APPROVED_REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        0,
    )
    .await?;

    let before_approval_sequence = max_ingest_sequence(db).await?;
    ModerationService::new(db.clone(), bus)
        .approve_reply(
            fixture.tenant_id,
            fixture.reply_id,
            fixture.topic_id,
            admin_security(fixture.admin_id),
        )
        .await?;
    let approval_revisions = load_owner_revisions_after(db, fixture.tenant_id, 4).await?;
    ensure_revision_shape(
        &approval_revisions,
        5,
        &[("forum_category", Some(fixture.category_id))],
        "approval",
    )?;
    let approval_status_event = load_reply_status_event(
        db,
        fixture.tenant_id,
        fixture.topic_id,
        fixture.reply_id,
        "pending",
        "approved",
    )
    .await?;
    insert_legacy_root(db, &approval_status_event, "forum").await?;
    let approval_deliveries =
        ingest_exact_typed_revisions(db, fixture, &approval_revisions).await?;
    let approval_inbox_order =
        load_inbox_order_after(db, fixture.tenant_id, before_approval_sequence).await?;
    if approval_inbox_order.len() != 2
        || approval_inbox_order[0].event_id != approval_status_event.id
        || approval_inbox_order[1].event_id != approval_revisions[0].event_id
        || approval_inbox_order[0].ingest_sequence >= approval_inbox_order[1].ingest_sequence
    {
        return Err(test_error(format!(
            "approval legacy/typed inbox order drifted: {approval_inbox_order:?}"
        )));
    }
    let approval_report = reconciler.sweep_due(1, 16).await?;
    if approval_report.claimed_events != 2
        || approval_report.completed_events != 2
        || approval_report.failed_events != 0
    {
        return Err(test_error(format!(
            "approval projection did not complete legacy and typed events: {approval_report:?}"
        )));
    }

    let approved_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_approved_documents(&approved_documents, fixture)?;
    assert_storefront_exact(
        db,
        fixture,
        "en",
        APPROVED_REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.reply_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        fixture,
        "fr",
        FRENCH_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.topic_id,
        1,
    )
    .await?;

    let all_revisions = load_owner_revisions_after(db, fixture.tenant_id, 0).await?;
    if all_revisions.len() != 5
        || all_revisions
            .windows(2)
            .any(|pair| pair[1].revision != pair[0].revision + 1)
    {
        return Err(test_error(format!(
            "D14 owner revisions are not exactly contiguous 1 through 5: {all_revisions:?}"
        )));
    }
    let root_ids = load_root_event_ids(db, fixture.tenant_id).await?;
    let revision_ids = all_revisions
        .iter()
        .map(|revision| revision.event_id)
        .collect::<BTreeSet<_>>();
    if root_ids != revision_ids {
        return Err(test_error(format!(
            "D14 ledger/root identities diverged: revisions={revision_ids:?}, roots={root_ids:?}"
        )));
    }
    for revision in &all_revisions {
        if count_inbox_rows(db, revision.event_id).await? != 1 {
            return Err(test_error(format!(
                "D14 root {} did not retain exactly one Search inbox row",
                revision.event_id
            )));
        }
    }
    if count_inbox_rows(db, approval_status_event.id).await? != 1 {
        return Err(test_error(
            "D14 approval status event did not retain exactly one Search inbox row",
        ));
    }

    let caught_up = reconciler.sweep_due(1, 16).await?;
    if caught_up.claimed_events != 0
        || caught_up.completed_events != 0
        || caught_up.failed_events != 0
    {
        return Err(test_error(format!(
            "caught-up D14 repeat performed duplicate work: {caught_up:?}"
        )));
    }

    Ok(ScenarioEvidence {
        id: "translation_and_moderation_approval",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "category_id": fixture.category_id,
            "topic_id": fixture.topic_id,
            "reply_id": fixture.reply_id,
            "owner_revision_rows": all_revisions,
            "baseline_deliveries": baseline_deliveries,
            "translation_deliveries": translation_deliveries,
            "approval_deliveries": approval_deliveries,
            "approval_status_root_event_id": approval_status_event.id,
            "approval_inbox_order": approval_inbox_order,
            "baseline_documents": baseline_documents,
            "translated_documents": translated_documents,
            "approved_documents": approved_documents,
            "english_topic_remained_visible": true,
            "french_topic_became_visible": true,
            "pending_reply_visible_before_approval": false,
            "approved_reply_visible_after_approval": true,
            "owner_revision_compared_to_ingest_sequence": false,
            "caught_up_repeat_performed_work": false,
            "topic_move_executed": false,
            "topic_move_blocked_on": "FORUM-21"
        }),
    })
}

async fn create_forum_fixture(db: &DatabaseConnection) -> TestResult<ForumFixture> {
    let tenant_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO users (id, tenant_id) VALUES ($1, $2)",
        vec![admin_id.into(), tenant_id.into()],
    ))
    .await?;

    let admin = admin_security(admin_id);
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "D14 moderated category".to_string(),
                slug: "d14-moderated-category".to_string(),
                description: Some("D14 translation moderation evidence".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: true,
            },
        )
        .await?;
    let bus = event_bus(db.clone());
    let topic = TopicService::new(db.clone(), bus.clone())
        .create(
            tenant_id,
            customer_security(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: format!("D14 English topic {ENGLISH_TOPIC_MARKER}"),
                slug: Some("d14-english-topic".to_string()),
                body: RichTextDocument::single_paragraph(format!(
                    "D14 English body {ENGLISH_TOPIC_MARKER}"
                )),
                metadata: json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?;
    let reply = ReplyService::new(db.clone(), bus)
        .create(
            tenant_id,
            customer_security(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph(format!(
                    "D14 pending reply {APPROVED_REPLY_MARKER}"
                )),
                parent_reply_id: None,
            },
        )
        .await?;
    if reply.status != "pending" {
        return Err(test_error(format!(
            "D14 moderated category produced reply status `{}` instead of pending",
            reply.status
        )));
    }

    Ok(ForumFixture {
        tenant_id,
        category_id: category.id,
        topic_id: topic.id,
        reply_id: reply.id,
        admin_id,
    })
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
                "D14 root envelope does not match owner revision: root={root:?}, revision={revision:?}"
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
                "D14 typed envelope lost transport/root identity: {typed:?}"
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
                    "D14 typed payload does not match owner revision: {payload:?}"
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
                    "D14 typed ingress returned unexpected outcome: {outcome:?}"
                )));
            }
        }
        let inbox = load_inbox_row(db, revision.event_id).await?;
        if inbox.status != "pending" || inbox.ingest_sequence <= 0 {
            return Err(test_error(format!(
                "D14 typed ingress did not create a pending durable inbox row: {inbox:?}"
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
            "D14 typed inbox sequences did not increase within phase: {facts:?}"
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
            "D14 {phase} expected {} owner revisions, received {revisions:?}",
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
                "D14 {phase} owner revision shape drifted: {revisions:?}"
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
    db.query_all_raw(Statement::from_sql_and_values(
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
        .query_all_raw(Statement::from_sql_and_values(
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
            "expected one D14 root envelope {event_id}, found {}",
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
        .query_all_raw(Statement::from_sql_and_values(
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
            "expected one D14 typed envelope caused by {root_event_id}, found {}",
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
        .query_all_raw(Statement::from_sql_and_values(
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

async fn load_reply_status_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    old_status: &str,
    new_status: &str,
) -> TestResult<EventEnvelope> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = 'forum.reply.status_changed' ORDER BY created_at ASC",
            Vec::new(),
        ))
        .await?;
    let mut matches = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id == tenant_id
            && matches!(
                &envelope.event,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id: actual_reply,
                    topic_id: actual_topic,
                    old_status: actual_old,
                    new_status: actual_new,
                    ..
                } if *actual_reply == reply_id
                    && *actual_topic == topic_id
                    && actual_old == old_status
                    && actual_new == new_status
            )
        {
            matches.push(envelope);
        }
    }
    if matches.len() != 1 {
        return Err(test_error(format!(
            "expected one D14 approval status event, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
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

async fn load_inbox_row(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxOrderRow> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT ingest_sequence, event_id, scope_key, event_type, status
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("D14 Search inbox row {event_id} was not found")))?;
    Ok(InboxOrderRow {
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        event_id: row.try_get("", "event_id")?,
        scope_key: row.try_get("", "scope_key")?,
        event_type: row.try_get("", "event_type")?,
        status: row.try_get("", "status")?,
    })
}

async fn load_inbox_order_after(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    after_sequence: i64,
) -> TestResult<Vec<InboxOrderRow>> {
    db.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT ingest_sequence, event_id, scope_key, event_type, status
        FROM search_projection_inbox
        WHERE tenant_id = $1 AND source_module = 'forum' AND ingest_sequence > $2
        ORDER BY ingest_sequence ASC
        "#,
        vec![tenant_id.into(), after_sequence.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(InboxOrderRow {
            ingest_sequence: row.try_get("", "ingest_sequence")?,
            event_id: row.try_get("", "event_id")?,
            scope_key: row.try_get("", "scope_key")?,
            event_type: row.try_get("", "event_type")?,
            status: row.try_get("", "status")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
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

async fn load_forum_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<SearchDocumentRow>> {
    db.query_all_raw(Statement::from_sql_and_values(
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

fn ensure_baseline_documents(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 2 {
        return Err(test_error(format!(
            "D14 baseline projected {} documents instead of two: {documents:?}",
            documents.len()
        )));
    }
    let category = find_document(documents, fixture.category_id, "forum_category", "en")?;
    let topic = find_document(documents, fixture.topic_id, "forum_topic", "en")?;
    if category.status != "public"
        || topic.status != "open"
        || !topic.title.contains(ENGLISH_TOPIC_MARKER)
        || !topic.body.contains(ENGLISH_TOPIC_MARKER)
        || documents
            .iter()
            .any(|document| document.document_id == fixture.reply_id)
    {
        return Err(test_error(format!(
            "D14 baseline documents drifted: {documents:?}"
        )));
    }
    Ok(())
}

fn ensure_translated_documents(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 4 {
        return Err(test_error(format!(
            "D14 translation projected {} documents instead of four: {documents:?}",
            documents.len()
        )));
    }
    let category_en = find_document(documents, fixture.category_id, "forum_category", "en")?;
    let category_fr = find_document(documents, fixture.category_id, "forum_category", "fr")?;
    let topic_en = find_document(documents, fixture.topic_id, "forum_topic", "en")?;
    let topic_fr = find_document(documents, fixture.topic_id, "forum_topic", "fr")?;
    if category_en.status != "public"
        || !category_fr.title.contains(FRENCH_CATEGORY_MARKER)
        || !category_fr.body.contains(FRENCH_CATEGORY_MARKER)
        || !topic_en.title.contains(ENGLISH_TOPIC_MARKER)
        || !topic_fr.title.contains(FRENCH_TOPIC_MARKER)
        || !topic_fr.body.contains(FRENCH_TOPIC_MARKER)
        || documents
            .iter()
            .any(|document| document.document_id == fixture.reply_id)
    {
        return Err(test_error(format!(
            "D14 translated documents drifted: {documents:?}"
        )));
    }
    Ok(())
}

fn ensure_approved_documents(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 5 {
        return Err(test_error(format!(
            "D14 approval projected {} documents instead of five: {documents:?}",
            documents.len()
        )));
    }
    ensure_translated_documents_without_reply_count(documents, fixture)?;
    let reply = find_document(documents, fixture.reply_id, "forum_reply", "en")?;
    let category_id = reply
        .facets
        .get("category_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| test_error("D14 approved reply facet has no category_id"))?;
    let topic_id = reply
        .facets
        .get("topic_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| test_error("D14 approved reply facet has no topic_id"))?;
    if reply.status != "approved"
        || !reply.body.contains(APPROVED_REPLY_MARKER)
        || category_id != fixture.category_id.to_string()
        || topic_id != fixture.topic_id.to_string()
    {
        return Err(test_error(format!(
            "D14 approved reply document drifted: {reply:?}"
        )));
    }
    Ok(())
}

fn ensure_translated_documents_without_reply_count(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    let category_fr = find_document(documents, fixture.category_id, "forum_category", "fr")?;
    let topic_en = find_document(documents, fixture.topic_id, "forum_topic", "en")?;
    let topic_fr = find_document(documents, fixture.topic_id, "forum_topic", "fr")?;
    if !category_fr.title.contains(FRENCH_CATEGORY_MARKER)
        || !topic_en.title.contains(ENGLISH_TOPIC_MARKER)
        || !topic_fr.title.contains(FRENCH_TOPIC_MARKER)
    {
        return Err(test_error(format!(
            "D14 translated documents were not retained after approval: {documents:?}"
        )));
    }
    Ok(())
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
            "D14 expected one {entity_type}:{document_id}:{locale}, found {}",
            matches.len()
        )));
    }
    Ok(matches[0])
}

async fn assert_storefront_exact(
    db: &DatabaseConnection,
    fixture: ForumFixture,
    locale: &str,
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
            locale: Some(locale.to_string()),
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
            category_ids: vec![fixture.category_id.to_string()],
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
                locale: locale.to_string(),
            }),
            transport: StorefrontSearchTransport::Graphql,
        },
    )
    .await?;
    if execution.result.total != expected_total {
        return Err(test_error(format!(
            "D14 storefront marker `{marker}` in `{locale}` returned total {}, expected {expected_total}",
            execution.result.total
        )));
    }
    if expected_total == 1 {
        if execution.result.items.len() != 1 || execution.result.items[0].id != expected_id {
            return Err(test_error(format!(
                "D14 storefront marker `{marker}` did not return exact owner object {expected_id}: {:?}",
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
            "D14 storefront marker `{marker}` leaked items or visible facets"
        )));
    }
    Ok(())
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

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row = db
        .query_one_raw(statement)
        .await?
        .ok_or_else(|| test_error("D14 scalar query returned no row"))?;
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

fn write_evidence(artifact: TranslationModerationEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| test_error("D14 evidence path has no parent directory"))?;
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
            "git rev-parse HEAD failed for D14 evidence generation",
        ));
    }
    let value = String::from_utf8(output.stdout)?;
    let value = value.trim();
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(test_error(
            "git rev-parse HEAD returned an invalid D14 commit SHA",
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
