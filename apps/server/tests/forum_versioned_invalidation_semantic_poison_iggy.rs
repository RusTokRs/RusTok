use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use rustok_core::{MigrationSource, events::EventTransport};
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ConsumedContractEvent, ContractDecodeFailureKind, DlqEntry,
    ExternalConfig, IggyConfig, IggyMode, IggyTransport, PersistentContractConsumerGroup,
    PersistentContractDelivery, SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceipt,
    ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
};
use rustok_iggy_connector::{
    ConnectorConfig, ConsumerCursor, ExternalConnector, IggyConnector, SubscriberMessage,
};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC, ForumSearchContractIngress,
    ForumSearchContractIngressError, ForumSearchContractIngressOutcome, SearchModule,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use tokio::time::timeout;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const IGGY_ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";
const IGGY_USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const IGGY_PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const NO_DUPLICATE_DLQ_TIMEOUT: Duration = Duration::from_millis(750);
const PUBLISH_LEASE: Duration = Duration::from_secs(30);
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const SEMANTIC_ERROR_CODE: &str = "forum.search_projection.contract_inbox_identity_conflict";
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_semantic_poison_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-semantic-poison-evidence.json";

struct PostgresIggyEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
    config: IggyConfig,
    fixture: ExternalConnector,
}

impl PostgresIggyEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search semantic poison proof"
            );
            return Ok(None);
        };
        let Some(config) = external_iggy_config(scope)? else {
            eprintln!("{IGGY_ADDRESS_ENV} is not set; skipping Forum Search semantic poison proof");
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_forum_semantic_poison_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;
        let migration_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in rustok_iggy_connector::migrations::migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;
        if let Err(error) = migration_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        let fixture = ExternalConnector::new();
        if let Err(error) = fixture.connect(&ConnectorConfig::from(&config)).await {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
            config,
            fixture,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.fixture.shutdown().await?;
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
struct SemanticPoisonEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_backend: &'static str,
    delivery_profile: &'static str,
    consumer_group: &'static str,
    stream: String,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn semantic_identity_conflict_reuses_durable_poison_result_after_restart() -> TestResult<()> {
    let Some(evidence) = PostgresIggyEvidence::setup("identity_conflict").await? else {
        return Ok(());
    };

    let stream = evidence.config.topology.stream_name.clone();
    let proof = run_semantic_poison_proof(&evidence).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(SemanticPoisonEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D5",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_backend: "external_iggy",
        delivery_profile: "outbox_iggy",
        consumer_group: FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        stream,
        topic: FORUM_SEARCH_CONTRACT_TOPIC,
        scenario_results: vec![scenario],
    })?;

    Ok(())
}

async fn run_semantic_poison_proof(
    evidence: &PostgresIggyEvidence,
) -> TestResult<ScenarioEvidence> {
    let expected_tenant_id = Uuid::new_v4();
    let conflicting_tenant_id = Uuid::new_v4();
    let conflict_root_id = Uuid::new_v4();
    let conflict_category_id = Uuid::new_v4();
    let next_root_id = Uuid::new_v4();

    let conflicting_root = root_envelope(conflicting_tenant_id, conflict_root_id, "forum", None);
    insert_legacy_root(&evidence.db, &conflicting_root, "forum").await?;
    let conflict_before = load_snapshot(&evidence.db, conflict_root_id).await?;

    let first_envelope = typed_invalidation(
        expected_tenant_id,
        conflict_root_id,
        21,
        "forum_category",
        Some(conflict_category_id),
    )?;
    let next_envelope = typed_invalidation(expected_tenant_id, next_root_id, 22, "forum", None)?;
    let first_typed_envelope_id = first_envelope.id();
    let next_typed_envelope_id = next_envelope.id();

    let stream = evidence.config.topology.stream_name.clone();
    let dlq_group = unique_name("semantic-dlq-observer");
    let first_transport = IggyTransport::new(evidence.config.clone()).await?;
    let first_group = first_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;
    let mut dlq_cursor = evidence
        .fixture
        .open_consumer_group(&stream, "dlq", &dlq_group)
        .await?;

    first_transport.publish_contract(first_envelope).await?;
    first_transport.publish_contract(next_envelope).await?;

    let first_delivery = receive_event(&first_group).await?;
    ensure_event_identity(
        &first_delivery,
        first_typed_envelope_id,
        conflict_root_id,
        expected_tenant_id,
    )?;
    let first_offset = first_delivery
        .offset()
        .ok_or_else(|| invalid_data("semantic poison delivery has no broker offset"))?;
    let first_raw_payload = first_delivery.raw_payload().to_vec();

    let ingress = ForumSearchContractIngress::new(evidence.db.clone());
    let first_error = ingress
        .ingest(&first_delivery.envelope)
        .await
        .expect_err("conflicting durable root identity must be semantic poison");
    ensure_identity_conflict(&first_error)?;
    let conflict_after_first = load_snapshot(&evidence.db, conflict_root_id).await?;
    if conflict_after_first != conflict_before
        || count_event_rows(&evidence.db, conflict_root_id).await? != 1
    {
        return Err(invalid_data(
            "semantic poison replaced or duplicated the conflicting Search inbox row",
        )
        .into());
    }

    let (identity, first_entry) = semantic_poison_descriptor(&first_delivery, 1)?;
    let delivery_id = identity.delivery_id();
    if first_entry.event_id != delivery_id
        || first_entry.broker_message_id() != Some(delivery_id)
        || first_entry.payload != first_raw_payload
        || first_entry.error != SEMANTIC_ERROR_CODE
    {
        return Err(invalid_data(
            "semantic poison DLQ entry did not retain deterministic connector identity, exact bytes, and stable error code",
        )
        .into());
    }

    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let publisher_id = Uuid::new_v4();
    let first_claim = store
        .reserve_and_claim(
            &identity,
            SEMANTIC_ERROR_CODE,
            1,
            publisher_id,
            PUBLISH_LEASE,
        )
        .await?;
    if first_claim != ConsumerPoisonPublishClaim::Claimed {
        return Err(invalid_data(format!(
            "first semantic poison publication did not acquire the durable claim: {first_claim:?}"
        ))
        .into());
    }
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Publishing,
        1,
    )?;

    first_transport.move_to_dlq(first_entry).await?;
    let physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    if physical_dlq.payload != first_raw_payload {
        return Err(invalid_data(
            "physical semantic-poison DLQ delivery changed the exact typed-event bytes",
        )
        .into());
    }
    acknowledge_cursor_message(&mut dlq_cursor, &physical_dlq).await?;
    store.mark_published(&identity, publisher_id).await?;
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Published,
        1,
    )?;

    drop(first_group);
    first_transport.shutdown().await?;
    drop(first_transport);

    let restarted_transport = IggyTransport::new(evidence.config.clone()).await?;
    let restarted_group = restarted_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;

    let restarted_proof = async {
        let redelivered = receive_event(&restarted_group).await?;
        ensure_event_identity(
            &redelivered,
            first_typed_envelope_id,
            conflict_root_id,
            expected_tenant_id,
        )?;
        if redelivered.offset() != Some(first_offset)
            || redelivered.raw_payload() != first_raw_payload.as_slice()
        {
            return Err(invalid_data(
                "semantic poison restart changed the exact source offset or typed-event bytes",
            )
            .into());
        }

        let redelivery_error = ingress
            .ingest(&redelivered.envelope)
            .await
            .expect_err("redelivered conflicting root identity must remain semantic poison");
        ensure_identity_conflict(&redelivery_error)?;
        let conflict_after_restart = load_snapshot(&evidence.db, conflict_root_id).await?;
        if conflict_after_restart != conflict_before
            || count_event_rows(&evidence.db, conflict_root_id).await? != 1
        {
            return Err(invalid_data(
                "semantic poison redelivery replaced or duplicated the durable conflict row",
            )
            .into());
        }

        let (redelivered_identity, redelivered_entry) =
            semantic_poison_descriptor(&redelivered, 9)?;
        if redelivered_identity.delivery_id() != delivery_id
            || redelivered_entry.broker_message_id() != Some(delivery_id)
        {
            return Err(invalid_data(
                "semantic poison redelivery changed the durable connector or DLQ identity",
            )
            .into());
        }
        let restart_claim = store
            .reserve_and_claim(
                &redelivered_identity,
                SEMANTIC_ERROR_CODE,
                9,
                Uuid::new_v4(),
                PUBLISH_LEASE,
            )
            .await?;
        if restart_claim != ConsumerPoisonPublishClaim::AlreadyPublished {
            return Err(invalid_data(format!(
                "semantic poison redelivery did not reuse the published receipt: {restart_claim:?}"
            ))
            .into());
        }
        let reused_receipt = require_receipt(&store, &identity).await?;
        ensure_receipt(&reused_receipt, ConsumerPoisonReceiptState::Published, 1)?;
        assert_no_duplicate_dlq_message(&mut dlq_cursor).await?;

        restarted_group.acknowledge(&redelivered).await?;
        store.mark_acknowledged(&identity).await?;
        ensure_receipt(
            &require_receipt(&store, &identity).await?,
            ConsumerPoisonReceiptState::Acknowledged,
            1,
        )?;

        let next_delivery = receive_event(&restarted_group).await?;
        ensure_event_identity(
            &next_delivery,
            next_typed_envelope_id,
            next_root_id,
            expected_tenant_id,
        )?;
        let next_offset = next_delivery
            .offset()
            .ok_or_else(|| invalid_data("next typed delivery has no broker offset"))?;
        if next_offset <= first_offset {
            return Err(invalid_data(format!(
                "semantic poison source acknowledgement did not advance the group: first={first_offset}, next={next_offset}"
            ))
            .into());
        }
        match ingress.ingest(&next_delivery.envelope).await? {
            ForumSearchContractIngressOutcome::DurablyAccepted {
                root_event_id,
                owner_revision,
            } if root_event_id == next_root_id && owner_revision == 22 => {}
            other => {
                return Err(invalid_data(format!(
                    "next valid Forum Search event did not reach the durable inbox: {other:?}"
                ))
                .into());
            }
        }
        restarted_group.acknowledge(&next_delivery).await?;

        Ok::<ScenarioEvidence, Box<dyn Error + Send + Sync>>(ScenarioEvidence {
            id: "semantic_poison_identity_conflict",
            result: "passed",
            facts: json!({
                "consumer_group": FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
                "stream": stream,
                "topic": FORUM_SEARCH_CONTRACT_TOPIC,
                "stable_error_code": SEMANTIC_ERROR_CODE,
                "expected_tenant_id": expected_tenant_id,
                "durable_conflicting_tenant_id": conflicting_tenant_id,
                "root_event_id": conflict_root_id,
                "typed_envelope_id": first_typed_envelope_id,
                "connector_delivery_id": delivery_id,
                "dlq_broker_message_id": delivery_id,
                "first_offset": first_offset,
                "redelivered_offset": redelivered.offset(),
                "durable_conflict_row_preserved": true,
                "conflict_inbox_rows": 1,
                "receipt_state_before_restart": "published",
                "restart_claim": "already_published",
                "first_delivery_attempt_count_after_restart": reused_receipt.first_delivery_attempt_count,
                "duplicate_dlq_message_observed": false,
                "receipt_state_after_source_acknowledgement": "acknowledged",
                "next_root_event_id": next_root_id,
                "next_typed_envelope_id": next_typed_envelope_id,
                "next_offset": next_offset,
                "source_acknowledgement_advanced_group": true
            }),
        })
    }
    .await;

    drop(restarted_group);
    let shutdown = restarted_transport.shutdown().await;
    let scenario = restarted_proof?;
    shutdown?;
    Ok(scenario)
}

