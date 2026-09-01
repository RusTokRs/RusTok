use std::collections::{BTreeSet, HashSet};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError, RequestContext, RichTextDocument};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    ForumSearchProjectionEvent,
};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumModule, ForumSearchProjectionSourceFactory,
    ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, ForumTopicAudiencePolicyService,
    SetForumCategoryAudiencePolicyInput, SetForumTopicAudiencePolicyInput,
    SharedForumAudienceFactsPort, TopicService,
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
const EVIDENCE_CONTRACT: &str = "forum_search_link_forum_03_private_trusted_exclusion_proof_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json";
const PUBLIC_TOPIC_MARKER: &str = "d15publictopicmarker";
const PRIVATE_TOPIC_MARKER: &str = "d15privatetopicmarker";
const TRUSTED_TOPIC_MARKER: &str = "d15trustedtopicmarker";
const TRUSTED_CHANNEL: &str = "trusted";
const WRONG_CHANNEL: &str = "general";
const MINIMUM_TRUST: u8 = 50;

struct PostgresPrivateTrustedEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresPrivateTrustedEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum private/trusted exclusion proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url, 1).await?;
        let schema_name = format!(
            "rustok_forum_private_trusted_{}_{}",
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
    public_category_id: Uuid,
    trusted_category_id: Uuid,
    public_topic_id: Uuid,
    private_topic_id: Uuid,
    trusted_topic_id: Uuid,
    private_user_id: Uuid,
    outsider_user_id: Uuid,
    low_trust_member_id: Uuid,
    high_trust_nonmember_id: Uuid,
    high_trust_member_id: Uuid,
    trusted_channel_id: Uuid,
    wrong_channel_id: Uuid,
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
struct SearchDocumentRow {
    document_id: Uuid,
    entity_type: String,
    locale: String,
    status: String,
    title: String,
    body: String,
}

#[derive(Clone, Debug, Serialize)]
struct StorefrontFact {
    label: &'static str,
    expected_total: u64,
    actual_total: u64,
    item_ids: Vec<Uuid>,
    visible_facet_buckets: usize,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct PrivateTrustedEvidenceArtifact {
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
struct ExactAudienceFactsPort {
    tenant_id: Uuid,
    user_id: Uuid,
    trust_level: Option<u8>,
    channel_memberships: Vec<String>,
    observed: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
}

#[async_trait]
impl ForumAudienceFactsPort for ExactAudienceFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        if request.tenant_id != self.tenant_id
            || request.user_id != self.user_id
            || context.tenant_id != self.tenant_id.to_string()
            || context.actor.id != self.user_id.to_string()
            || context.deadline_ms.unwrap_or_default() == 0
        {
            return Err(PortError::validation(
                "forum.d15.audience_context_mismatch",
                "D15 audience facts request identity or deadline drifted",
            ));
        }
        self.observed
            .lock()
            .map_err(|_| {
                PortError::unavailable(
                    "forum.d15.audience_observation_unavailable",
                    "D15 audience observation is unavailable",
                )
            })?
            .push(request.clone());

        let requested_channels = request
            .channel_slugs
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let channel_memberships = self
            .channel_memberships
            .iter()
            .filter(|slug| requested_channels.contains(*slug))
            .cloned()
            .collect::<Vec<_>>();
        Ok(ForumAudienceFacts {
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            trust_level: if request.include_trust_level {
                self.trust_level
            } else {
                None
            },
            channel_memberships,
            group_memberships: Vec::new(),
        })
    }
}

#[derive(Clone)]
enum ViewerMode {
    Public,
    Authenticated {
        user_id: Uuid,
        facts: SharedForumAudienceFactsPort,
    },
}

#[derive(Clone)]
struct ExactForumEligibilityPort {
    db: DatabaseConnection,
    mode: ViewerMode,
}

impl ExactForumEligibilityPort {
    fn public(db: DatabaseConnection) -> SharedStorefrontSearchResultEligibilityPort {
        Arc::new(Self {
            db,
            mode: ViewerMode::Public,
        })
    }

    fn authenticated(
        db: DatabaseConnection,
        user_id: Uuid,
        facts: SharedForumAudienceFactsPort,
    ) -> SharedStorefrontSearchResultEligibilityPort {
        Arc::new(Self {
            db,
            mode: ViewerMode::Authenticated { user_id, facts },
        })
    }
}

#[async_trait]
impl StorefrontSearchResultEligibilityPort for ExactForumEligibilityPort {
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
        let allowed = match &self.mode {
            ViewerMode::Public => {
                ForumSearchResultEligibilityService::new(self.db.clone())
                    .filter_public_storefront_visible(request.tenant_id, channel_slug, &candidates)
                    .await
            }
            ViewerMode::Authenticated { user_id, facts } => {
                let mut context = PortContext::new(
                    request.tenant_id.to_string(),
                    PortActor::user(user_id.to_string()),
                    request.locale.clone(),
                    format!("d15-{}", Uuid::new_v4()),
                )
                .with_deadline(Duration::from_secs(5));
                if let Some(channel_slug) = channel_slug {
                    context = context.with_channel(channel_slug.to_string());
                }
                ForumSearchResultEligibilityService::with_audience_facts(
                    self.db.clone(),
                    facts.clone(),
                )
                .filter_authenticated_storefront_visible(
                    request.tenant_id,
                    SecurityContext::new(UserRole::Customer, Some(*user_id)),
                    context,
                    &candidates,
                )
                .await
            }
        }
        .map_err(|error| {
            PortError::unavailable(
                "forum.d15.search_result_eligibility_failed",
                format!("D15 Forum eligibility failed: {error}"),
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
async fn private_and_trusted_channel_candidates_fail_closed_in_storefront_search() -> TestResult<()>
{
    let Some(evidence) = PostgresPrivateTrustedEvidence::setup("link").await? else {
        return Ok(());
    };

    let proof = run_private_trusted_exclusion_proof(&evidence.db).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(PrivateTrustedEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D15",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_used: false,
        scenario_results: vec![scenario],
    })?;
    Ok(())
}

async fn run_private_trusted_exclusion_proof(
    db: &DatabaseConnection,
) -> TestResult<ScenarioEvidence> {
    let fixture = create_forum_fixture(db).await?;
    let revisions = load_owner_revisions_after(db, fixture.tenant_id, 0).await?;
    ensure_revision_shape(&revisions, fixture)?;
    let deliveries = ingest_exact_typed_revisions(db, fixture.tenant_id, &revisions).await?;

    let projection_source = ForumSearchProjectionSourceFactory.build(db.clone());
    let reconciler = ForumProjectionReconciler::new(db.clone(), projection_source);
    let report = reconciler.sweep_due(1, 32).await?;
    if report.claimed_events != 8 || report.completed_events != 8 || report.failed_events != 0 {
        return Err(test_error(format!(
            "D15 projection did not complete exactly eight owner events: {report:?}"
        )));
    }

    let legitimate_documents = load_forum_documents(db, fixture.tenant_id).await?;
    ensure_legitimate_projection(&legitimate_documents, fixture)?;
    let legitimate_private_topic_documents =
        count_forum_document(db, fixture.tenant_id, fixture.private_topic_id).await?;
    let legitimate_trusted_topic_documents =
        count_forum_document(db, fixture.tenant_id, fixture.trusted_topic_id).await?;
    if legitimate_private_topic_documents != 0 || legitimate_trusted_topic_documents != 0 {
        return Err(test_error(format!(
            "D15 legitimate projection retained restricted topic rows: private={legitimate_private_topic_documents}, trusted={legitimate_trusted_topic_documents}"
        )));
    }

    let category_scope: SharedStorefrontSearchCategoryScopePort = Arc::new(ExactCategoryScopePort);
    let public_eligibility = ExactForumEligibilityPort::public(db.clone());
    let public_control = assert_storefront_exact(
        db,
        category_scope.clone(),
        public_eligibility.clone(),
        fixture,
        fixture.public_category_id,
        None,
        PUBLIC_TOPIC_MARKER,
        fixture.public_topic_id,
        1,
        "public_control",
    )
    .await?;

    insert_stale_topic_documents(db, fixture).await?;
    if count_stale_documents(db, fixture.tenant_id).await? != 2 {
        return Err(test_error(
            "D15 stale candidate injection did not create exactly two rows",
        ));
    }

    let public_private = assert_storefront_exact(
        db,
        category_scope.clone(),
        public_eligibility.clone(),
        fixture,
        fixture.public_category_id,
        None,
        PRIVATE_TOPIC_MARKER,
        fixture.private_topic_id,
        0,
        "public_private_denied",
    )
    .await?;
    let public_trusted = assert_storefront_exact(
        db,
        category_scope.clone(),
        public_eligibility,
        fixture,
        fixture.trusted_category_id,
        Some(TRUSTED_CHANNEL),
        TRUSTED_TOPIC_MARKER,
        fixture.trusted_topic_id,
        0,
        "public_trusted_denied",
    )
    .await?;

    let private_allowed_observed = Arc::new(Mutex::new(Vec::new()));
    let private_allowed_facts = exact_facts_port(
        fixture,
        fixture.private_user_id,
        None,
        Vec::new(),
        private_allowed_observed.clone(),
    );
    let private_allowed = assert_storefront_exact(
        db,
        category_scope.clone(),
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.private_user_id,
            private_allowed_facts,
        ),
        fixture,
        fixture.public_category_id,
        None,
        PRIVATE_TOPIC_MARKER,
        fixture.private_topic_id,
        1,
        "private_explicit_user_allowed",
    )
    .await?;
    ensure_no_fact_requests(&private_allowed_observed, "private explicit allow")?;

    let outsider_observed = Arc::new(Mutex::new(Vec::new()));
    let outsider_facts = exact_facts_port(
        fixture,
        fixture.outsider_user_id,
        None,
        Vec::new(),
        outsider_observed.clone(),
    );
    let private_outsider = assert_storefront_exact(
        db,
        category_scope.clone(),
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.outsider_user_id,
            outsider_facts,
        ),
        fixture,
        fixture.public_category_id,
        None,
        PRIVATE_TOPIC_MARKER,
        fixture.private_topic_id,
        0,
        "private_outsider_denied",
    )
    .await?;
    ensure_no_fact_requests(&outsider_observed, "private outsider")?;

    let low_trust_observed = Arc::new(Mutex::new(Vec::new()));
    let low_trust_facts = exact_facts_port(
        fixture,
        fixture.low_trust_member_id,
        Some(10),
        vec![TRUSTED_CHANNEL.to_string()],
        low_trust_observed.clone(),
    );
    let trusted_low_trust = assert_storefront_exact(
        db,
        category_scope.clone(),
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.low_trust_member_id,
            low_trust_facts,
        ),
        fixture,
        fixture.trusted_category_id,
        Some(TRUSTED_CHANNEL),
        TRUSTED_TOPIC_MARKER,
        fixture.trusted_topic_id,
        0,
        "trusted_low_trust_denied",
    )
    .await?;
    ensure_fact_request_shape(
        &low_trust_observed,
        &[(true, Vec::<String>::new())],
        "low-trust member",
    )?;

    let nonmember_observed = Arc::new(Mutex::new(Vec::new()));
    let nonmember_facts = exact_facts_port(
        fixture,
        fixture.high_trust_nonmember_id,
        Some(80),
        Vec::new(),
        nonmember_observed.clone(),
    );
    let trusted_nonmember = assert_storefront_exact(
        db,
        category_scope.clone(),
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.high_trust_nonmember_id,
            nonmember_facts,
        ),
        fixture,
        fixture.trusted_category_id,
        Some(TRUSTED_CHANNEL),
        TRUSTED_TOPIC_MARKER,
        fixture.trusted_topic_id,
        0,
        "trusted_nonmember_denied",
    )
    .await?;
    ensure_fact_request_shape(
        &nonmember_observed,
        &[
            (true, Vec::<String>::new()),
            (false, vec![TRUSTED_CHANNEL.to_string()]),
        ],
        "high-trust non-member",
    )?;

    let wrong_route_observed = Arc::new(Mutex::new(Vec::new()));
    let wrong_route_facts = exact_facts_port(
        fixture,
        fixture.high_trust_member_id,
        Some(80),
        vec![TRUSTED_CHANNEL.to_string()],
        wrong_route_observed.clone(),
    );
    let trusted_wrong_route = assert_storefront_exact(
        db,
        category_scope.clone(),
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.high_trust_member_id,
            wrong_route_facts,
        ),
        fixture,
        fixture.trusted_category_id,
        Some(WRONG_CHANNEL),
        TRUSTED_TOPIC_MARKER,
        fixture.trusted_topic_id,
        0,
        "trusted_wrong_route_denied",
    )
    .await?;
    ensure_no_fact_requests(&wrong_route_observed, "trusted wrong route")?;

    let trusted_member_observed = Arc::new(Mutex::new(Vec::new()));
    let trusted_member_facts = exact_facts_port(
        fixture,
        fixture.high_trust_member_id,
        Some(80),
        vec![TRUSTED_CHANNEL.to_string()],
        trusted_member_observed.clone(),
    );
    let trusted_member = assert_storefront_exact(
        db,
        category_scope,
        ExactForumEligibilityPort::authenticated(
            db.clone(),
            fixture.high_trust_member_id,
            trusted_member_facts,
        ),
        fixture,
        fixture.trusted_category_id,
        Some(TRUSTED_CHANNEL),
        TRUSTED_TOPIC_MARKER,
        fixture.trusted_topic_id,
        1,
        "trusted_exact_member_allowed",
    )
    .await?;
    ensure_fact_request_shape(
        &trusted_member_observed,
        &[
            (true, Vec::<String>::new()),
            (false, vec![TRUSTED_CHANNEL.to_string()]),
        ],
        "exact trusted member",
    )?;

    let root_ids = load_root_event_ids(db, fixture.tenant_id).await?;
    let revision_ids = revisions
        .iter()
        .map(|revision| revision.event_id)
        .collect::<BTreeSet<_>>();
    if root_ids != revision_ids {
        return Err(test_error(format!(
            "D15 ledger/root identities diverged: revisions={revision_ids:?}, roots={root_ids:?}"
        )));
    }
    for revision in &revisions {
        if count_inbox_rows(db, revision.event_id).await? != 1 {
            return Err(test_error(format!(
                "D15 root {} did not retain exactly one Search inbox row",
                revision.event_id
            )));
        }
    }
    let caught_up = reconciler.sweep_due(1, 32).await?;
    if caught_up.claimed_events != 0
        || caught_up.completed_events != 0
        || caught_up.failed_events != 0
    {
        return Err(test_error(format!(
            "caught-up D15 repeat performed duplicate work: {caught_up:?}"
        )));
    }

    let fact_requests = json!({
        "private_explicit_user": observed_requests(&private_allowed_observed)?,
        "private_outsider": observed_requests(&outsider_observed)?,
        "low_trust_member": observed_requests(&low_trust_observed)?,
        "high_trust_nonmember": observed_requests(&nonmember_observed)?,
        "high_trust_wrong_route": observed_requests(&wrong_route_observed)?,
        "high_trust_member": observed_requests(&trusted_member_observed)?
    });
    let storefront_matrix = vec![
        public_control,
        public_private,
        public_trusted,
        private_allowed,
        private_outsider,
        trusted_low_trust,
        trusted_nonmember,
        trusted_wrong_route,
        trusted_member,
    ];

    Ok(ScenarioEvidence {
        id: "private_and_trusted_channel_exclusion",
        result: "passed",
        facts: json!({
            "tenant_id": fixture.tenant_id,
            "public_category_id": fixture.public_category_id,
            "trusted_category_id": fixture.trusted_category_id,
            "public_topic_id": fixture.public_topic_id,
            "private_topic_id": fixture.private_topic_id,
            "trusted_topic_id": fixture.trusted_topic_id,
            "owner_revision_rows": revisions,
            "typed_ingress_deliveries": deliveries,
            "legitimate_projection_documents": legitimate_documents,
            "legitimate_private_topic_documents": legitimate_private_topic_documents,
            "legitimate_trusted_topic_documents": legitimate_trusted_topic_documents,
            "stale_search_rows_injected": 2,
            "storefront_matrix": storefront_matrix,
            "owner_fact_requests": fact_requests,
            "trusted_route_channel": TRUSTED_CHANNEL,
            "minimum_trust_level": MINIMUM_TRUST,
            "private_policy": "explicit_user_allow",
            "trusted_policy": "route_channel_and_inherited_trust_and_topic_channel_membership",
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
    let private_user_id = Uuid::new_v4();
    let outsider_user_id = Uuid::new_v4();
    let low_trust_member_id = Uuid::new_v4();
    let high_trust_nonmember_id = Uuid::new_v4();
    let high_trust_member_id = Uuid::new_v4();
    let trusted_channel_id = Uuid::new_v4();
    let wrong_channel_id = Uuid::new_v4();
    for user_id in [
        admin_id,
        private_user_id,
        outsider_user_id,
        low_trust_member_id,
        high_trust_nonmember_id,
        high_trust_member_id,
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO users (id, tenant_id) VALUES ($1, $2)",
            vec![user_id.into(), tenant_id.into()],
        ))
        .await?;
    }

    let admin = admin_security(admin_id);
    let categories = CategoryService::new(db.clone());
    let public_category = categories
        .create(
            tenant_id,
            admin.clone(),
            category_input("D15 public category", "d15-public-category", 0),
        )
        .await?;
    let trusted_category = categories
        .create(
            tenant_id,
            admin.clone(),
            category_input("D15 trusted category", "d15-trusted-category", 1),
        )
        .await?;

    let bus = event_bus(db.clone());
    let topics = TopicService::new(db.clone(), bus);
    let public_topic = topics
        .create(
            tenant_id,
            admin.clone(),
            topic_input(
                public_category.id,
                format!("D15 public topic {PUBLIC_TOPIC_MARKER}"),
                "d15-public-topic",
                None,
            ),
        )
        .await?;
    let private_topic = topics
        .create(
            tenant_id,
            admin.clone(),
            topic_input(
                public_category.id,
                format!("D15 private topic {PRIVATE_TOPIC_MARKER}"),
                "d15-private-topic",
                None,
            ),
        )
        .await?;
    let trusted_topic = topics
        .create(
            tenant_id,
            admin.clone(),
            topic_input(
                trusted_category.id,
                format!("D15 trusted topic {TRUSTED_TOPIC_MARKER}"),
                "d15-trusted-topic",
                Some(vec![TRUSTED_CHANNEL.to_string()]),
            ),
        )
        .await?;

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            private_topic.id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    allow_user_ids: vec![private_user_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await?;
    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            trusted_category.id,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    minimum_trust_level: Some(MINIMUM_TRUST),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await?;
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            trusted_topic.id,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: vec![TRUSTED_CHANNEL.to_string()],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await?;

    Ok(ForumFixture {
        tenant_id,
        public_category_id: public_category.id,
        trusted_category_id: trusted_category.id,
        public_topic_id: public_topic.id,
        private_topic_id: private_topic.id,
        trusted_topic_id: trusted_topic.id,
        private_user_id,
        outsider_user_id,
        low_trust_member_id,
        high_trust_nonmember_id,
        high_trust_member_id,
        trusted_channel_id,
        wrong_channel_id,
    })
}

fn category_input(name: &str, slug: &str, position: i32) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: slug.to_string(),
        description: Some("D15 private trusted exclusion evidence".to_string()),
        icon: None,
        color: None,
        parent_id: None,
        position: Some(position),
        moderated: false,
    }
}

