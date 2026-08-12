use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortError, RequestContext, RichTextDocument};
use rustok_core::{MigrationSource, SecurityContext, UserRole, events::EventTransport};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    ForumSearchProjectionEvent,
};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumEventService,
    ForumModule, ForumProjectionOwnerRevisionImpact as ForumOwnerRevisionImpact,
    ForumSearchProjectionSourceFactory, ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, TopicService,
};
use rustok_iggy::{
    ConsumedContractEvent, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractConsumerGroup, PersistentContractDelivery, SerializationFormat,
    TopologyConfig,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC,
    ForumProjectionOwnerRevisionImpact as SearchOwnerRevisionImpact,
    ForumProjectionOwnerRevisionRecord, ForumProjectionOwnerRevisionRequest,
    ForumProjectionOwnerRevisionSourcePort, ForumProjectionOwnerTenantHead,
    ForumProjectionOwnerTenantPageRequest, ForumProjectionReconciler, ForumSearchContractIngress,
    ForumSearchContractIngressOutcome, ForumStorefrontSearchAttributeFilter,
    ForumStorefrontSearchRequest, SearchModule, SearchProjectionSourceFactory,
    SharedStorefrontSearchCategoryScopePort, SharedStorefrontSearchResultEligibilityPort,
    StorefrontSearchCategoryScopePort, StorefrontSearchCategoryScopeRequest,
    StorefrontSearchResultCandidate, StorefrontSearchResultCandidateKind,
    StorefrontSearchResultEligibilityPort, StorefrontSearchResultEligibilityRequest,
    StorefrontSearchTransport, execute_forum_storefront_search,
};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use tokio::time::timeout;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const FORUM_TEST_DATABASE_ENV: &str = "RUSTOK_FORUM_TEST_DATABASE_URL";
const IGGY_ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";
const IGGY_USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const IGGY_PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const TYPED_EVENT_TYPE: &str = "forum.search_projection.invalidation_issued";
const TOPIC_MARKER: &str = "d10normaldeliverytopic";
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_normal_delivery_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-normal-delivery-evidence.json";

struct PostgresNormalDeliveryEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresNormalDeliveryEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum normal-delivery proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_normal_delivery_{}_{}",
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
            create_checkpoint_audit(&db).await?;
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
        "forum.search_projection_owner_revision.normal_delivery_unavailable",
        "Forum projection owner revision source is temporarily unavailable",
    )
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
                    "forum.search_projection.normal_delivery_eligibility_unavailable",
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

