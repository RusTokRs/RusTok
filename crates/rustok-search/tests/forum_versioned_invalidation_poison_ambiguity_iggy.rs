use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use iggy::prelude::{Client, Identifier, IggyClient, TopicClient};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractConsumerGroup, PersistentContractDelivery, SerializationFormat,
    TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptError,
    ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
};
use rustok_iggy_connector::{ConnectorConfig, ExternalConnector, IggyConnector, PublishRequest};
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
const PUBLISH_LEASE: Duration = Duration::from_secs(1);
const LEASE_RECLAIM_WAIT: Duration = Duration::from_millis(1_500);
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_poison_ambiguity_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json";

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
    delivery_profile: &'static str,
    consumer_group: &'static str,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

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

        let db = connect_in_schema(database_url, &schema_name).await?;
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
    expected_physical_dlq_messages: u64,
) -> TestResult<ScenarioEvidence> {
    let evidence = EvidenceRuntime::setup(database_url, config, scope).await?;
    let stream = evidence.config.topology.stream_name.clone();
    let first_transport = IggyTransport::new(evidence.config.clone()).await?;
    let first_group = first_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;
    let observer = connect_observer(&evidence.config).await?;

    let marker = if expected_physical_dlq_messages == 1 { 0x61 } else { 0x62 };
    let first_payload = vec![0xff, 0x00, marker, 0x01];
    let second_payload = vec![0xff, 0x00, marker, 0x02];
    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_group).await?;
    ensure_failure(&first_failure, &first_payload)?;
    let first_offset = first_failure.offset();
    let first_delivery_id = first_failure.delivery_id();
    let identity = poison_identity(&first_failure)?;
    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let first_publisher = Uuid::new_v4();
    let recovery_publisher = Uuid::new_v4();

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                first_failure.stable_error_code(),
                1,
                first_publisher,
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Publishing).await?;
    assert_message_count(&observer, &stream, 0).await?;

    let first_entry = first_failure.to_dlq_entry(1);
    let deterministic_message_id = first_entry
        .broker_message_id()
        .ok_or_else(|| invalid_data("raw poison DLQ entry has no deterministic message ID"))?;
    if deterministic_message_id != first_delivery_id {
        return Err(invalid_data("DLQ message ID differs from deterministic delivery ID").into());
    }
    first_transport.move_to_dlq(first_entry).await?;
    assert_message_count(&observer, &stream, 1).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Publishing).await?;

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                first_failure.stable_error_code(),
                2,
                recovery_publisher,
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::Busy
    );

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
    ensure_failure(&redelivered, &first_payload)?;
    if redelivered.offset() != first_offset || redelivered.delivery_id() != first_delivery_id {
        return Err(invalid_data("restart changed the unacknowledged raw poison identity").into());
    }

    assert_eq!(
        store
            .reserve_and_claim(
                &poison_identity(&redelivered)?,
                redelivered.stable_error_code(),
                2,
                recovery_publisher,
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    if !matches!(
        store.mark_published(&identity, first_publisher).await,
        Err(ConsumerPoisonReceiptError::ClaimLost)
    ) {
        return Err(invalid_data("stale publisher retained authority after lease takeover").into());
    }

    let retry_entry = redelivered.to_dlq_entry(2);
    if retry_entry.broker_message_id() != Some(deterministic_message_id) {
        return Err(invalid_data("redelivery changed the deterministic DLQ message ID").into());
    }
    recovery_transport.move_to_dlq(retry_entry).await?;
    let observed_physical_dlq_messages = message_count(&observer, &stream).await?;
    if observed_physical_dlq_messages != expected_physical_dlq_messages {
        return Err(invalid_data(format!(
            "{scope} expected {expected_physical_dlq_messages} physical DLQ messages but observed {observed_physical_dlq_messages}"
        ))
        .into());
    }

    store.mark_published(&identity, recovery_publisher).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Published).await?;
    recovery_group.acknowledge_decode_failure(&redelivered).await?;
    store.mark_acknowledged(&identity).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Acknowledged).await?;

    let next_failure = receive_decode_failure(&recovery_group).await?;
    ensure_failure(&next_failure, &second_payload)?;
    if next_failure.offset() <= first_offset {
        return Err(invalid_data("terminalization did not advance to the next source delivery").into());
    }
    let next_offset = next_failure.offset();

    drop(recovery_group);
    observer.shutdown().await?;
    recovery_transport.shutdown().await?;
    evidence.cleanup().await?;

    Ok(ScenarioEvidence {
        id: format!("raw_poison_publish_mark_ambiguity_{scope}"),
        result: "passed",
        facts: json!({
            "dedup_mode": scope,
            "stream": stream,
            "first_offset": first_offset,
            "first_delivery_id": first_delivery_id,
            "deterministic_dlq_message_id": deterministic_message_id,
            "publish_succeeded_before_mark_published": true,
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
            "malformed ambiguity fixture unexpectedly decoded as a contract event",
        )
        .into()),
    }
}