fn topic_input(
    category_id: Uuid,
    title: String,
    slug: &str,
    channel_slugs: Option<Vec<String>>,
) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".to_string(),
        category_id,
        body: RichTextDocument::single_paragraph(format!("{title} body")),
        title,
        slug: Some(slug.to_string()),
        metadata: json!({}),
        tags: Vec::new(),
        channel_slugs,
    }
}

fn exact_facts_port(
    fixture: ForumFixture,
    user_id: Uuid,
    trust_level: Option<u8>,
    channel_memberships: Vec<String>,
    observed: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
) -> SharedForumAudienceFactsPort {
    Arc::new(ExactAudienceFactsPort {
        tenant_id: fixture.tenant_id,
        user_id,
        trust_level,
        channel_memberships,
        observed,
    })
}

fn ensure_revision_shape(revisions: &[OwnerRevisionRow], fixture: ForumFixture) -> TestResult<()> {
    let expected = [
        (1, "forum", None),
        (2, "forum", None),
        (3, "forum_category", Some(fixture.public_category_id)),
        (4, "forum_category", Some(fixture.public_category_id)),
        (5, "forum_category", Some(fixture.trusted_category_id)),
        (6, "forum_topic", Some(fixture.private_topic_id)),
        (7, "forum", None),
        (8, "forum_topic", Some(fixture.trusted_topic_id)),
    ];
    if revisions.len() != expected.len() {
        return Err(test_error(format!(
            "D15 expected eight owner revisions, received {revisions:?}"
        )));
    }
    for (actual, (revision, target_type, target_id)) in revisions.iter().zip(expected) {
        if actual.revision != revision
            || actual.target_type != target_type
            || actual.target_id != target_id
        {
            return Err(test_error(format!(
                "D15 owner revision shape drifted: {revisions:?}"
            )));
        }
    }
    Ok(())
}

