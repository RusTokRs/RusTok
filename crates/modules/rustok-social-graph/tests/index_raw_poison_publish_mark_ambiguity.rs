#![cfg(feature = "index-consumer")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

use iggy::prelude::{Client, Identifier, IggyClient, TopicClient};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractDelivery, SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptError,
    ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
};
use rustok_iggy_connector::{ConnectorConfig, ExternalConnector, IggyConnector, PublishRequest};
use rustok_social_graph::index_consumer::{
    SOCIAL_GRAPH_INDEX_CONSUMER_GROUP, SocialGraphIndexConsumer,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use tokio::time::timeout;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL";
const DEDUP_ENABLED_ADDRESS_ENV: &str =
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_ADDRESS";
const DEDUP_DISABLED_ADDRESS_ENV: &str =
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const PUBLISH_LEASE: Duration = Duration::from_secs(1);
const LEASE_RECLAIM_WAIT: Duration = Duration::from_millis(1_500);

const DEDUP_ENABLED_RETRY_COUNT: u64 = 1;
const DEDUP_DISABLED_RETRY_COUNT: u64 = 2;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct PostgresIggyAmbiguityEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
    config: IggyConfig,
    fixture: ExternalConnector,
}

impl PostgresIggyAmbiguityEvidence {
    async fn setup(address_env: &'static str, scope: &str) -> TestResult<Option<Self>> {
        ensure_distinct_mode_addresses()?;
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Social Graph publish/mark ambiguity evidence"
            );
            return Ok(None);
        };
        let Some(config) = external_iggy_config(address_env, scope)? else {
            eprintln!(
                "{address_env} is not set; skipping Social Graph publish/mark ambiguity evidence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_sg_poison_ambiguity_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect_in_schema(&database_url, &schema_name).await?;
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

#[tokio::test]
async fn dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate() -> TestResult<()>
{
    exercise_publish_mark_ambiguity(
        DEDUP_ENABLED_ADDRESS_ENV,
        "dedup_enabled",
        DEDUP_ENABLED_RETRY_COUNT,
    )
    .await
}

#[tokio::test]
async fn dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate() -> TestResult<()> {
    exercise_publish_mark_ambiguity(
        DEDUP_DISABLED_ADDRESS_ENV,
        "dedup_disabled",
        DEDUP_DISABLED_RETRY_COUNT,
    )
    .await
}

async fn exercise_publish_mark_ambiguity(
    address_env: &'static str,
    scope: &str,
    expected_retry_count: u64,
) -> TestResult<()> {
    let Some(evidence) = PostgresIggyAmbiguityEvidence::setup(address_env, scope).await? else {
        return Ok(());
    };

    let stream = evidence.config.topology.stream_name.clone();
    let first_transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);
    let first_consumer =
        SocialGraphIndexConsumer::open(Arc::clone(&first_transport), evidence.db.clone()).await?;
    let observer = connect_observer(&evidence.config).await?;

    let first_payload = match expected_retry_count {
        DEDUP_ENABLED_RETRY_COUNT => vec![0xff, 0x00, 0x51, 0x01],
        DEDUP_DISABLED_RETRY_COUNT => vec![0xff, 0x00, 0x52, 0x01],
        other => return Err(invalid_data(format!("unsupported retry count {other}")).into()),
    };
    let second_payload = match expected_retry_count {
        DEDUP_ENABLED_RETRY_COUNT => vec![0xff, 0x00, 0x51, 0x02],
        DEDUP_DISABLED_RETRY_COUNT => vec![0xff, 0x00, 0x52, 0x02],
        _ => unreachable!("validated retry count"),
    };
    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_consumer).await?;
    assert_eq!(first_failure.raw_payload(), first_payload.as_slice());
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
    let first_broker_message_id = first_entry.broker_message_id().ok_or_else(|| {
        invalid_data("raw poison ambiguity entry has no deterministic message ID")
    })?;
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

    drop(first_consumer);
    first_transport.shutdown().await?;
    tokio::time::sleep(LEASE_RECLAIM_WAIT).await;

    let recovery_transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);
    let recovery_consumer =
        SocialGraphIndexConsumer::open(Arc::clone(&recovery_transport), evidence.db.clone())
            .await?;
    let redelivered = receive_decode_failure(&recovery_consumer).await?;
    assert_eq!(redelivered.offset(), first_offset);
    assert_eq!(redelivered.delivery_id(), first_delivery_id);
    assert_eq!(redelivered.raw_payload(), first_payload.as_slice());

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
    assert!(matches!(
        store.mark_published(&identity, first_publisher).await,
        Err(ConsumerPoisonReceiptError::ClaimLost)
    ));

    let retry_entry = redelivered.to_dlq_entry(2);
    assert_eq!(
        retry_entry.broker_message_id(),
        Some(first_broker_message_id)
    );
    recovery_transport.move_to_dlq(retry_entry).await?;
    assert_message_count(&observer, &stream, expected_retry_count).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Publishing).await?;

    store.mark_published(&identity, recovery_publisher).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Published).await?;
    recovery_consumer
        .acknowledge_decode_failure(&redelivered)
        .await?;
    store.mark_acknowledged(&identity).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Acknowledged).await?;

    let next_failure = receive_decode_failure(&recovery_consumer).await?;
    assert_eq!(next_failure.raw_payload(), second_payload.as_slice());
    assert!(next_failure.offset() > first_offset);

    drop(recovery_consumer);
    observer.shutdown().await?;
    recovery_transport.shutdown().await?;
    evidence.cleanup().await
}

