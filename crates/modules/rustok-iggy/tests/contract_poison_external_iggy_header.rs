#![cfg(feature = "iggy")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::time::Duration;

use futures_util::StreamExt;
use iggy::prelude::{Client, IggyClient};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, ExternalConfig, IggyConfig, IggyMode,
    IggyTransport, SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::SubscriberMessageMetadata;
use tokio::time::timeout;
use uuid::Uuid;

const ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const PARTITIONS: u32 = 3;
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn deterministic_dlq_uuid_is_physical_iggy_header_and_selects_one_based_partition()
-> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!("{ADDRESS_ENV} is not set; skipping external Iggy physical DLQ header evidence");
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let probe_group = unique_name("header-probe");
    let transport = IggyTransport::new(config.clone()).await?;
    let client = connect_sdk_probe(&config).await?;
    let mut probe = client
        .consumer_group(&probe_group, &stream, "dlq")?
        .commit_failed_messages()
        .build();
    probe.init().await?;

    let payload = vec![0xff, 0x00, 0x7f, 0x22, 0x01];
    let failure = synthetic_decode_failure(&stream, payload.clone())?;
    let entry = failure.to_dlq_entry(1);
    let expected_id = entry
        .broker_message_id()
        .ok_or_else(|| invalid_data("decode failure DLQ entry has no deterministic broker ID"))?;
    let expected_partition = expected_partition(expected_id, PARTITIONS)?;

    transport.move_to_dlq(entry).await?;

    let received = timeout(RECEIVE_TIMEOUT, probe.next())
        .await
        .map_err(|_| invalid_data("timed out waiting for the physical Iggy DLQ message"))?
        .ok_or_else(|| invalid_data("Iggy DLQ probe ended before receiving a message"))??;

    assert_eq!(received.message.header.id, expected_id.as_u128());
    assert_eq!(received.partition_id, expected_partition);
    assert_eq!(received.message.payload.as_ref(), payload.as_slice());

    probe
        .store_offset(received.message.header.offset, Some(received.partition_id))
        .await?;
    drop(probe);
    client.shutdown().await?;
    transport.shutdown().await?;
    Ok(())
}

fn synthetic_decode_failure(
    stream: &str,
    payload: Vec<u8>,
) -> rustok_core::Result<ConsumedContractDecodeFailure> {
    ConsumedContractDecodeFailure::new(
        stream.to_string(),
        "domain".to_string(),
        SubscriberMessageMetadata::new(stream, "domain", 1)
            .with_offset(42)
            .with_ack_token("physical-header-evidence-only"),
        payload,
        ContractDecodeFailureKind::Deserialize,
    )
}

fn expected_partition(message_id: Uuid, partitions: u32) -> Result<u32, IoError> {
    if partitions == 0 {
        return Err(invalid_data(
            "physical header evidence requires at least one partition",
        ));
    }
    let partition = (message_id.as_u128() % u128::from(partitions)) as u32 + 1;
    if !(1..=partitions).contains(&partition) {
        return Err(invalid_data(
            "deterministic Iggy partition is outside the one-based range",
        ));
    }
    Ok(partition)
}

async fn connect_sdk_probe(config: &IggyConfig) -> TestResult<IggyClient> {
    let connection_string = connection_string(&config.external)?;
    let client = IggyClient::from_connection_string(&connection_string)?;
    client.connect().await?;
    Ok(client)
}

fn connection_string(config: &ExternalConfig) -> Result<String, IoError> {
    let address = config
        .addresses
        .first()
        .ok_or_else(|| invalid_data("external Iggy evidence address is missing"))?;
    if config.protocol != "tcp" {
        return Err(invalid_data("physical Iggy header evidence requires TCP"));
    }
    if config.tls_enabled || config.tls_domain.is_some() || config.tls_ca_file.is_some() {
        return Err(invalid_data(
            "physical Iggy header evidence does not claim TLS coverage",
        ));
    }
    if config.username.is_empty() != config.password.is_empty() {
        return Err(invalid_data(
            "external Iggy evidence username and password must both be set or both be empty",
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

fn external_test_config() -> TestResult<Option<IggyConfig>> {
    let address = match env::var(ADDRESS_ENV) {
        Ok(value) => bounded_env(ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(
            "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS must be host:port without credentials or query parameters",
        )
        .into());
    }

    let username = optional_bounded_env(USERNAME_ENV, 191)?;
    let password = optional_bounded_env(PASSWORD_ENV, 191)?;
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
            stream_name: unique_name("header-stream"),
            domain_partitions: PARTITIONS,
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
    format!("rustok-poison-{scope}-{}", Uuid::new_v4().simple())
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