async fn ingest_exact_typed_revisions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    revisions: &[OwnerRevisionRow],
) -> TestResult<Vec<DeliveryIdentityFact>> {
    let mut facts = Vec::with_capacity(revisions.len());
    for revision in revisions {
        let root = load_root_envelope(db, tenant_id, revision.event_id).await?;
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
                "D15 root envelope does not match owner revision: root={root:?}, revision={revision:?}"
            )));
        }
        let typed = load_typed_envelope(db, tenant_id, revision.event_id).await?;
        typed.validate_registered_schema()?;
        if typed.id() == revision.event_id
            || typed.causation_id() != Some(revision.event_id)
            || typed.event_type() != TYPED_EVENT_TYPE
            || typed.schema_version() != 1
        {
            return Err(test_error(format!(
                "D15 typed envelope lost transport/root identity: {typed:?}"
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
                    "D15 typed payload does not match owner revision: {payload:?}"
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
                    "D15 typed ingress returned unexpected outcome: {outcome:?}"
                )));
            }
        }
        let inbox = load_inbox_row(db, revision.event_id).await?;
        if inbox.status != "pending" || inbox.ingest_sequence <= 0 {
            return Err(test_error(format!(
                "D15 typed ingress did not create a pending durable inbox row: {inbox:?}"
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
            "D15 typed inbox sequences did not increase: {facts:?}"
        )));
    }
    Ok(facts)
}

