use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, ExternalConfig, IggyConfig, IggyMode,
    IggyTransport, PersistentContractConsumerGroup, PersistentContractDelivery,
    SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceipt,
    ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
};
use rustok_iggy_connector::{
    ConnectorConfig, ConsumerCursor, ExternalConnector, IggyConnector, PublishRequest,
    SubscriberMessage,
};
use rustok_search::{FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_raw_poison_evidence_v1";
const EVIDENCE_PATH: &str = "target/forum-search-versioned-invalidation-raw-poison-evidence.json";

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
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search raw poison proof"
            );
            return Ok(None);
        };
        let Some(config) = external_iggy_config(scope)? else {
            eprintln!("{IGGY_ADDRESS_ENV} is not set; skipping Forum Search raw poison proof");
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_forum_poison_{}_{}",
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

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct RawPoisonEvidenceArtifact {
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
async fn raw_poison_receipt_prevents_duplicate_dlq_after_restart() -> TestResult<()> {
    let Some(evidence) = PostgresIggyEvidence::setup("raw_poison").await? else {
        return Ok(());
    };

    let stream = evidence.config.topology.stream_name.clone();
    let proof = run_raw_poison_proof(&evidence).await;
    let cleanup = evidence.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(RawPoisonEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D4",
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

async fn run_raw_poison_proof(evidence: &PostgresIggyEvidence) -> TestResult<ScenarioEvidence> {
    let stream = evidence.config.topology.stream_name.clone();
    let dlq_group = unique_name("dlq-observer");
    let first_payload = vec![0xff, 0x00, 0x46, 0x04, 0x01];
    let second_payload = vec![0xff, 0x00, 0x46, 0x04, 0x02];

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

    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_group).await?;
    ensure_decode_failure(&first_failure, &first_payload)?;
    let first_offset = first_failure.offset();
    let first_delivery_id = first_failure.delivery_id();
    let first_dlq_entry = first_failure.to_dlq_entry(1);
    if first_dlq_entry.event_id != first_delivery_id
        || first_dlq_entry.broker_message_id() != Some(first_delivery_id)
        || first_dlq_entry.payload != first_payload
    {
        return Err(invalid_data(
            "raw poison DLQ entry did not retain the deterministic connector delivery identity and exact bytes",
        )
        .into());
    }

    let identity = poison_identity(&first_failure)?;
    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let first_publisher_id = Uuid::new_v4();
    let first_claim = store
        .reserve_and_claim(
            &identity,
            first_failure.stable_error_code(),
            1,
            first_publisher_id,
            PUBLISH_LEASE,
        )
        .await?;
    if first_claim != ConsumerPoisonPublishClaim::Claimed {
        return Err(invalid_data(format!(
            "first raw poison publication did not acquire the durable claim: {first_claim:?}"
        ))
        .into());
    }
    let publishing_receipt = require_receipt(&store, &identity).await?;
    ensure_receipt(
        &publishing_receipt,
        ConsumerPoisonReceiptState::Publishing,
        first_failure.stable_error_code(),
        1,
    )?;

    first_transport.move_to_dlq(first_dlq_entry).await?;
    let physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    if physical_dlq.payload != first_payload {
        return Err(invalid_data(
            "physical Iggy DLQ delivery did not preserve the exact raw poison bytes",
        )
        .into());
    }
    acknowledge_cursor_message(&mut dlq_cursor, &physical_dlq).await?;

    let before_publish_mark = require_receipt(&store, &identity).await?;
    ensure_receipt(
        &before_publish_mark,
        ConsumerPoisonReceiptState::Publishing,
        first_failure.stable_error_code(),
        1,
    )?;
    store.mark_published(&identity, first_publisher_id).await?;
    let published_receipt = require_receipt(&store, &identity).await?;
    ensure_receipt(
        &published_receipt,
        ConsumerPoisonReceiptState::Published,
        first_failure.stable_error_code(),
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
        let redelivered = receive_decode_failure(&restarted_group).await?;
        ensure_decode_failure(&redelivered, &first_payload)?;
        if redelivered.offset() != first_offset
            || redelivered.delivery_id() != first_delivery_id
        {
            return Err(invalid_data(format!(
                "raw poison restart identity changed: first offset/id={first_offset}/{first_delivery_id}, redelivered={}/{}",
                redelivered.offset(),
                redelivered.delivery_id()
            ))
            .into());
        }

        let redelivered_identity = poison_identity(&redelivered)?;
        let restart_claim = store
            .reserve_and_claim(
                &redelivered_identity,
                redelivered.stable_error_code(),
                9,
                Uuid::new_v4(),
                PUBLISH_LEASE,
            )
            .await?;
        if restart_claim != ConsumerPoisonPublishClaim::AlreadyPublished {
            return Err(invalid_data(format!(
                "redelivery did not reuse the durable published receipt: {restart_claim:?}"
            ))
            .into());
        }
        let reused_receipt = require_receipt(&store, &identity).await?;
        ensure_receipt(
            &reused_receipt,
            ConsumerPoisonReceiptState::Published,
            redelivered.stable_error_code(),
            1,
        )?;

        assert_no_duplicate_dlq_message(&mut dlq_cursor).await?;

        restarted_group
            .acknowledge_decode_failure(&redelivered)
            .await?;
        store.mark_acknowledged(&identity).await?;
        let acknowledged_receipt = require_receipt(&store, &identity).await?;
        ensure_receipt(
            &acknowledged_receipt,
            ConsumerPoisonReceiptState::Acknowledged,
            redelivered.stable_error_code(),
            1,
        )?;

        let next_failure = receive_decode_failure(&restarted_group).await?;
        ensure_decode_failure(&next_failure, &second_payload)?;
        if next_failure.offset() <= first_offset
            || next_failure.delivery_id() == first_delivery_id
        {
            return Err(invalid_data(format!(
                "source acknowledgement did not advance to the next raw delivery: first={first_offset}/{first_delivery_id}, next={}/{}",
                next_failure.offset(),
                next_failure.delivery_id()
            ))
            .into());
        }

        Ok::<ScenarioEvidence, Box<dyn Error + Send + Sync>>(ScenarioEvidence {
            id: "raw_poison_dlq_redelivery",
            result: "passed",
            facts: json!({
                "consumer_group": FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
                "stream": stream,
                "topic": FORUM_SEARCH_CONTRACT_TOPIC,
                "stable_error_code": redelivered.stable_error_code(),
                "delivery_id": first_delivery_id,
                "dlq_broker_message_id": first_delivery_id,
                "first_offset": first_offset,
                "redelivered_offset": redelivered.offset(),
                "exact_payload_length": first_payload.len(),
                "receipt_state_before_restart": "published",
                "restart_claim": "already_published",
                "first_delivery_attempt_count_after_restart": reused_receipt.first_delivery_attempt_count,
                "duplicate_dlq_message_observed": false,
                "receipt_state_after_source_acknowledgement": "acknowledged",
                "next_offset": next_failure.offset(),
                "next_delivery_id": next_failure.delivery_id(),
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

async fn publish_fixture(
    connector: &ExternalConnector,
    stream: &str,
    payload: Vec<u8>,
) -> TestResult<()> {
    connector
        .publish(PublishRequest::new(
            stream,
            FORUM_SEARCH_CONTRACT_TOPIC,
            "forum-search-raw-poison-fixture",
            payload,
            Uuid::new_v4().to_string(),
        ))
        .await?;
    Ok(())
}

async fn receive_decode_failure(
    group: &PersistentContractConsumerGroup,
) -> TestResult<ConsumedContractDecodeFailure> {
    let delivery = timeout(RECEIVE_TIMEOUT, group.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for a Forum Search raw poison delivery"))??
        .ok_or_else(|| invalid_data("Forum Search source cursor ended before a delivery"))?;
    match delivery {
        PersistentContractDelivery::DecodeFailure(failure) => Ok(*failure),
        PersistentContractDelivery::Event(_) => Err(invalid_data(
            "malformed Forum Search fixture unexpectedly decoded as a registered contract event",
        )
        .into()),
    }
}

fn ensure_decode_failure(
    failure: &ConsumedContractDecodeFailure,
    expected_payload: &[u8],
) -> TestResult<()> {
    if failure.kind() != ContractDecodeFailureKind::Deserialize
        || failure.stable_error_code() != "iggy.contract.decode_invalid"
        || failure.stream().is_empty()
        || failure.topic() != FORUM_SEARCH_CONTRACT_TOPIC
        || failure.partition() == 0
        || failure.ack_token().is_none()
        || failure.raw_payload() != expected_payload
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search raw poison delivery: {failure:?}"
        ))
        .into());
    }
    failure.validate_connector_metadata()?;
    Ok(())
}

fn poison_identity(
    failure: &ConsumedContractDecodeFailure,
) -> Result<ConsumerPoisonIdentity, rustok_iggy_connector::migrations::ConsumerPoisonReceiptError> {
    ConsumerPoisonIdentity::new(
        failure.delivery_id(),
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        failure.stream(),
        failure.topic(),
        failure.partition(),
        failure.offset(),
        failure.raw_payload().to_vec(),
    )
}

async fn require_receipt(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
) -> TestResult<ConsumerPoisonReceipt> {
    store
        .find(identity)
        .await?
        .ok_or_else(|| invalid_data("expected Forum Search poison receipt was not found").into())
}

fn ensure_receipt(
    receipt: &ConsumerPoisonReceipt,
    expected_state: ConsumerPoisonReceiptState,
    expected_error_code: &str,
    expected_first_attempt_count: u32,
) -> TestResult<()> {
    if receipt.state != expected_state
        || receipt.stable_error_code != expected_error_code
        || receipt.first_delivery_attempt_count != expected_first_attempt_count
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search poison receipt: {receipt:?}"
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
        .map_err(|_| invalid_data("timed out waiting for the Forum Search DLQ delivery"))??
        .ok_or_else(|| invalid_data("Forum Search DLQ cursor ended before a message").into())
}

async fn acknowledge_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
    message: &SubscriberMessage,
) -> TestResult<()> {
    let ack_token =
        message.metadata.ack_token.as_deref().ok_or_else(|| {
            invalid_data("Forum Search DLQ delivery has no acknowledgement token")
        })?;
    cursor.acknowledge(ack_token).await?;
    Ok(())
}

async fn assert_no_duplicate_dlq_message(cursor: &mut Box<dyn ConsumerCursor>) -> TestResult<()> {
    match timeout(NO_DUPLICATE_DLQ_TIMEOUT, cursor.receive()).await {
        Err(_) | Ok(Ok(None)) => Ok(()),
        Ok(Ok(Some(_))) => Err(invalid_data(
            "published raw poison redelivery unexpectedly produced a second DLQ message",
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

fn write_evidence(artifact: RawPoisonEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(&artifact)?;
    fs::write(path, bytes)?;
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
