#![cfg(feature = "iggy")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::time::Duration;

use iggy::prelude::{Client, Identifier, IggyClient, TopicClient};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, DlqEntry, ExternalConfig, IggyConfig,
    IggyMode, IggyTransport, SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::SubscriberMessageMetadata;
use uuid::Uuid;

const DISABLED_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS";
const ENABLED_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS";
const CAPACITY_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS";
const EXPIRY_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_DEDUP_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_DEDUP_TEST_PASSWORD";
const EXPIRY_WAIT_ENV: &str = "RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS";
const MIN_EXPIRY_WAIT_MS: u64 = 100;
const MAX_EXPIRY_WAIT_MS: u64 = 300_000;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn disabled_deduplication_persists_repeated_uuid_twice() -> TestResult<()> {
    let Some(harness) = ExternalDedupHarness::start(DISABLED_ADDRESS_ENV, "disabled").await? else {
        eprintln!(
            "{DISABLED_ADDRESS_ENV} is not set; skipping disabled Iggy deduplication evidence"
        );
        return Ok(());
    };

    let entry = poison_entry(&harness.stream, 10, vec![0x10, 0x20, 0x30])?;
    harness.assert_message_count(0).await?;
    harness.transport.move_to_dlq(entry.clone()).await?;
    harness.assert_message_count(1).await?;
    harness.transport.move_to_dlq(entry).await?;
    harness.assert_message_count(2).await?;
    harness.shutdown().await
}

#[tokio::test]
async fn enabled_deduplication_suppresses_immediate_repeated_uuid() -> TestResult<()> {
    let Some(harness) = ExternalDedupHarness::start(ENABLED_ADDRESS_ENV, "enabled").await? else {
        eprintln!("{ENABLED_ADDRESS_ENV} is not set; skipping enabled Iggy deduplication evidence");
        return Ok(());
    };

    let entry = poison_entry(&harness.stream, 20, vec![0x20, 0x30, 0x40])?;
    harness.assert_message_count(0).await?;
    harness.transport.move_to_dlq(entry.clone()).await?;
    harness.assert_message_count(1).await?;
    harness.transport.move_to_dlq(entry).await?;
    harness.assert_message_count(1).await?;
    harness.shutdown().await
}

#[tokio::test]
async fn bounded_deduplication_capacity_eviction_accepts_old_uuid_again() -> TestResult<()> {
    let Some(harness) = ExternalDedupHarness::start(CAPACITY_ADDRESS_ENV, "capacity").await? else {
        eprintln!(
            "{CAPACITY_ADDRESS_ENV} is not set; skipping capacity-eviction Iggy deduplication evidence"
        );
        return Ok(());
    };

    let first = poison_entry(&harness.stream, 30, vec![0x30, 0x40, 0x50])?;
    let second = poison_entry(&harness.stream, 31, vec![0x31, 0x41, 0x51])?;
    assert_ne!(first.broker_message_id(), second.broker_message_id());

    harness.assert_message_count(0).await?;
    harness.transport.move_to_dlq(first.clone()).await?;
    harness.assert_message_count(1).await?;
    harness.transport.move_to_dlq(first.clone()).await?;
    harness.assert_message_count(1).await?;
    harness.transport.move_to_dlq(second).await?;
    harness.assert_message_count(2).await?;
    harness.transport.move_to_dlq(first).await?;
    harness.assert_message_count(3).await?;
    harness.shutdown().await
}

#[tokio::test]
async fn expired_deduplication_entry_accepts_same_uuid_after_bounded_wait() -> TestResult<()> {
    let Some(harness) = ExternalDedupHarness::start(EXPIRY_ADDRESS_ENV, "expiry").await? else {
        eprintln!("{EXPIRY_ADDRESS_ENV} is not set; skipping expiry Iggy deduplication evidence");
        return Ok(());
    };
    let wait = expiry_wait()?;
    let entry = poison_entry(&harness.stream, 40, vec![0x40, 0x50, 0x60])?;

    harness.assert_message_count(0).await?;
    harness.transport.move_to_dlq(entry.clone()).await?;
    harness.assert_message_count(1).await?;
    harness.transport.move_to_dlq(entry.clone()).await?;
    harness.assert_message_count(1).await?;
    tokio::time::sleep(wait).await;
    harness.transport.move_to_dlq(entry).await?;
    harness.assert_message_count(2).await?;
    harness.shutdown().await
}

