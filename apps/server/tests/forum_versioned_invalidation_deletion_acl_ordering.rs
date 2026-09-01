use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortError, RequestContext};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumModule, ForumSearchProjectionSourceFactory,
    ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, ForumTopicAudiencePolicyService, ModerationService,
    ReplyService, SetForumTopicAudiencePolicyInput, TopicService,
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
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json";
const HIDDEN_REPLY_MARKER: &str = "d8hiddenreplymarker";
const DELETED_TOPIC_MARKER: &str = "d8deletedtopicmarker";
const ACL_TOPIC_MARKER: &str = "d8acltopicmarker";

struct PostgresDeletionAclEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresDeletionAclEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum deletion/ACL ordering proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_deletion_acl_{}_{}",
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
    hidden_reply_topic_id: Uuid,
    hidden_reply_id: Uuid,
    deleted_topic_id: Uuid,
    acl_topic_id: Uuid,
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
struct InboxOrderRow {
    ingest_sequence: i64,
    event_id: Uuid,
    scope_key: String,
    event_type: String,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct DeletionAclEvidenceArtifact {
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
    observed: Arc<Mutex<Vec<Vec<StorefrontSearchResultCandidate>>>>,
}

impl RealForumPublicEligibilityPort {
    fn shared(
        db: DatabaseConnection,
        observed: Arc<Mutex<Vec<Vec<StorefrontSearchResultCandidate>>>>,
    ) -> SharedStorefrontSearchResultEligibilityPort {
        Arc::new(Self { db, observed })
    }
}

#[async_trait]
impl StorefrontSearchResultEligibilityPort for RealForumPublicEligibilityPort {
    async fn filter_forum_result_candidates(
        &self,
        request: StorefrontSearchResultEligibilityRequest,
    ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError> {
        self.observed
            .lock()
            .map_err(|_| owner_unavailable())?
            .push(request.candidates.clone());
        let forum_candidates = request
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
            .filter_public_storefront_visible(request.tenant_id, channel_slug, &forum_candidates)
            .await
            .map_err(|_| owner_unavailable())?;
        Ok(allowed.into_iter().map(from_forum_candidate).collect())
    }
}

fn owner_unavailable() -> PortError {
    PortError::unavailable(
        "forum.search_projection.deletion_acl_owner_unavailable",
        "Forum deletion/ACL eligibility owner is temporarily unavailable",
    )
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
async fn deletion_acl_ordering_cannot_restore_denied_storefront_results() -> TestResult<()> {
    let Some(evidence) = PostgresDeletionAclEvidence::setup("ordering").await? else {
        return Ok(());
    };

    let proof = run_deletion_acl_ordering_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(DeletionAclEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D8",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_deletion_acl_ordering_proof(db: &DatabaseConnection) -> TestResult<ScenarioEvidence> {
    let fixture = create_forum_fixture(db).await?;
    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let reconciler = ForumProjectionReconciler::new(db.clone(), projection_source);

    let baseline_approved = load_reply_status_event(
        db,
        fixture.tenant_id,
        fixture.hidden_reply_topic_id,
        fixture.hidden_reply_id,
        "pending",
        "approved",
    )
    .await?;
    insert_legacy_root(db, &baseline_approved, "forum").await?;
    let baseline_report = reconciler.sweep_due(1, 16).await?;
    if baseline_report.claimed_events != 1
        || baseline_report.completed_events != 1
        || baseline_report.failed_events != 0
    {
        return Err(test_error(format!(
            "baseline approved-reply rebuild did not complete exactly once: {baseline_report:?}"
        )));
    }
    let baseline_ingest_sequence = max_ingest_sequence(db).await?;

    let observed = Arc::new(Mutex::new(Vec::new()));
    let category_scope: SharedStorefrontSearchCategoryScopePort = Arc::new(ExactCategoryScopePort);
    let eligibility = RealForumPublicEligibilityPort::shared(db.clone(), observed.clone());
    assert_storefront_exact(
        db,
        category_scope.clone(),
        eligibility.clone(),
        fixture,
        HIDDEN_REPLY_MARKER,
        "forum_reply",
        "approved",
        fixture.hidden_reply_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        category_scope.clone(),
        eligibility.clone(),
        fixture,
        DELETED_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.deleted_topic_id,
        1,
    )
    .await?;
    assert_storefront_exact(
        db,
        category_scope.clone(),
        eligibility.clone(),
        fixture,
        ACL_TOPIC_MARKER,
        "forum_topic",
        "open",
        fixture.acl_topic_id,
        1,
    )
    .await?;

    let baseline_revision = max_owner_revision(db, fixture.tenant_id).await?;
    let bus = event_bus(db.clone());
    ModerationService::new(db.clone(), bus.clone())
        .hide_reply(
            fixture.tenant_id,
            fixture.hidden_reply_id,
            fixture.hidden_reply_topic_id,
            admin_security(fixture.admin_id),
        )
        .await?;
    let after_hide_revision = max_owner_revision(db, fixture.tenant_id).await?;
    TopicService::new(db.clone(), bus)
        .delete(
            fixture.tenant_id,
            fixture.deleted_topic_id,
            admin_security(fixture.admin_id),
        )
        .await?;
    let after_delete_revision = max_owner_revision(db, fixture.tenant_id).await?;
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            fixture.tenant_id,
            fixture.acl_topic_id,
            admin_security(fixture.admin_id),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await?;
    let after_acl_revision = max_owner_revision(db, fixture.tenant_id).await?;

    if after_hide_revision != baseline_revision + 1
        || after_delete_revision != after_hide_revision + 2
        || after_acl_revision != after_delete_revision + 1
    {
        return Err(test_error(format!(
            "owner mutations emitted unexpected revision counts: baseline={baseline_revision}, hide={after_hide_revision}, delete={after_delete_revision}, acl={after_acl_revision}"
        )));
    }

    let revisions = load_owner_revisions_after(db, fixture.tenant_id, baseline_revision).await?;
    ensure_owner_revision_shape(&revisions, fixture)?;

    let hidden_legacy = load_reply_status_event(
        db,
        fixture.tenant_id,
        fixture.hidden_reply_topic_id,
        fixture.hidden_reply_id,
        "approved",
        "hidden",
    )
    .await?;
    let deleted_legacy = load_topic_status_event(
        db,
        fixture.tenant_id,
        fixture.deleted_topic_id,
        "open",
        "archived",
    )
    .await?;

    let mut expected_admission = Vec::new();
    for revision in revisions[1..].iter().rev() {
        ingest_typed_revision(db, fixture.tenant_id, revision).await?;
        expected_admission.push(revision.event_id);
    }
    insert_legacy_root(db, &deleted_legacy, "forum").await?;
    expected_admission.push(deleted_legacy.id);
    insert_legacy_root(db, &hidden_legacy, "forum").await?;
    expected_admission.push(hidden_legacy.id);
    ingest_typed_revision(db, fixture.tenant_id, &revisions[0]).await?;
    expected_admission.push(revisions[0].event_id);

    ingest_typed_revision(db, fixture.tenant_id, &revisions[0]).await?;
    insert_legacy_root(db, &hidden_legacy, "forum").await?;

    let inbox_order = load_inbox_order_after(db, baseline_ingest_sequence).await?;
    let actual_admission = inbox_order
        .iter()
        .map(|row| row.event_id)
        .collect::<Vec<_>>();
    if actual_admission != expected_admission || inbox_order.len() != 6 {
        return Err(test_error(format!(
            "out-of-order durable admission drifted: expected={expected_admission:?}, actual={actual_admission:?}"
        )));
    }
    for event_id in &expected_admission {
        if count_inbox_rows(db, *event_id).await? != 1 {
            return Err(test_error(format!(
                "duplicate root identity {event_id} did not retain exactly one inbox row"
            )));
        }
    }

    let report = reconciler.sweep_due(1, 32).await?;
    if report.claimed_events != 6 || report.completed_events != 6 || report.failed_events != 0 {
        return Err(test_error(format!(
            "out-of-order Forum reconciliation did not complete all unique roots: {report:?}"
        )));
    }

    for document_id in [
        fixture.hidden_reply_id,
        fixture.deleted_topic_id,
        fixture.acl_topic_id,
    ] {
        if count_forum_document(db, fixture.tenant_id, document_id).await? != 0 {
            return Err(test_error(format!(
                "denied owner object {document_id} remained in Search after reconciliation"
            )));
        }
    }

    for (marker, entity_type, status, expected_id) in [
        (
            HIDDEN_REPLY_MARKER,
            "forum_reply",
            "approved",
            fixture.hidden_reply_id,
        ),
        (
            DELETED_TOPIC_MARKER,
            "forum_topic",
            "open",
            fixture.deleted_topic_id,
        ),
        (
            ACL_TOPIC_MARKER,
            "forum_topic",
            "open",
            fixture.acl_topic_id,
        ),
    ] {
        assert_storefront_exact(
            db,
            category_scope.clone(),
            eligibility.clone(),
            fixture,
            marker,
            entity_type,
            status,
            expected_id,
            0,
        )
        .await?;
    }

    insert_stale_search_documents(db, fixture).await?;
    if count_stale_markers(db, fixture.tenant_id).await? != 3 {
        return Err(test_error(
            "stale Search candidate injection did not create exactly three rows",
        ));
    }
    observed
        .lock()
        .map_err(|_| test_error("eligibility observation lock was poisoned"))?
        .clear();

    for (marker, entity_type, status, expected_id) in [
        (
            HIDDEN_REPLY_MARKER,
            "forum_reply",
            "approved",
            fixture.hidden_reply_id,
        ),
        (
            DELETED_TOPIC_MARKER,
            "forum_topic",
            "open",
            fixture.deleted_topic_id,
        ),
        (
            ACL_TOPIC_MARKER,
            "forum_topic",
            "open",
            fixture.acl_topic_id,
        ),
    ] {
        assert_storefront_exact(
            db,
            category_scope.clone(),
            eligibility.clone(),
            fixture,
            marker,
            entity_type,
            status,
            expected_id,
            0,
        )
        .await?;
    }

    let owner_requests = observed
        .lock()
        .map_err(|_| test_error("eligibility observation lock was poisoned"))?
        .clone();
    let expected_candidates = [
        StorefrontSearchResultCandidate {
            document_id: fixture.hidden_reply_id,
            kind: StorefrontSearchResultCandidateKind::ForumReply,
        },
        StorefrontSearchResultCandidate {
            document_id: fixture.deleted_topic_id,
            kind: StorefrontSearchResultCandidateKind::ForumTopic,
        },
        StorefrontSearchResultCandidate {
            document_id: fixture.acl_topic_id,
            kind: StorefrontSearchResultCandidateKind::ForumTopic,
        },
    ];
    if owner_requests.len() != 3
        || owner_requests
            .iter()
            .zip(expected_candidates)
            .any(|(actual, expected)| actual.as_slice() != std::slice::from_ref(&expected))
    {
        return Err(test_error(format!(
            "storefront owner did not reauthorize the exact stale candidates: {owner_requests:?}"
        )));
    }

    Ok(ScenarioEvidence {
        id: "deletion_acl_ordering",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "category_id": fixture.category_id,
            "initially_searchable": {
                "approved_reply_id": fixture.hidden_reply_id,
                "future_deleted_topic_id": fixture.deleted_topic_id,
                "future_acl_denied_topic_id": fixture.acl_topic_id
            },
            "baseline_owner_revision": baseline_revision,
            "owner_revision_rows": revisions,
            "legacy_hide_root_event_id": hidden_legacy.id,
            "legacy_delete_root_event_id": deleted_legacy.id,
            "durable_admission_order": inbox_order,
            "duplicate_root_rows": 1,
            "unique_post_mutation_inbox_rows": 6,
            "projection_absence_after_reconciliation": true,
            "stale_search_rows_injected": 3,
            "storefront_owner_requests": owner_requests,
            "final_visible_totals": {
                "hidden_reply": 0,
                "deleted_topic": 0,
                "acl_denied_topic": 0
            },
            "final_visible_items": 0,
            "final_visible_facet_buckets": 0,
            "denied_content_restored": false
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
                name: "D8 public category".to_string(),
                slug: "d8-public-category".to_string(),
                description: Some("Forum deletion ACL ordering evidence".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: true,
            },
        )
        .await?;
    let bus = event_bus(db.clone());
    let topics = TopicService::new(db.clone(), bus.clone());
    let hidden_reply_topic = topics
        .create(
            tenant_id,
            customer_security(),
            topic_input(category.id, "D8 reply owner topic", "d8-reply-owner-topic"),
        )
        .await?;
    let deleted_topic = topics
        .create(
            tenant_id,
            customer_security(),
            topic_input(
                category.id,
                "D8 deleted topic d8deletedtopicmarker",
                "d8-deleted-topic",
            ),
        )
        .await?;
    let acl_topic = topics
        .create(
            tenant_id,
            customer_security(),
            topic_input(category.id, "D8 ACL topic d8acltopicmarker", "d8-acl-topic"),
        )
        .await?;
    let reply = ReplyService::new(db.clone(), bus.clone())
        .create(
            tenant_id,
            customer_security(),
            hidden_reply_topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(format!(
                    "D8 approved reply {HIDDEN_REPLY_MARKER}"
                )),
                parent_reply_id: None,
            },
        )
        .await?;
    if reply.status != "pending" {
        return Err(test_error(format!(
            "moderated D8 category produced unexpected reply status `{}`",
            reply.status
        )));
    }
    ModerationService::new(db.clone(), bus)
        .approve_reply(tenant_id, reply.id, hidden_reply_topic.id, admin.clone())
        .await?;

    Ok(ForumFixture {
        tenant_id,
        category_id: category.id,
        hidden_reply_topic_id: hidden_reply_topic.id,
        hidden_reply_id: reply.id,
        deleted_topic_id: deleted_topic.id,
        acl_topic_id: acl_topic.id,
        admin_id,
    })
}

fn topic_input(category_id: Uuid, title: &str, slug: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".to_string(),
        category_id,
        title: title.to_string(),
        slug: Some(slug.to_string()),
        body: rustok_api::RichTextDocument::single_paragraph(format!("{title} body")),
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

fn ensure_owner_revision_shape(
    revisions: &[OwnerRevisionRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if revisions.len() != 4 {
        return Err(test_error(format!(
            "D8 expected four owner revisions, received {revisions:?}"
        )));
    }
    for pair in revisions.windows(2) {
        if pair[1].revision != pair[0].revision + 1 {
            return Err(test_error(format!(
                "D8 owner revisions are not contiguous: {revisions:?}"
            )));
        }
    }
    let expected = [
        ("forum_category", Some(fixture.category_id)),
        ("forum_topic", Some(fixture.deleted_topic_id)),
        ("forum_category", Some(fixture.category_id)),
        ("forum_topic", Some(fixture.acl_topic_id)),
    ];
    if revisions
        .iter()
        .zip(expected)
        .any(|(actual, (target_type, target_id))| {
            actual.target_type != target_type || actual.target_id != target_id
        })
    {
        return Err(test_error(format!(
            "D8 owner revision targets drifted: {revisions:?}"
        )));
    }
    Ok(())
}

async fn ingest_typed_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    revision: &OwnerRevisionRow,
) -> TestResult<()> {
    let envelope = ContractEventEnvelope::new_caused_by(
        tenant_id,
        None,
        revision.event_id,
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: revision.revision,
            target_type: revision.target_type.clone(),
            target_id: revision.target_id,
        },
    )?;
    let outcome = ForumSearchContractIngress::new(db.clone())
        .ingest(&envelope)
        .await?;
    match outcome {
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id,
            owner_revision,
        } if root_event_id == revision.event_id && owner_revision == revision.revision => Ok(()),
        other => Err(test_error(format!(
            "typed D8 invalidation returned unexpected ingress outcome: {other:?}"
        ))),
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

async fn max_owner_revision(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(MAX(revision), 0)::BIGINT AS value FROM forum_projection_revision_ledger WHERE tenant_id = $1",
            vec![tenant_id.into()],
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

async fn load_inbox_order_after(
    db: &DatabaseConnection,
    after_sequence: i64,
) -> TestResult<Vec<InboxOrderRow>> {
    db.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT ingest_sequence, event_id, scope_key, event_type
        FROM search_projection_inbox
        WHERE ingest_sequence > $1
        ORDER BY ingest_sequence ASC
        "#,
        vec![after_sequence.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(InboxOrderRow {
            ingest_sequence: row.try_get("", "ingest_sequence")?,
            event_id: row.try_get("", "event_id")?,
            scope_key: row.try_get("", "scope_key")?,
            event_type: row.try_get("", "event_type")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
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

async fn count_forum_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum' AND document_id = $2",
            vec![tenant_id.into(), document_id.into()],
        ),
    )
    .await
}

async fn load_reply_status_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    old_status: &str,
    new_status: &str,
) -> TestResult<EventEnvelope> {
    load_exact_event(db, "forum.reply.status_changed", |envelope| {
        envelope.tenant_id == tenant_id
            && matches!(
                &envelope.event,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id: actual_reply_id,
                    topic_id: actual_topic_id,
                    old_status: actual_old,
                    new_status: actual_new,
                    ..
                } if *actual_reply_id == reply_id
                    && *actual_topic_id == topic_id
                    && actual_old == old_status
                    && actual_new == new_status
            )
    })
    .await
}

async fn load_topic_status_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    old_status: &str,
    new_status: &str,
) -> TestResult<EventEnvelope> {
    load_exact_event(db, "forum.topic.status_changed", |envelope| {
        envelope.tenant_id == tenant_id
            && matches!(
                &envelope.event,
                DomainEvent::ForumTopicStatusChanged {
                    topic_id: actual_topic_id,
                    old_status: actual_old,
                    new_status: actual_new,
                    ..
                } if *actual_topic_id == topic_id
                    && actual_old == old_status
                    && actual_new == new_status
            )
    })
    .await
}

async fn load_exact_event<F>(
    db: &DatabaseConnection,
    event_type: &str,
    predicate: F,
) -> TestResult<EventEnvelope>
where
    F: Fn(&EventEnvelope) -> bool,
{
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![event_type.to_string().into()],
        ))
        .await?;
    let mut matches = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if predicate(&envelope) {
            matches.push(envelope);
        }
    }
    if matches.len() != 1 {
        return Err(test_error(format!(
            "expected exactly one `{event_type}` owner event, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
}

async fn assert_storefront_exact(
    db: &DatabaseConnection,
    category_scope: SharedStorefrontSearchCategoryScopePort,
    eligibility: SharedStorefrontSearchResultEligibilityPort,
    fixture: ForumFixture,
    marker: &str,
    entity_type: &str,
    status: &str,
    expected_id: Uuid,
    expected_total: u64,
) -> TestResult<()> {
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
                locale: "en".to_string(),
            }),
            transport: StorefrontSearchTransport::Graphql,
        },
    )
    .await?;
    if execution.result.total != expected_total {
        return Err(test_error(format!(
            "storefront marker `{marker}` returned total {}, expected {expected_total}",
            execution.result.total
        )));
    }
    if expected_total == 1 {
        if execution.result.items.len() != 1 || execution.result.items[0].id != expected_id {
            return Err(test_error(format!(
                "storefront marker `{marker}` did not return exact owner object {expected_id}: {:?}",
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
            "denied storefront marker `{marker}` leaked items or visible facets"
        )));
    }
    Ok(())
}

async fn insert_stale_search_documents(
    db: &DatabaseConnection,
    fixture: ForumFixture,
) -> TestResult<()> {
    for (document_id, entity_type, status, marker, slug, topic_id) in [
        (
            fixture.hidden_reply_id,
            "forum_reply",
            "approved",
            HIDDEN_REPLY_MARKER,
            None,
            Some(fixture.hidden_reply_topic_id),
        ),
        (
            fixture.deleted_topic_id,
            "forum_topic",
            "open",
            DELETED_TOPIC_MARKER,
            Some("d8-deleted-topic"),
            None,
        ),
        (
            fixture.acl_topic_id,
            "forum_topic",
            "open",
            ACL_TOPIC_MARKER,
            Some("d8-acl-topic"),
            None,
        ),
    ] {
        let document_key = format!("{entity_type}:{document_id}:en");
        let facets = json!({
            "kind": entity_type,
            "category_id": fixture.category_id
        });
        let payload = json!({
            "category_id": fixture.category_id,
            "topic_id": topic_id.unwrap_or(document_id),
            "owner_state": "intentionally_stale"
        });
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_documents (
                document_key, tenant_id, document_id, source_module, entity_type,
                locale, status, is_public, title, subtitle, slug, handle, body,
                keywords_text, facets, payload, published_at, created_at, updated_at, indexed_at
            ) VALUES (
                $1, $2, $3, 'forum', $4, 'en', $5, TRUE, $6, NULL, $7, NULL, $8,
                $9, $10, $11, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            "#,
            vec![
                document_key.into(),
                fixture.tenant_id.into(),
                document_id.into(),
                entity_type.to_string().into(),
                status.to_string().into(),
                format!("Intentionally stale {marker}").into(),
                slug.map(str::to_string).into(),
                format!("Intentionally stale body {marker}").into(),
                marker.to_string().into(),
                facets.into(),
                payload.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn count_stale_markers(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS value
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'forum'
              AND payload ->> 'owner_state' = 'intentionally_stale'
            "#,
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row = db
        .query_one_raw(statement)
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

fn write_evidence(artifact: DeletionAclEvidenceArtifact) -> TestResult<()> {
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