fn semantic_poison_descriptor(
    consumed: &ConsumedContractEvent,
    observed_attempts: u32,
) -> TestResult<(ConsumerPoisonIdentity, DlqEntry)> {
    let offset = consumed.offset().ok_or_else(|| {
        invalid_data("validated semantic poison delivery has no connector offset")
    })?;
    let delivery_identity = ConsumedContractDecodeFailure::new(
        consumed.stream.clone(),
        consumed.topic.clone(),
        consumed.connector_metadata.clone(),
        consumed.raw_payload().to_vec(),
        ContractDecodeFailureKind::SchemaValidation,
    )?;
    let delivery_id = delivery_identity.delivery_id();
    let identity = ConsumerPoisonIdentity::new(
        delivery_id,
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        &consumed.stream,
        &consumed.topic,
        consumed.partition,
        offset,
        consumed.raw_payload().to_vec(),
    )?;
    let entry = DlqEntry::new(
        delivery_id,
        consumed.topic.clone(),
        consumed.raw_payload().to_vec(),
        SEMANTIC_ERROR_CODE,
        observed_attempts,
    )
    .with_connector_metadata(consumed.connector_metadata.clone())
    .with_broker_message_id(delivery_id);
    Ok((identity, entry))
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

async fn receive_event(
    group: &PersistentContractConsumerGroup,
) -> TestResult<ConsumedContractEvent> {
    let delivery = timeout(RECEIVE_TIMEOUT, group.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for a Forum Search typed delivery"))??
        .ok_or_else(|| invalid_data("Forum Search source cursor ended before a delivery"))?;
    match delivery {
        PersistentContractDelivery::Event(consumed) => Ok(*consumed),
        PersistentContractDelivery::DecodeFailure(failure) => Err(invalid_data(format!(
            "valid Forum Search semantic-poison fixture decoded as raw poison: {}",
            failure.stable_error_code()
        ))
        .into()),
    }
}

fn ensure_event_identity(
    delivery: &ConsumedContractEvent,
    expected_typed_envelope_id: Uuid,
    expected_root_event_id: Uuid,
    expected_tenant_id: Uuid,
) -> TestResult<()> {
    if delivery.envelope.id() != expected_typed_envelope_id
        || delivery.envelope.causation_id() != Some(expected_root_event_id)
        || delivery.envelope.tenant_id() != expected_tenant_id
        || delivery.envelope.event_type() != "forum.search_projection.invalidation_issued"
        || delivery.offset().is_none()
        || delivery.ack_token().is_none()
        || delivery.raw_payload().is_empty()
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search typed delivery identity: {delivery:?}"
        ))
        .into());
    }
    delivery.validate_connector_metadata()?;
    delivery.envelope.validate_registered_schema()?;
    Ok(())
}