#[derive(Clone, Copy, Debug)]
struct ForumFixture {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct OwnerRevisionRow {
    revision: i64,
    event_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
struct BrokerDeliveryFact {
    owner_revision: i64,
    root_event_id: Uuid,
    typed_envelope_id: Uuid,
    offset: u64,
    ingest_sequence: i64,
}

#[derive(Clone, Debug, Serialize)]
struct InboxRow {
    event_id: Uuid,
    ingest_sequence: i64,
    scope_key: String,
    status: String,
    attempt_count: i32,
    envelope_json: JsonValue,
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

#[derive(Clone, Debug, Serialize)]
struct SearchDocumentRow {
    document_id: Uuid,
    entity_type: String,
    status: String,
    title: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct NormalDeliveryEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    delivery_profile: &'static str,
    consumer_group: &'static str,
    stream: String,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn normal_owner_delivery_projects_and_checkpoints_exactly_once() -> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!("{IGGY_ADDRESS_ENV} is not set; skipping Forum normal-delivery Iggy proof");
        return Ok(());
    };
    let Some(evidence) = PostgresNormalDeliveryEvidence::setup("normal").await? else {
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let proof = run_normal_delivery_proof(&evidence.db, config).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(NormalDeliveryEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D10",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        delivery_profile: "outbox_iggy",
        consumer_group: FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        stream,
        topic: FORUM_SEARCH_CONTRACT_TOPIC,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_normal_delivery_proof(
    db: &DatabaseConnection,
    config: IggyConfig,
) -> TestResult<ScenarioEvidence> {
    let fixture = create_forum_fixture(db).await?;
    let revisions = load_owner_revisions(db, fixture.tenant_id).await?;
    ensure_owner_revision_shape(&revisions, fixture)?;
    let roots = load_root_envelopes(db, fixture.tenant_id).await?;
    let typed = load_typed_envelopes(db, fixture.tenant_id).await?;
    ensure_owner_event_identity(&revisions, &roots, &typed, fixture)?;

    if count_rows(db, "search_projection_inbox").await? != 0
        || count_rows(db, "search_projection_owner_checkpoints").await? != 0
        || count_forum_documents(db, fixture.tenant_id).await? != 0
    {
        return Err(test_error(
            "normal-delivery fixture started with unexpected Search state",
        ));
    }

    let transport = IggyTransport::new(config).await?;
    let transport_proof = async {
        let group = transport
            .open_persistent_contract_consumer_group(
                FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
                FORUM_SEARCH_CONTRACT_TOPIC,
            )
            .await?;
        for envelope in &typed {
            transport.publish_contract(envelope.clone()).await?;
        }

        let ingress = ForumSearchContractIngress::new(db.clone());
        let mut deliveries = Vec::new();
        for (revision, expected) in revisions.iter().zip(&typed) {
            let delivery = receive_event(&group).await?;
            ensure_delivery_identity(&delivery, expected, revision, fixture.tenant_id)?;
            let offset = delivery
                .offset()
                .ok_or_else(|| test_error("normal Iggy delivery has no broker offset"))?;
            ensure_durable_outcome(
                ingress.ingest(&delivery.envelope).await?,
                revision.event_id,
                revision.revision,
            )?;
            let inbox = load_inbox_row(db, revision.event_id).await?;
            ensure_pending_inbox(&inbox, revision, fixture)?;
            group.acknowledge(&delivery).await?;
            deliveries.push(BrokerDeliveryFact {
                owner_revision: revision.revision,
                root_event_id: revision.event_id,
                typed_envelope_id: expected.id(),
                offset,
                ingest_sequence: inbox.ingest_sequence,
            });
        }
        drop(group);
        Ok::<Vec<BrokerDeliveryFact>, Box<dyn Error + Send + Sync>>(deliveries)
    }
    .await;
    let shutdown = transport.shutdown().await;
    let deliveries = transport_proof?;
    shutdown?;

    if deliveries.len() != 2
        || deliveries[1].offset <= deliveries[0].offset
        || deliveries[1].ingest_sequence <= deliveries[0].ingest_sequence
    {
        return Err(test_error(format!(
            "normal delivery did not preserve independent monotonic broker and inbox order: {deliveries:?}"
        )));
    }

    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let owner_source = Arc::new(RealForumOwnerRevisionSource { db: db.clone() });
    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        db.clone(),
        projection_source,
        owner_source,
    );
    let report = reconciler.sweep_due(8, 8).await?;
    if report.due_tenants != 1
        || report.claimed_events != 2
        || report.completed_events != 2
        || report.failed_events != 0
        || report.owner_tenants_scanned != 1
        || report.owner_tenants_reconciled != 1
        || report.owner_tenants_blocked != 0
        || report.owner_tenants_failed != 0
        || report.owner_revisions_checkpointed != 2
        || report.owner_rebuilds != 0
    {
        return Err(test_error(format!(
            "normal delivery produced an unexpected reconciler report: {report:?}"
        )));
    }

    let completed_inbox = load_inbox_rows(db, fixture.tenant_id).await?;
    if completed_inbox.len() != 2
        || completed_inbox
            .iter()
            .any(|row| row.status != "completed" || row.attempt_count != 1)
    {
        return Err(test_error(format!(
            "normal delivery did not complete exactly two inbox rows once: {completed_inbox:?}"
        )));
    }

    let checkpoint = load_checkpoint(db, fixture.tenant_id)
        .await?
        .ok_or_else(|| test_error("normal delivery did not create an owner checkpoint"))?;
    if checkpoint.owner_revision != 2
        || checkpoint.event_id != revisions[1].event_id
        || checkpoint.outcome != "delivery_covered"
    {
        return Err(test_error(format!(
            "normal delivery stored an unexpected final checkpoint: {checkpoint:?}"
        )));
    }
    let audit = load_checkpoint_audit(db, fixture.tenant_id).await?;
    if audit
        .iter()
        .map(|row| row.owner_revision)
        .collect::<Vec<_>>()
        != [1, 2]
        || audit.iter().map(|row| row.event_id).collect::<Vec<_>>()
            != revisions.iter().map(|row| row.event_id).collect::<Vec<_>>()
        || audit
            .iter()
            .any(|row| row.outcome != "delivery_covered" || row.observed_forum_documents != 2)
    {
        return Err(test_error(format!(
            "normal delivery checkpoint audit drifted from projection-before-checkpoint ordering: {audit:?}"
        )));
    }

    let documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_projected_documents(&documents, fixture)?;
    let storefront_total = assert_storefront_topic(db, fixture).await?;

    let caught_up = reconciler.sweep_due(8, 8).await?;
    if caught_up.due_tenants != 0
        || caught_up.claimed_events != 0
        || caught_up.completed_events != 0
        || caught_up.owner_tenants_reconciled != 0
        || caught_up.owner_revisions_checkpointed != 0
        || caught_up.owner_rebuilds != 0
        || load_checkpoint_audit(db, fixture.tenant_id).await?.len() != 2
    {
        return Err(test_error(format!(
            "caught-up normal delivery repeated projection or checkpoint work: {caught_up:?}"
        )));
    }

    Ok(ScenarioEvidence {
        id: "normal_delivery",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "category_id": fixture.category_id,
            "topic_id": fixture.topic_id,
            "owner_revision_rows": revisions,
            "legacy_root_event_ids": roots.iter().map(|envelope| envelope.id).collect::<Vec<_>>(),
            "typed_envelope_ids": typed.iter().map(ContractEventEnvelope::id).collect::<Vec<_>>(),
            "typed_causation_ids": typed.iter().map(ContractEventEnvelope::causation_id).collect::<Vec<_>>(),
            "broker_deliveries": deliveries,
            "completed_inbox_rows": completed_inbox,
            "reconciler_claimed_events": report.claimed_events,
            "reconciler_completed_events": report.completed_events,
            "owner_revisions_checkpointed": report.owner_revisions_checkpointed,
            "owner_repair_rebuilds": report.owner_rebuilds,
            "checkpoint_audit_revisions": audit.iter().map(|row| row.owner_revision).collect::<Vec<_>>(),
            "checkpoint_audit_outcomes": audit.iter().map(|row| row.outcome.clone()).collect::<Vec<_>>(),
            "checkpoint_audit_document_counts": audit.iter().map(|row| row.observed_forum_documents).collect::<Vec<_>>(),
            "final_checkpoint_revision": checkpoint.owner_revision,
            "final_checkpoint_event_id": checkpoint.event_id,
            "final_checkpoint_outcome": checkpoint.outcome,
            "projected_documents": documents,
            "storefront_topic_total": storefront_total,
            "caught_up_repeat_claims": caught_up.claimed_events,
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
            SecurityContext::new(UserRole::Admin, Some(admin_id)),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "D10 normal delivery category".to_string(),
                slug: "d10-normal-delivery-category".to_string(),
                description: Some("Normal delivery owner fixture".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let topic = TopicService::new(db.clone(), event_bus(db.clone()))
        .create(
            tenant_id,
            SecurityContext::new(UserRole::Customer, None),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: format!("D10 normal delivery {TOPIC_MARKER}"),
                slug: Some("d10-normal-delivery-topic".to_string()),
                body: RichTextDocument::single_paragraph(format!(
                    "Current Forum owner body {TOPIC_MARKER}"
                )),
                metadata: json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?;

    Ok(ForumFixture {
        tenant_id,
        category_id: category.id,
        topic_id: topic.id,
    })
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
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

fn ensure_owner_revision_shape(
    revisions: &[OwnerRevisionRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if revisions.len() != 2
        || revisions[0].revision != 1
        || revisions[0].target_type != "forum"
        || revisions[0].target_id.is_some()
        || revisions[1].revision != 2
        || revisions[1].target_type != "forum_category"
        || revisions[1].target_id != Some(fixture.category_id)
    {
        return Err(test_error(format!(
            "normal delivery owner revisions have an unexpected shape: {revisions:?}"
        )));
    }
    Ok(())
}

async fn load_root_envelopes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<EventEnvelope>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![ROOT_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut envelopes = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id == tenant_id {
            envelopes.push(envelope);
        }
    }
    Ok(envelopes)
}

async fn load_typed_envelopes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<ContractEventEnvelope>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload FROM sys_events WHERE event_type = $1 ORDER BY created_at ASC",
            vec![TYPED_EVENT_TYPE.to_string().into()],
        ))
        .await?;
    let mut envelopes = Vec::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: ContractEventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id() == tenant_id {
            envelopes.push(envelope);
        }
    }
    Ok(envelopes)
}

fn ensure_owner_event_identity(
    revisions: &[OwnerRevisionRow],
    roots: &[EventEnvelope],
    typed: &[ContractEventEnvelope],
    fixture: ForumFixture,
) -> TestResult<()> {
    if roots.len() != 2 || typed.len() != 2 {
        return Err(test_error(format!(
            "normal delivery expected two root and two typed events: roots={}, typed={}",
            roots.len(),
            typed.len()
        )));
    }
    let root_ids = roots
        .iter()
        .map(|envelope| envelope.id)
        .collect::<BTreeSet<_>>();
    let revision_ids = revisions
        .iter()
        .map(|revision| revision.event_id)
        .collect::<BTreeSet<_>>();
    if root_ids != revision_ids {
        return Err(test_error(format!(
            "normal delivery ledger and root event identities diverged: roots={root_ids:?}, revisions={revision_ids:?}"
        )));
    }

    for (revision, envelope) in revisions.iter().zip(typed) {
        envelope.validate_registered_schema()?;
        if envelope.tenant_id() != fixture.tenant_id
            || envelope.causation_id() != Some(revision.event_id)
            || envelope.id() == revision.event_id
            || envelope.event_type() != TYPED_EVENT_TYPE
            || envelope.schema_version() != 1
        {
            return Err(test_error(format!(
                "typed normal-delivery envelope lost transport/root identity: {envelope:?}"
            )));
        }
        match envelope.payload()? {
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
                    "typed normal-delivery payload does not match owner revision: {payload:?}"
                )));
            }
        }
    }
    Ok(())
}

async fn receive_event(
    group: &PersistentContractConsumerGroup,
) -> TestResult<ConsumedContractEvent> {
    let delivery = timeout(RECEIVE_TIMEOUT, group.receive_delivery())
        .await
        .map_err(|_| test_error("timed out waiting for a normal Forum Iggy delivery"))??
        .ok_or_else(|| test_error("Iggy consumer group ended before normal delivery"))?;
    match delivery {
        PersistentContractDelivery::Event(consumed) => Ok(consumed),
        PersistentContractDelivery::DecodeFailure(failure) => Err(test_error(format!(
            "normal Forum typed event decoded as poison: {}",
            failure.stable_error_code()
        ))),
    }
}

fn ensure_delivery_identity(
    delivery: &ConsumedContractEvent,
    expected: &ContractEventEnvelope,
    revision: &OwnerRevisionRow,
    tenant_id: Uuid,
) -> TestResult<()> {
    if delivery.topic != FORUM_SEARCH_CONTRACT_TOPIC
        || delivery.envelope != *expected
        || delivery.envelope.id() != expected.id()
        || delivery.envelope.causation_id() != Some(revision.event_id)
        || delivery.envelope.tenant_id() != tenant_id
        || delivery.offset().is_none()
        || delivery.ack_token().is_none()
        || delivery.raw_payload().is_empty()
    {
        return Err(test_error(format!(
            "normal Iggy delivery identity drifted: {delivery:?}"
        )));
    }
    delivery.validate_connector_metadata()?;
    delivery.envelope.validate_registered_schema()?;
    Ok(())
}

fn ensure_durable_outcome(
    outcome: ForumSearchContractIngressOutcome,
    root_event_id: Uuid,
    owner_revision: i64,
) -> TestResult<()> {
    match outcome {
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id: actual_root,
            owner_revision: actual_revision,
        } if actual_root == root_event_id && actual_revision == owner_revision => Ok(()),
        other => Err(test_error(format!(
            "normal delivery ingress returned an unexpected outcome: {other:?}"
        ))),
    }
}