fn ensure_failure(failure: &ConsumedContractDecodeFailure, expected_payload: &[u8]) -> TestResult<()> {
    if failure.topic() != FORUM_SEARCH_CONTRACT_TOPIC
        || failure.partition() == 0
        || failure.ack_token().is_none()
        || failure.raw_payload() != expected_payload
    {
        return Err(invalid_data(format!("unexpected raw poison delivery: {failure:?}")).into());
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

async fn assert_receipt_state(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    expected: ConsumerPoisonReceiptState,
) -> TestResult<()> {
    let receipt = store
        .find(identity)
        .await?
        .ok_or_else(|| invalid_data("expected poison receipt was not found"))?;
    if receipt.state != expected {
        return Err(invalid_data(format!(
            "expected poison receipt state {expected:?}, observed {:?}",
            receipt.state
        ))
        .into());
    }
    Ok(())
}

async fn connect_observer(config: &IggyConfig) -> TestResult<IggyClient> {
    let client = IggyClient::from_connection_string(&connection_string(&config.external)?)?;
    client.connect().await?;
    Ok(client)
}

async fn message_count(client: &IggyClient, stream: &str) -> TestResult<u64> {
    let stream_id: Identifier = stream.to_string().try_into()?;
    let topic_id: Identifier = "dlq".to_string().try_into()?;
    let topic = client
        .get_topic(&stream_id, &topic_id)
        .await?
        .ok_or_else(|| invalid_data("poison ambiguity DLQ topic is missing"))?;
    let partition = topic
        .partitions
        .iter()
        .find(|partition| partition.id == 1)
        .ok_or_else(|| invalid_data("poison ambiguity DLQ partition 1 is missing"))?;
    Ok(partition.messages_count)
}

async fn assert_message_count(client: &IggyClient, stream: &str, expected: u64) -> TestResult<()> {
    let observed = message_count(client, stream).await?;
    if observed != expected {
        return Err(invalid_data(format!(
            "expected {expected} physical DLQ messages but observed {observed}"
        ))
        .into());
    }
    Ok(())
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
    if address.contains("://") || address.contains('@') || address.contains('?') || address.contains('#') {
        return Err(invalid_data(format!(
            "{name} must be host:port without credentials or URL delimiters"
        )));
    }
    Ok(())
}

fn connection_string(config: &ExternalConfig) -> Result<String, IoError> {
    let address = config
        .addresses
        .first()
        .ok_or_else(|| invalid_data("Iggy address is missing"))?;
    validate_address("Iggy address", address)?;
    if config.username.is_empty() {
        Ok(format!("iggy://{address}"))
    } else {
        Ok(format!("iggy://{}:{}@{address}", config.username, config.password))
    }
}

fn optional_bounded_env(name: &'static str, max_len: usize) -> TestResult<String> {
    match env::var(name) {
        Ok(value) => Ok(bounded_env(name, value, max_len)?),
        Err(env::VarError::NotPresent) => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn bounded_env(name: &'static str, value: String, max_len: usize) -> Result<String, IoError> {
    if value.trim() != value || value.is_empty() || value.len() > max_len || value.chars().any(char::is_control) {
        return Err(invalid_data(format!("{name} is invalid for retained evidence")));
    }
    Ok(value)
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn connect_in_schema(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
        .await?;
    Ok(db)
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '_' })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() { "test".to_string() } else { normalized.to_string() }
}

fn unique_name(scope: &str) -> String {
    format!("rustok-forum-search-poison-{scope}-{}", Uuid::new_v4().simple())
}

fn write_evidence(artifact: EvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path.parent().ok_or_else(|| invalid_data("evidence path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn source_commit() -> TestResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root())
        .output()?;
    if !output.status.success() {
        return Err(invalid_data("git rev-parse HEAD failed").into());
    }
    let commit = String::from_utf8(output.stdout)?.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_data("git rev-parse HEAD returned an invalid source commit").into());
    }
    Ok(commit)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