fn ensure_identity_conflict(error: &ForumSearchContractIngressError) -> TestResult<()> {
    if !matches!(
        error,
        ForumSearchContractIngressError::InboxIdentityConflict
    ) || error.stable_code() != SEMANTIC_ERROR_CODE
        || error.is_retryable()
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search semantic conflict classification: {error:?}"
        ))
        .into());
    }
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
        .ok_or_else(|| invalid_data(format!("Search inbox row {event_id} was not found")))?;
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
        .ok_or_else(|| invalid_data("Search inbox count query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn require_receipt(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
) -> TestResult<ConsumerPoisonReceipt> {
    store.find(identity).await?.ok_or_else(|| {
        invalid_data("expected Forum Search semantic poison receipt was not found").into()
    })
}

fn ensure_receipt(
    receipt: &ConsumerPoisonReceipt,
    expected_state: ConsumerPoisonReceiptState,
    expected_first_attempt_count: u32,
) -> TestResult<()> {
    if receipt.state != expected_state
        || receipt.stable_error_code != SEMANTIC_ERROR_CODE
        || receipt.first_delivery_attempt_count != expected_first_attempt_count
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search semantic poison receipt: {receipt:?}"
        ))
        .into());
    }
    Ok(())
}

async fn receive_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
) -> TestResult<SubscriberMessage> {
    timeout(RECEIVE_TIMEOUT, cursor.receive())
        .await
        .map_err(|_| invalid_data("timed out waiting for the semantic-poison DLQ delivery"))??
        .ok_or_else(|| invalid_data("semantic-poison DLQ cursor ended before a message").into())
}