async fn load_inbox_row(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxRow> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT event_id, ingest_sequence, scope_key, status, attempt_count, envelope_json
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("normal Search inbox row {event_id} was not found")))?;
    Ok(InboxRow {
        event_id: row.try_get("", "event_id")?,
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        scope_key: row.try_get("", "scope_key")?,
        status: row.try_get("", "status")?,
        attempt_count: row.try_get("", "attempt_count")?,
        envelope_json: row.try_get("", "envelope_json")?,
    })
}

async fn load_inbox_rows(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Vec<InboxRow>> {
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT event_id, ingest_sequence, scope_key, status, attempt_count, envelope_json
        FROM search_projection_inbox
        WHERE tenant_id = $1 AND source_module = 'forum'
        ORDER BY ingest_sequence ASC
        "#,
        vec![tenant_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(InboxRow {
            event_id: row.try_get("", "event_id")?,
            ingest_sequence: row.try_get("", "ingest_sequence")?,
            scope_key: row.try_get("", "scope_key")?,
            status: row.try_get("", "status")?,
            attempt_count: row.try_get("", "attempt_count")?,
            envelope_json: row.try_get("", "envelope_json")?,
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

fn ensure_pending_inbox(
    inbox: &InboxRow,
    revision: &OwnerRevisionRow,
    fixture: ForumFixture,
) -> TestResult<()> {
    let root: EventEnvelope = serde_json::from_value(inbox.envelope_json.clone())?;
    let expected_scope = if revision.target_type == "forum" {
        "forum".to_string()
    } else {
        format!("forum_category:{}", fixture.category_id)
    };
    if inbox.event_id != revision.event_id
        || inbox.ingest_sequence <= 0
        || inbox.scope_key != expected_scope
        || inbox.status != "pending"
        || inbox.attempt_count != 0
        || root.id != revision.event_id
        || root.tenant_id != fixture.tenant_id
        || root.event_type != ROOT_EVENT_TYPE
        || root.causation_id.is_some()
        || !matches!(
            root.event,
            DomainEvent::ReindexRequested {
                ref target_type,
                target_id,
            } if target_type == &revision.target_type && target_id == revision.target_id
        )
    {
        return Err(test_error(format!(
            "normal delivery created an unexpected pending inbox row: {inbox:?}"
        )));
    }
    root.validate_registered_schema()?;
    Ok(())
}

async fn create_checkpoint_audit(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE forum_normal_delivery_checkpoint_audit (
            sequence BIGSERIAL PRIMARY KEY,
            tenant_id UUID NOT NULL,
            owner_revision BIGINT NOT NULL,
            event_id UUID NOT NULL,
            outcome VARCHAR(32) NOT NULL,
            observed_forum_documents BIGINT NOT NULL
        );

        CREATE OR REPLACE FUNCTION forum_capture_normal_delivery_checkpoint()
        RETURNS trigger AS $$
        BEGIN
            INSERT INTO forum_normal_delivery_checkpoint_audit (
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

        CREATE TRIGGER forum_normal_delivery_checkpoint_audit
        AFTER INSERT OR UPDATE ON search_projection_owner_checkpoints
        FOR EACH ROW EXECUTE FUNCTION forum_capture_normal_delivery_checkpoint();
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
        FROM forum_normal_delivery_checkpoint_audit
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

fn ensure_projected_documents(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 2 {
        return Err(test_error(format!(
            "normal delivery projected {} Forum documents instead of two: {documents:?}",
            documents.len()
        )));
    }
    let category = documents
        .iter()
        .find(|document| document.document_id == fixture.category_id)
        .ok_or_else(|| test_error("normal delivery omitted the Forum category"))?;
    let topic = documents
        .iter()
        .find(|document| document.document_id == fixture.topic_id)
        .ok_or_else(|| test_error("normal delivery omitted the Forum topic"))?;
    if category.entity_type != "forum_category"
        || category.status != "public"
        || category.title != "D10 normal delivery category"
        || topic.entity_type != "forum_topic"
        || topic.status != "open"
        || !topic.title.contains(TOPIC_MARKER)
        || !topic.body.contains(TOPIC_MARKER)
    {
        return Err(test_error(format!(
            "normal delivery projected unexpected current Forum documents: {documents:?}"
        )));
    }
    Ok(())
}

async fn assert_storefront_topic(
    db: &DatabaseConnection,
    fixture: ForumFixture,
) -> TestResult<u64> {
    let category_scope: SharedStorefrontSearchCategoryScopePort = Arc::new(ExactCategoryScopePort);
    let eligibility: SharedStorefrontSearchResultEligibilityPort =
        Arc::new(RealForumPublicEligibilityPort { db: db.clone() });
    let execution = execute_forum_storefront_search(
        db,
        Some(category_scope),
        Some(eligibility),
        ForumStorefrontSearchRequest {
            tenant_id: fixture.tenant_id,
            query: TOPIC_MARKER.to_string(),
            locale: Some("en".to_string()),
            fallback_locale: "en".to_string(),
            channel_id: None,
            current_channel_only: None,
            limit: Some(10),
            offset: Some(0),
            ranking_profile: None,
            preset_key: None,
            entity_types: vec!["forum_topic".to_string()],
            source_modules: vec!["forum".to_string()],
            statuses: vec!["open".to_string()],
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
    if execution.result.total != 1
        || execution.result.items.len() != 1
        || execution.result.items[0].id != fixture.topic_id
    {
        return Err(test_error(format!(
            "normal delivery storefront Search did not return the exact topic: {:?}",
            execution.result
        )));
    }
    Ok(execution.result.total)
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

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row = db
        .query_one(statement)
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn external_test_config() -> TestResult<Option<IggyConfig>> {
    let address = match env::var(IGGY_ADDRESS_ENV) {
        Ok(value) => bounded_env(IGGY_ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(test_error(
            "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS must be host:port without credentials or query parameters",
        ));
    }
    let username = optional_bounded_env(IGGY_USERNAME_ENV, 191)?;
    let password = optional_bounded_env(IGGY_PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(test_error(
            "external Iggy evidence username and password must both be set or both be empty",
        ));
    }
    Ok(Some(IggyConfig {
        mode: IggyMode::External,
        serialization: SerializationFormat::Json,
        external: ExternalConfig {
            addresses: vec![address],
            protocol: "tcp".to_string(),
            username,
            password,
            tls_enabled: false,
            tls_domain: None,
            tls_ca_file: None,
        },
        topology: TopologyConfig {
            stream_name: unique_name("normal"),
            domain_partitions: 1,
            replication_factor: 1,
        },
        ..IggyConfig::default()
    }))
}

fn optional_bounded_env(name: &'static str, max_len: usize) -> TestResult<String> {
    match env::var(name) {
        Ok(value) => Ok(bounded_env(name, value, max_len)?),
        Err(env::VarError::NotPresent) => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn bounded_env(name: &'static str, value: String, max_len: usize) -> Result<String, IoError> {
    if value.trim() != value || value.is_empty() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{name} must be non-empty and have no surrounding whitespace"),
        ));
    }
    if value.len() > max_len {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{name} exceeds the evidence limit"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{name} must not contain control characters"),
        ));
    }
    Ok(value)
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

fn unique_name(scope: &str) -> String {
    format!("rustok-forum-search-{scope}-{}", Uuid::new_v4().simple())
}

fn write_evidence(artifact: NormalDeliveryEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| test_error("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
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
