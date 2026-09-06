#![cfg(feature = "index-consumer")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

use rustok_iggy::{
    ConsumedContractDecodeFailure, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractDelivery, SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptState,
    ConsumerPoisonReceiptStore,
};
use rustok_iggy_connector::{
    ConnectorConfig, ConsumerCursor, ExternalConnector, IggyConnector, PublishRequest,
    SubscriberMessage,
};
use rustok_social_graph::index_consumer::{
    SOCIAL_GRAPH_INDEX_CONSUMER_GROUP, SocialGraphIndexConsumer,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use tokio::time::timeout;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL";
const ADDRESS_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const NO_SECOND_DLQ_TIMEOUT: Duration = Duration::from_millis(750);
const PUBLISH_LEASE: Duration = Duration::from_secs(30);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Social Graph raw poison PostgreSQL/Iggy evidence"
            );
            return Ok(None);
        };
        let Some(config) = external_iggy_config(scope)? else {
            eprintln!(
                "{ADDRESS_ENV} is not set; skipping Social Graph raw poison PostgreSQL/Iggy evidence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_sg_poison_{}_{}",
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
async fn raw_poison_persists_published_before_source_acknowledgement() -> TestResult<()> {
    let Some(evidence) = PostgresIggyEvidence::setup("published_before_ack").await? else {
        return Ok(());
    };

    let stream = evidence.config.topology.stream_name.clone();
    let dlq_group = unique_name("ordering-dlq");
    let transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);
    let consumer =
        SocialGraphIndexConsumer::open(Arc::clone(&transport), evidence.db.clone()).await?;
    let mut dlq_cursor = evidence
        .fixture
        .open_consumer_group(&stream, "dlq", &dlq_group)
        .await?;

    let first_payload = vec![0xff, 0x00, 0x31, 0x01];
    let second_payload = vec![0xff, 0x00, 0x31, 0x02];
    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let failure = receive_decode_failure(&consumer).await?;
    assert_eq!(failure.raw_payload(), first_payload.as_slice());
    let first_offset = failure.offset();
    let identity = poison_identity(&failure)?;
    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let publisher_id = Uuid::new_v4();

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                failure.stable_error_code(),
                1,
                publisher_id,
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Publishing).await?;

    transport.move_to_dlq(failure.to_dlq_entry(1)).await?;
    let physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    assert_eq!(physical_dlq.payload.as_slice(), first_payload.as_slice());
    acknowledge_cursor_message(&mut dlq_cursor, &physical_dlq).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Publishing).await?;

    store.mark_published(&identity, publisher_id).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Published).await?;

    consumer.acknowledge_decode_failure(&failure).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Published).await?;
    store.mark_acknowledged(&identity).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Acknowledged).await?;

    let next_failure = receive_decode_failure(&consumer).await?;
    assert_eq!(next_failure.raw_payload(), second_payload.as_slice());
    assert!(next_failure.offset() > first_offset);

    drop(consumer);
    transport.shutdown().await?;
    evidence.cleanup().await
}

