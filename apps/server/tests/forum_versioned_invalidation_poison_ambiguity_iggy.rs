use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, ExternalConfig, IggyConfig,
    IggyMode, IggyTransport, PersistentContractConsumerGroup, PersistentContractDelivery,
    SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceipt,
    ConsumerPoisonReceiptError, ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
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

const DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const DEDUP_ENABLED_ADDRESS_ENV: &str =
    "RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS";
const DEDUP_DISABLED_ADDRESS_ENV: &str =
    "RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const NO_ADDITIONAL_DLQ_TIMEOUT: Duration = Duration::from_millis(750);
const PUBLISH_LEASE: Duration = Duration::from_secs(1);
const LEASE_RECLAIM_WAIT: Duration = Duration::from_millis(1_500);
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_poison_ambiguity_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json";

struct EvidenceRuntime {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
    config: IggyConfig,
    fixture: ExternalConnector,
}

impl EvidenceRuntime {
    async fn setup(database_url: &str, config: IggyConfig, scope: &str) -> TestResult<Self> {
        let control = connect(database_url).await?;
        let schema_name = format!(
            "rustok_forum_poison_ambiguity_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(database_url).await?;
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

        Ok(Self {
            control,
            db,
            schema_name,
            config,
            fixture,
        })
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
    id: String,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct EvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_backend: &'static str,
    delivery_profile: &'static str,
    consumer_group: &'static str,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn raw_poison_publish_mark_ambiguity_obeys_configured_dedup_modes() -> TestResult<()> {
    ensure_distinct_mode_addresses()?;
    let Some(database_url) = postgres_database_url() else {
        eprintln!(
            "{DATABASE_ENV} or PostgreSQL DATABASE_URL is not set; skipping Forum Search poison ambiguity proof"
        );
        return Ok(());
    };
    let Some(dedup_enabled) = external_iggy_config(DEDUP_ENABLED_ADDRESS_ENV, "dedup-enabled")?
    else {
        eprintln!("{DEDUP_ENABLED_ADDRESS_ENV} is not set; skipping poison ambiguity proof");
        return Ok(());
    };
    let Some(dedup_disabled) =
        external_iggy_config(DEDUP_DISABLED_ADDRESS_ENV, "dedup-disabled")?
    else {
        eprintln!("{DEDUP_DISABLED_ADDRESS_ENV} is not set; skipping poison ambiguity proof");
        return Ok(());
    };

    let enabled = exercise_mode(&database_url, dedup_enabled, "dedup_enabled", 1).await?;
    let disabled = exercise_mode(&database_url, dedup_disabled, "dedup_disabled", 2).await?;

    write_evidence(EvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D6",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_backend: "external_iggy",
        delivery_profile: "outbox_iggy",
        consumer_group: FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        topic: FORUM_SEARCH_CONTRACT_TOPIC,
        scenario_results: vec![enabled, disabled],
    })?;
    Ok(())
}

async fn exercise_mode(
    database_url: &str,
    config: IggyConfig,
    scope: &str,
    expected_physical_dlq_messages: u32,
) -> TestResult<ScenarioEvidence> {
    let evidence = EvidenceRuntime::setup(database_url, config, scope).await?;
    let stream = evidence.config.topology.stream_name.clone();
    let dlq_group = unique_name(&format!("{scope}-dlq-observer"));
    let marker = if expected_physical_dlq_messages == 1 {
        0x61
    } else {
        0x62
    };
    let first_payload = vec![0xff, 0x00, marker, 0x01];
    let second_payload = vec![0xff, 0x00, marker, 0x02];

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
    assert_no_additional_dlq_message(&mut dlq_cursor).await?;

    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_group).await?;
    ensure_decode_failure(&first_failure, &first_payload)?;
    let first_offset = first_failure.offset();
    let first_delivery_id = first_failure.delivery_id();
    let identity = poison_identity(&first_failure)?;
    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let first_publisher = Uuid::new_v4();
    let recovery_publisher = Uuid::new_v4();