#[derive(Debug)]
struct InboxRow {
    ingest_sequence: i64,
    scope_key: String,
    status: String,
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
            "expected one D15 root envelope {event_id}, found {}",
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
            "expected one D15 typed envelope caused by {root_event_id}, found {}",
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

async fn load_inbox_row(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxRow> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT ingest_sequence, scope_key, status
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("D15 Search inbox row {event_id} was not found")))?;
    Ok(InboxRow {
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        scope_key: row.try_get("", "scope_key")?,
        status: row.try_get("", "status")?,
    })
}

async fn load_forum_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Vec<SearchDocumentRow>> {
    db.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT document_id, entity_type, locale, status, title, body
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
        })
    })
    .collect::<Result<Vec<_>, sea_orm::DbErr>>()
    .map_err(Into::into)
}

fn ensure_legitimate_projection(
    documents: &[SearchDocumentRow],
    fixture: ForumFixture,
) -> TestResult<()> {
    if documents.len() != 2 {
        return Err(test_error(format!(
            "D15 legitimate projection contains {} documents instead of two: {documents:?}",
            documents.len()
        )));
    }
    let category = documents
        .iter()
        .find(|document| {
            document.document_id == fixture.public_category_id
                && document.entity_type == "forum_category"
                && document.locale == "en"
        })
        .ok_or_else(|| test_error("D15 legitimate projection omitted the public category"))?;
    let topic = documents
        .iter()
        .find(|document| {
            document.document_id == fixture.public_topic_id
                && document.entity_type == "forum_topic"
                && document.locale == "en"
        })
        .ok_or_else(|| test_error("D15 legitimate projection omitted the public topic"))?;
    if category.status != "public"
        || topic.status != "open"
        || !topic.title.contains(PUBLIC_TOPIC_MARKER)
        || !topic.body.contains(PUBLIC_TOPIC_MARKER)
        || documents.iter().any(|document| {
            document.document_id == fixture.private_topic_id
                || document.document_id == fixture.trusted_category_id
                || document.document_id == fixture.trusted_topic_id
        })
    {
        return Err(test_error(format!(
            "D15 legitimate projection retained private or trusted content: {documents:?}"
        )));
    }
    Ok(())
}