struct ExternalDedupHarness {
    transport: IggyTransport,
    client: IggyClient,
    stream: String,
    stream_id: Identifier,
    topic_id: Identifier,
}

impl ExternalDedupHarness {
    async fn start(address_env: &'static str, scope: &str) -> TestResult<Option<Self>> {
        let Some(config) = external_test_config(address_env, scope)? else {
            return Ok(None);
        };
        let stream = config.topology.stream_name.clone();
        let stream_id: Identifier = stream.clone().try_into()?;
        let topic_id: Identifier = "dlq".to_string().try_into()?;
        let transport = IggyTransport::new(config.clone()).await?;
        let client = connect_observer(&config).await?;
        Ok(Some(Self {
            transport,
            client,
            stream,
            stream_id,
            topic_id,
        }))
    }

    async fn message_count(&self) -> TestResult<u64> {
        let topic = self
            .client
            .get_topic(&self.stream_id, &self.topic_id)
            .await?
            .ok_or_else(|| invalid_data("external Iggy dedup evidence DLQ topic is missing"))?;
        let partition = topic
            .partitions
            .iter()
            .find(|partition| partition.id == 1)
            .ok_or_else(|| invalid_data("external Iggy dedup evidence partition 1 is missing"))?;
        Ok(partition.messages_count)
    }

    async fn assert_message_count(&self, expected: u64) -> TestResult<()> {
        let observed = self.message_count().await?;
        if observed != expected {
            return Err(invalid_data(format!(
                "external Iggy dedup evidence expected {expected} DLQ messages but observed {observed}"
            ))
            .into());
        }
        Ok(())
    }

    async fn shutdown(self) -> TestResult<()> {
        self.client.shutdown().await?;
        self.transport.shutdown().await?;
        Ok(())
    }
}

fn poison_entry(stream: &str, offset: u64, payload: Vec<u8>) -> rustok_core::Result<DlqEntry> {
    Ok(ConsumedContractDecodeFailure::new(
        stream.to_string(),
        "domain".to_string(),
        SubscriberMessageMetadata::new(stream, "domain", 1)
            .with_offset(offset)
            .with_ack_token(format!("dedup-evidence-{offset}")),
        payload,
        ContractDecodeFailureKind::Deserialize,
    )?
    .to_dlq_entry(1))
}

fn external_test_config(address_env: &'static str, scope: &str) -> TestResult<Option<IggyConfig>> {
    let address = match env::var(address_env) {
        Ok(value) => bounded_env(address_env, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(format!(
            "{address_env} must be host:port without credentials or query parameters"
        ))
        .into());
    }

    let username = optional_bounded_env(USERNAME_ENV, 191)?;
    let password = optional_bounded_env(PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data(
            "Iggy dedup evidence username and password must both be set or both be empty",
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

async fn connect_observer(config: &IggyConfig) -> TestResult<IggyClient> {
    let connection_string = connection_string(&config.external)?;
    let client = IggyClient::from_connection_string(&connection_string)?;
    client.connect().await?;
    Ok(client)
}

fn connection_string(config: &ExternalConfig) -> Result<String, IoError> {
    let address = config
        .addresses
        .first()
        .ok_or_else(|| invalid_data("external Iggy dedup evidence address is missing"))?;
    if config.protocol != "tcp" {
        return Err(invalid_data("Iggy dedup evidence requires TCP"));
    }
    if config.tls_enabled || config.tls_domain.is_some() || config.tls_ca_file.is_some() {
        return Err(invalid_data(
            "Iggy dedup evidence does not claim TLS coverage",
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

fn expiry_wait() -> TestResult<Duration> {
    let value = env::var(EXPIRY_WAIT_ENV).map_err(|error| {
        invalid_data(format!(
            "{EXPIRY_WAIT_ENV} is required for expiry evidence: {error}"
        ))
    })?;
    let millis = value
        .parse::<u64>()
        .map_err(|error| invalid_data(format!("{EXPIRY_WAIT_ENV} is invalid: {error}")))?;
    if !(MIN_EXPIRY_WAIT_MS..=MAX_EXPIRY_WAIT_MS).contains(&millis) {
        return Err(invalid_data(format!(
            "{EXPIRY_WAIT_ENV} must be between {MIN_EXPIRY_WAIT_MS} and {MAX_EXPIRY_WAIT_MS}"
        ))
        .into());
    }
    Ok(Duration::from_millis(millis))
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

fn unique_name(scope: &str) -> String {
    format!("rustok-poison-dedup-{scope}-{}", Uuid::new_v4().simple())
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