    let first_claim = store
        .reserve_and_claim(
            &identity,
            first_failure.stable_error_code(),
            1,
            first_publisher,
            PUBLISH_LEASE,
        )
        .await?;
    if first_claim != ConsumerPoisonPublishClaim::Claimed {
        return Err(invalid_data(format!(
            "first poison publisher did not acquire the durable claim: {first_claim:?}"
        ))
        .into());
    }
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Publishing,
        first_failure.stable_error_code(),
        1,
    )?;

    let first_entry = first_failure.to_dlq_entry(1);
    let deterministic_message_id = first_entry
        .broker_message_id()
        .ok_or_else(|| invalid_data("raw poison DLQ entry has no deterministic broker message ID"))?;
    if deterministic_message_id != first_delivery_id || first_entry.payload != first_payload {
        return Err(invalid_data(
            "first DLQ entry did not retain the deterministic delivery identity and exact bytes",
        )
        .into());
    }
    first_transport.move_to_dlq(first_entry).await?;
    let first_physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    ensure_dlq_payload(&first_physical_dlq, &first_payload)?;
    acknowledge_cursor_message(&mut dlq_cursor, &first_physical_dlq).await?;
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Publishing,
        first_failure.stable_error_code(),
        1,
    )?;

    let busy_claim = store
        .reserve_and_claim(
            &identity,
            first_failure.stable_error_code(),
            2,
            recovery_publisher,
            PUBLISH_LEASE,
        )
        .await?;
    if busy_claim != ConsumerPoisonPublishClaim::Busy {
        return Err(invalid_data(format!(
            "unexpired poison claim did not remain busy: {busy_claim:?}"
        ))
        .into());
    }

    drop(first_group);
    first_transport.shutdown().await?;
    drop(first_transport);
    tokio::time::sleep(LEASE_RECLAIM_WAIT).await;

    let recovery_transport = IggyTransport::new(evidence.config.clone()).await?;
    let recovery_group = recovery_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;
    let redelivered = receive_decode_failure(&recovery_group).await?;
    ensure_decode_failure(&redelivered, &first_payload)?;
    if redelivered.offset() != first_offset || redelivered.delivery_id() != first_delivery_id {
        return Err(invalid_data("restart changed the unacknowledged poison identity").into());
    }

    let recovery_claim = store
        .reserve_and_claim(
            &poison_identity(&redelivered)?,
            redelivered.stable_error_code(),
            2,
            recovery_publisher,
            PUBLISH_LEASE,
        )
        .await?;
    if recovery_claim != ConsumerPoisonPublishClaim::Claimed {
        return Err(invalid_data(format!(
            "recovery publisher did not reclaim the expired receipt: {recovery_claim:?}"
        ))
        .into());
    }
    if !matches!(
        store.mark_published(&identity, first_publisher).await,
        Err(ConsumerPoisonReceiptError::ClaimLost)
    ) {
        return Err(invalid_data("stale publisher retained authority after lease takeover").into());
    }

    let retry_entry = redelivered.to_dlq_entry(2);
    if retry_entry.broker_message_id() != Some(deterministic_message_id)
        || retry_entry.payload != first_payload
    {
        return Err(invalid_data("redelivery changed the deterministic DLQ identity or bytes").into());
    }
    recovery_transport.move_to_dlq(retry_entry).await?;

    let observed_physical_dlq_messages = if expected_physical_dlq_messages == 1 {
        assert_no_additional_dlq_message(&mut dlq_cursor).await?;
        1
    } else {
        let duplicate = receive_cursor_message(&mut dlq_cursor).await?;
        ensure_dlq_payload(&duplicate, &first_payload)?;
        acknowledge_cursor_message(&mut dlq_cursor, &duplicate).await?;
        assert_no_additional_dlq_message(&mut dlq_cursor).await?;
        2
    };
    if observed_physical_dlq_messages != expected_physical_dlq_messages {
        return Err(invalid_data(format!(
            "{scope} expected {expected_physical_dlq_messages} physical DLQ messages but observed {observed_physical_dlq_messages}"
        ))
        .into());
    }

    store.mark_published(&identity, recovery_publisher).await?;
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Published,
        redelivered.stable_error_code(),
        1,
    )?;
    recovery_group
        .acknowledge_decode_failure(&redelivered)
        .await?;
    store.mark_acknowledged(&identity).await?;
    ensure_receipt(
        &require_receipt(&store, &identity).await?,
        ConsumerPoisonReceiptState::Acknowledged,
        redelivered.stable_error_code(),
        1,
    )?;

    let next_failure = receive_decode_failure(&recovery_group).await?;
    ensure_decode_failure(&next_failure, &second_payload)?;
    if next_failure.offset() <= first_offset || next_failure.delivery_id() == first_delivery_id {
        return Err(invalid_data("terminalization did not advance to the next source delivery").into());
    }
    let next_offset = next_failure.offset();

    drop(recovery_group);
    recovery_transport.shutdown().await?;
    evidence.cleanup().await?;

    Ok(ScenarioEvidence {
        id: format!("raw_poison_publish_mark_ambiguity_{scope}"),
        result: "passed",
        facts: json!({
            "dedup_mode": scope,
            "stream": stream,
            "consumer_group": FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            "topic": FORUM_SEARCH_CONTRACT_TOPIC,
            "first_offset": first_offset,
            "first_delivery_id": first_delivery_id,
            "deterministic_dlq_message_id": deterministic_message_id,
            "publish_succeeded_before_mark_published": true,
            "unexpired_claim_was_busy": true,
            "expired_claim_reclaimed_after_restart": true,
            "stale_publisher_fenced": true,
            "expected_physical_dlq_messages": expected_physical_dlq_messages,
            "observed_physical_dlq_messages": observed_physical_dlq_messages,
            "source_acknowledged_after_durable_published": true,
            "receipt_state_after_acknowledgement": "acknowledged",
            "next_offset": next_offset
        }),
    })
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
            "forum-search-raw-poison-ambiguity-fixture",
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
        .map_err(|_| invalid_data("timed out waiting for Forum Search raw poison delivery"))??
        .ok_or_else(|| invalid_data("Forum Search source cursor ended before poison delivery"))?;
    match delivery {
        PersistentContractDelivery::DecodeFailure(failure) => Ok(failure),
        PersistentContractDelivery::Event(_) => Err(invalid_data(
            "malformed ambiguity fixture unexpectedly decoded as a registered contract event",
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
) -> Result<ConsumerPoisonIdentity, ConsumerPoisonReceiptError> {
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
        .map_err(|_| invalid_data("timed out waiting for Forum Search DLQ delivery"))??
        .ok_or_else(|| invalid_data("Forum Search DLQ cursor ended before a message").into())
}

fn ensure_dlq_payload(message: &SubscriberMessage, expected_payload: &[u8]) -> TestResult<()> {
    if message.payload != expected_payload {
        return Err(invalid_data("physical Iggy DLQ delivery changed the poison bytes").into());
    }
    Ok(())
}

async fn acknowledge_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
    message: &SubscriberMessage,
) -> TestResult<()> {
    let ack_token = message
        .metadata
        .ack_token
        .as_deref()
        .ok_or_else(|| invalid_data("Forum Search DLQ delivery has no acknowledgement token"))?;
    cursor.acknowledge(ack_token).await?;
    Ok(())
}

async fn assert_no_additional_dlq_message(
    cursor: &mut Box<dyn ConsumerCursor>,
) -> TestResult<()> {
    match timeout(NO_ADDITIONAL_DLQ_TIMEOUT, cursor.receive()).await {
        Err(_) | Ok(Ok(None)) => Ok(()),
        Ok(Ok(Some(_))) => Err(invalid_data(
            "Forum Search poison ambiguity observed an unexpected additional DLQ message",
        )
        .into()),
        Ok(Err(error)) => Err(error.into()),
    }
}

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

fn external_iggy_config(address_env: &'static str, scope: &str) -> TestResult<Option<IggyConfig>> {
    let address = match env::var(address_env) {
        Ok(value) => bounded_env(address_env, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_address(address_env, &address)?;
    let username = optional_bounded_env(USERNAME_ENV, 191)?;
    let password = optional_bounded_env(PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data("Iggy username and password must both be set or both be empty").into());
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

fn ensure_distinct_mode_addresses() -> TestResult<()> {
    let enabled = env::var(DEDUP_ENABLED_ADDRESS_ENV).ok();
    let disabled = env::var(DEDUP_DISABLED_ADDRESS_ENV).ok();
    if let (Some(enabled), Some(disabled)) = (enabled, disabled) {
        let enabled = bounded_env(DEDUP_ENABLED_ADDRESS_ENV, enabled, 255)?;
        let disabled = bounded_env(DEDUP_DISABLED_ADDRESS_ENV, disabled, 255)?;
        validate_address(DEDUP_ENABLED_ADDRESS_ENV, &enabled)?;
        validate_address(DEDUP_DISABLED_ADDRESS_ENV, &disabled)?;
        if enabled == disabled {
            return Err(invalid_data(
                "dedup-enabled and dedup-disabled evidence require distinct Iggy addresses",
            )
            .into());
        }
    }
    Ok(())
}

fn validate_address(name: &'static str, address: &str) -> Result<(), IoError> {
    if address.contains("://")
        || address.contains('@')
        || address.contains('?')
        || address.contains('#')
    {
        return Err(invalid_data(format!(
            "{name} must be host:port without credentials or URL delimiters"
        )));
    }
    Ok(())
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

fn write_evidence(artifact: EvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
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