async fn acknowledge_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
    message: &SubscriberMessage,
) -> TestResult<()> {
    let ack_token =
        message.metadata.ack_token.as_deref().ok_or_else(|| {
            invalid_data("semantic-poison DLQ delivery has no acknowledgement token")
        })?;
    cursor.acknowledge(ack_token).await?;
    Ok(())
}

async fn assert_no_duplicate_dlq_message(cursor: &mut Box<dyn ConsumerCursor>) -> TestResult<()> {
    match timeout(NO_DUPLICATE_DLQ_TIMEOUT, cursor.receive()).await {
        Err(_) | Ok(Ok(None)) => Ok(()),
        Ok(Ok(Some(_))) => Err(invalid_data(
            "published semantic poison redelivery unexpectedly produced a second DLQ message",
        )
        .into()),
        Ok(Err(error)) => Err(error.into()),
    }
}

fn postgres_database_url() -> Option<String> {
    env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

fn external_iggy_config(scope: &str) -> TestResult<Option<IggyConfig>> {
    let address = match env::var(IGGY_ADDRESS_ENV) {
        Ok(value) => bounded_env(IGGY_ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(
            "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS must be host:port without credentials or query parameters",
        )
        .into());
    }

    let username = optional_bounded_env(IGGY_USERNAME_ENV, 191)?;
    let password = optional_bounded_env(IGGY_PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data(
            "external Iggy evidence username and password must both be set or both be empty",
        )
        .into());
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
            stream_name: unique_name(scope),
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
        return Err(invalid_data(format!(
            "{name} must be non-empty and have no surrounding whitespace"
        )));
    }
    if value.len() > max_len {
        return Err(invalid_data(format!("{name} exceeds the evidence limit")));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_data(format!(
            "{name} must not contain control characters"
        )));
    }
    Ok(value)
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

fn unique_name(scope: &str) -> String {
    format!("rustok-forum-search-{scope}-{}", Uuid::new_v4().simple())
}

fn write_evidence(artifact: SemanticPoisonEvidenceArtifact) -> TestResult<()> {
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