#[tokio::test]
async fn published_redelivery_is_acknowledgement_only_without_republication() -> TestResult<()> {
    let Some(evidence) = PostgresIggyEvidence::setup("ack_only_redelivery").await? else {
        return Ok(());
    };

    let stream = evidence.config.topology.stream_name.clone();
    let dlq_group = unique_name("recovery-dlq");
    let first_transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);
    let first_consumer =
        SocialGraphIndexConsumer::open(Arc::clone(&first_transport), evidence.db.clone()).await?;
    let mut dlq_cursor = evidence
        .fixture
        .open_consumer_group(&stream, "dlq", &dlq_group)
        .await?;

    let first_payload = vec![0xff, 0x00, 0x41, 0x01];
    let second_payload = vec![0xff, 0x00, 0x41, 0x02];
    publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;
    publish_fixture(&evidence.fixture, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_consumer).await?;
    let first_offset = first_failure.offset();
    let first_delivery_id = first_failure.delivery_id();
    let identity = poison_identity(&first_failure)?;
    let store = ConsumerPoisonReceiptStore::new(evidence.db.clone());
    let publisher_id = Uuid::new_v4();

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                first_failure.stable_error_code(),
                1,
                publisher_id,
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    first_transport
        .move_to_dlq(first_failure.to_dlq_entry(1))
        .await?;
    let physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    assert_eq!(physical_dlq.payload.as_slice(), first_payload.as_slice());
    acknowledge_cursor_message(&mut dlq_cursor, &physical_dlq).await?;
    store.mark_published(&identity, publisher_id).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Published).await?;

    drop(first_consumer);
    first_transport.shutdown().await?;

    let reopened_transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);
    let reopened_consumer =
        SocialGraphIndexConsumer::open(Arc::clone(&reopened_transport), evidence.db.clone())
            .await?;
    let redelivered = receive_decode_failure(&reopened_consumer).await?;
    assert_eq!(redelivered.offset(), first_offset);
    assert_eq!(redelivered.delivery_id(), first_delivery_id);
    assert_eq!(redelivered.raw_payload(), first_payload.as_slice());

    assert_eq!(
        store
            .reserve_and_claim(
                &poison_identity(&redelivered)?,
                redelivered.stable_error_code(),
                9,
                Uuid::new_v4(),
                PUBLISH_LEASE,
            )
            .await?,
        ConsumerPoisonPublishClaim::AlreadyPublished
    );
    reopened_consumer
        .acknowledge_decode_failure(&redelivered)
        .await?;
    store.mark_acknowledged(&identity).await?;
    assert_receipt_state(&store, &identity, ConsumerPoisonReceiptState::Acknowledged).await?;

    assert_no_second_dlq_message(&mut dlq_cursor).await?;
    let next_failure = receive_decode_failure(&reopened_consumer).await?;
    assert_eq!(next_failure.raw_payload(), second_payload.as_slice());
    assert!(next_failure.offset() > first_offset);

    drop(reopened_consumer);
    reopened_transport.shutdown().await?;
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
            "raw-poison-ordering-fixture",
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
        .map_err(|_| invalid_data("timed out waiting for a Social Graph raw poison delivery"))??
        .ok_or_else(|| invalid_data("Social Graph source cursor ended before a delivery"))?;
    match delivery {
        PersistentContractDelivery::DecodeFailure(failure) => Ok(*failure),
        PersistentContractDelivery::Event(_) => Err(invalid_data(
            "malformed ordering fixture unexpectedly decoded as a registered contract event",
        )
        .into()),
    }
}

async fn receive_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
) -> TestResult<SubscriberMessage> {
    let message = timeout(RECEIVE_TIMEOUT, cursor.receive())
        .await
        .map_err(|_| invalid_data("timed out waiting for a raw poison DLQ message"))??;
    message.ok_or_else(|| {
        Box::<dyn Error + Send + Sync>::from(invalid_data(
            "raw poison DLQ cursor ended before a message",
        ))
    })
}

async fn acknowledge_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
    message: &SubscriberMessage,
) -> TestResult<()> {
    let token = message
        .metadata
        .ack_token
        .as_deref()
        .ok_or_else(|| invalid_data("raw poison DLQ message has no acknowledgement token"))?;
    cursor.acknowledge(token).await?;
    Ok(())
}

async fn assert_no_second_dlq_message(cursor: &mut Box<dyn ConsumerCursor>) -> TestResult<()> {
    match timeout(NO_SECOND_DLQ_TIMEOUT, cursor.receive()).await {
        Err(_) | Ok(Ok(None)) => Ok(()),
        Ok(Ok(Some(_))) => Err(invalid_data(
            "acknowledgement-only recovery unexpectedly published another DLQ message",
        )
        .into()),
        Ok(Err(error)) => Err(error.into()),
    }
}

async fn assert_receipt_state(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    expected: ConsumerPoisonReceiptState,
) -> TestResult<()> {
    let receipt = store
        .find(identity)
        .await?
        .ok_or_else(|| invalid_data("expected neutral poison receipt was not found"))?;
    assert_eq!(receipt.state, expected);
    Ok(())
}

fn poison_identity(
    failure: &ConsumedContractDecodeFailure,
) -> Result<ConsumerPoisonIdentity, rustok_iggy_connector::migrations::ConsumerPoisonReceiptError> {
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

fn external_iggy_config(scope: &str) -> TestResult<Option<IggyConfig>> {
    let address = match env::var(ADDRESS_ENV) {
        Ok(value) => bounded_env(ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(format!(
            "{ADDRESS_ENV} must be host:port without scheme, credentials, or query"
        ))
        .into());
    }

    let username = optional_bounded_env(USERNAME_ENV, 191)?;
    let password = optional_bounded_env(PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data(
            "raw poison PostgreSQL/Iggy username and password must both be set or both be empty",
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
    format!("rustok-sg-poison-{scope}-{}", Uuid::new_v4().simple())
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