async fn insert_stale_topic_documents(
    db: &DatabaseConnection,
    fixture: ForumFixture,
) -> TestResult<()> {
    for (document_id, category_id, marker, slug, channel_slugs, owner_state) in [
        (
            fixture.private_topic_id,
            fixture.public_category_id,
            PRIVATE_TOPIC_MARKER,
            "d15-private-topic",
            Vec::<String>::new(),
            "stale_private",
        ),
        (
            fixture.trusted_topic_id,
            fixture.trusted_category_id,
            TRUSTED_TOPIC_MARKER,
            "d15-trusted-topic",
            vec![TRUSTED_CHANNEL.to_string()],
            "stale_trusted",
        ),
    ] {
        let document_key = format!("forum_topic:{document_id}:en");
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_documents (
                document_key, tenant_id, document_id, source_module, entity_type,
                locale, status, is_public, title, subtitle, slug, handle, body,
                keywords_text, facets, payload, published_at, created_at, updated_at, indexed_at
            ) VALUES (
                $1, $2, $3, 'forum', 'forum_topic', 'en', 'open', TRUE,
                $4, NULL, $5, NULL, $6, $7, $8, $9,
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            "#,
            vec![
                document_key.into(),
                fixture.tenant_id.into(),
                document_id.into(),
                format!("Intentionally stale D15 {marker}").into(),
                slug.to_string().into(),
                format!("Intentionally stale D15 body {marker}").into(),
                marker.to_string().into(),
                json!({
                    "kind": "forum_topic",
                    "category_id": category_id,
                    "has_channels": !channel_slugs.is_empty(),
                    "channel_slugs": channel_slugs.clone()
                })
                .into(),
                json!({
                    "topic_id": document_id,
                    "category_id": category_id,
                    "channel_slugs": channel_slugs,
                    "owner_state": owner_state,
                    "route": format!("/modules/forum?topic={document_id}")
                })
                .into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn assert_storefront_exact(
    db: &DatabaseConnection,
    category_scope: SharedStorefrontSearchCategoryScopePort,
    eligibility: SharedStorefrontSearchResultEligibilityPort,
    fixture: ForumFixture,
    category_id: Uuid,
    channel_slug: Option<&str>,
    marker: &str,
    expected_id: Uuid,
    expected_total: u64,
    label: &'static str,
) -> TestResult<StorefrontFact> {
    let channel_id = match channel_slug {
        Some(TRUSTED_CHANNEL) => Some(fixture.trusted_channel_id),
        Some(WRONG_CHANNEL) => Some(fixture.wrong_channel_id),
        Some(other) => {
            return Err(test_error(format!(
                "D15 storefront `{label}` requested unknown route channel `{other}`"
            )));
        }
        None => None,
    };
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
            entity_types: vec!["forum_topic".to_string()],
            source_modules: vec!["forum".to_string()],
            statuses: vec!["open".to_string()],
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
                channel_id,
                channel_slug: channel_slug.map(str::to_string),
                channel_resolution_source: None,
                locale: "en".to_string(),
            }),
            transport: StorefrontSearchTransport::Graphql,
        },
    )
    .await?;
    let visible_facet_buckets = execution
        .result
        .facets
        .iter()
        .map(|facet| facet.buckets.len())
        .sum::<usize>();
    let item_ids = execution
        .result
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    if execution.result.total != expected_total {
        return Err(test_error(format!(
            "D15 storefront `{label}` returned total {}, expected {expected_total}",
            execution.result.total
        )));
    }
    if expected_total == 1 {
        if item_ids.len() != 1 || item_ids[0] != expected_id {
            return Err(test_error(format!(
                "D15 storefront `{label}` did not return exact owner object {expected_id}: {item_ids:?}"
            )));
        }
    } else if !item_ids.is_empty() || visible_facet_buckets != 0 {
        return Err(test_error(format!(
            "D15 denied storefront `{label}` leaked items or visible facets"
        )));
    }
    Ok(StorefrontFact {
        label,
        expected_total,
        actual_total: execution.result.total,
        item_ids,
        visible_facet_buckets,
    })
}