async fn publish_fixture(
    connector: &ExternalConnector,
    stream: &str,
    payload: Vec<u8>,
) -> TestResult<()> {
    connector
        .publish(PublishRequest::new(
            stream,
            "domain",
            "raw-poison-publish-mark-ambiguity-fixture",
            payload,
            Uuid::new_v4().to_string(),
        ))
        .await?;
    Ok(())
}

async fn receive_decode_failure(
    consumer: &SocialGraphIndexConsumer,
) -> TestResult<ConsumedContractDecodeFailure> {
    let delivery = timeout(RECEIVE_TIMEOUT, consumer.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for publish/mark ambiguity delivery"))??
        .ok_or_else(|| {
            invalid_data("source cursor ended before publish/mark ambiguity delivery")
        })?;
    match delivery {
        PersistentContractDelivery::DecodeFailure(failure) => Ok(*failure),
        PersistentContractDelivery::Event(_) => Err(invalid_data(
            "malformed publish/mark ambiguity fixture unexpectedly decoded as a contract event",
        )
        .into()),
    }
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
        .ok_or_else(|| invalid_data("publish/mark ambiguity DLQ topic is missing"))?;
    let partition = topic
        .partitions
        .iter()
        .find(|partition| partition.id == 1)
        .ok_or_else(|| invalid_data("publish/mark ambiguity DLQ partition 1 is missing"))?;
    Ok(partition.messages_count)
}

async fn assert_message_count(client: &IggyClient, stream: &str, expected: u64) -> TestResult<()> {
    let observed = message_count(client, stream).await?;
    if observed != expected {
        return Err(invalid_data(format!(
            "publish/mark ambiguity expected {expected} physical DLQ messages but observed {observed}"
        ))
        .into());
    }
    Ok(())
}

async fn assert_receipt_state(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    expected: ConsumerPoisonReceiptState,
) -> TestResult<()> {
    let receipt = store
        .find(identity)
        .await?
        .ok_or_else(|| invalid_data("expected publish/mark ambiguity receipt was not found"))?;
    assert_eq!(receipt.state, expected);
    Ok(())
}

fn poison_identity(
    failure: &ConsumedContractDecodeFailure,
) -> Result<ConsumerPoisonIdentity, ConsumerPoisonReceiptError> {
    ConsumerPoisonIdentity::new(
        failure.delivery_id(),
        SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        failure.stream(),
        failure.topic(),
        failure.partition(),
        failure.offset(),
        failure.raw_payload().to_vec(),
    )
}

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
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
        return Err(invalid_data(
            "publish/mark ambiguity Iggy username and password must both be set or both be empty",
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
                "dedup-enabled and dedup-disabled ambiguity evidence require distinct Iggy addresses",
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

fn connection_string(config: &ExternalConfig) -> Result<String, IoError> {
    let address = config
        .addresses
        .first()
        .ok_or_else(|| invalid_data("publish/mark ambiguity Iggy address is missing"))?;
    if config.protocol != "tcp" {
        return Err(invalid_data("publish/mark ambiguity evidence requires TCP"));
    }
    if config.tls_enabled || config.tls_domain.is_some() || config.tls_ca_file.is_some() {
        return Err(invalid_data(
            "publish/mark ambiguity source harness does not claim TLS coverage",
        ));
    }
    validate_connection_component(address, "address", &['@', '?', '#'])?;
    validate_connection_component(&config.username, "username", &[':', '@'])?;
    validate_connection_component(&config.password, "password", &[':', '@'])?;

    if config.username.is_empty() {
        Ok(format!("iggy://{address}"))
    } else {
        Ok(format!(
            "iggy://{}:{}@{address}",
            config.username, config.password
        ))
    }
}

fn validate_connection_component(
    value: &str,
    field: &'static str,
    forbidden: &[char],
) -> Result<(), IoError> {
    if let Some(delimiter) = value
        .chars()
        .find(|character| forbidden.contains(character))
    {
        return Err(invalid_data(format!(
            "Iggy {field} contains unsupported connection delimiter '{delimiter}'"
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
    format!(
        "rustok-sg-poison-ambiguity-{scope}-{}",
        Uuid::new_v4().simple()
    )
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