fn ensure_no_fact_requests(
    observed: &Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
    label: &str,
) -> TestResult<()> {
    let requests = observed_requests(observed)?;
    if !requests.is_empty() {
        return Err(test_error(format!(
            "D15 {label} unexpectedly called owner facts: {requests:?}"
        )));
    }
    Ok(())
}

fn ensure_fact_request_shape(
    observed: &Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
    expected: &[(bool, Vec<String>)],
    label: &str,
) -> TestResult<()> {
    let requests = observed_requests(observed)?;
    if requests.len() != expected.len() {
        return Err(test_error(format!(
            "D15 {label} expected {} facts requests, received {requests:?}",
            expected.len()
        )));
    }
    for (request, (include_trust_level, channel_slugs)) in requests.iter().zip(expected) {
        if request.include_trust_level != *include_trust_level
            || request.channel_slugs != *channel_slugs
            || !request.group_ids.is_empty()
        {
            return Err(test_error(format!(
                "D15 {label} facts request scope drifted: {requests:?}"
            )));
        }
    }
    Ok(())
}

fn observed_requests(
    observed: &Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
) -> TestResult<Vec<ForumAudienceFactsRequest>> {
    observed
        .lock()
        .map_err(|_| test_error("D15 audience observation lock was poisoned"))
        .map(|requests| requests.clone())
}

async fn count_stale_documents(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS value
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'forum'
              AND payload ->> 'owner_state' IN ('stale_private', 'stale_trusted')
            "#,
            vec![tenant_id.into()],
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

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row = db
        .query_one_raw(statement)
        .await?
        .ok_or_else(|| test_error("D15 scalar query returned no row"))?;
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

fn write_evidence(artifact: PrivateTrustedEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| test_error("D15 evidence path has no parent directory"))?;
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
            "git rev-parse HEAD failed for D15 evidence generation",
        ));
    }
    let value = String::from_utf8(output.stdout)?;
    let value = value.trim();
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(test_error(
            "git rev-parse HEAD returned an invalid D15 commit SHA",
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
