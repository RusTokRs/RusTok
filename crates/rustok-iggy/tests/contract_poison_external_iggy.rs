#![cfg(feature = "iggy")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::time::Duration;

use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, ExternalConfig, IggyConfig, IggyMode,
    IggyTransport, PersistentContractConsumerGroup, PersistentContractDelivery,
    SerializationFormat, TopologyConfig,
};
use rustok_iggy_connector::{
    ConnectorConfig, ConsumerCursor, ExternalConnector, IggyConnector, PublishRequest,
};
use tokio::time::timeout;
use uuid::Uuid;

const ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn malformed_delivery_redelivers_until_explicit_ack_and_dlq_keeps_exact_bytes()
-> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!("{ADDRESS_ENV} is not set; skipping external Iggy raw poison lifecycle evidence");
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let source_group = unique_name("source");
    let dlq_group = unique_name("dlq");
    let first_payload = vec![0xff, 0x00, 0x7f, 0x01];
    let second_payload = br#"{"incomplete":true"#.to_vec();

    let transport = IggyTransport::new(config.clone()).await?;
    let fixture_connector = ExternalConnector::new();
    let fixture_config = ConnectorConfig::from(&config);
    fixture_connector.connect(&fixture_config).await?;

    let first_source_cursor = transport
        .open_persistent_contract_consumer_group(&source_group, "domain")
        .await?;
    let mut dlq_cursor = fixture_connector
        .open_consumer_group(&stream, "dlq", &dlq_group)
        .await?;

    publish_fixture(&fixture_connector, &stream, first_payload.clone()).await?;
    publish_fixture(&fixture_connector, &stream, second_payload.clone()).await?;

    let first_failure = receive_decode_failure(&first_source_cursor).await?;
    assert_eq!(first_failure.raw_payload(), first_payload.as_slice());
    assert_eq!(first_failure.kind(), ContractDecodeFailureKind::Deserialize);
    assert_eq!(
        first_failure.stable_error_code(),
        "iggy.contract.decode_invalid"
    );
    assert!(first_failure.ack_token().is_some());
    let first_offset = first_failure.offset();
    let first_delivery_id = first_failure.delivery_id();

    transport.move_to_dlq(first_failure.to_dlq_entry(1)).await?;
    let first_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    assert_eq!(first_dlq.payload.as_slice(), first_payload.as_slice());
    acknowledge_cursor_message(&mut dlq_cursor, &first_dlq).await?;

    drop(first_source_cursor);
    transport.shutdown().await?;

    let reopened_transport = IggyTransport::new(config.clone()).await?;
    let reopened_source_cursor = reopened_transport
        .open_persistent_contract_consumer_group(&source_group, "domain")
        .await?;
    let redelivered = receive_decode_failure(&reopened_source_cursor).await?;
    assert_eq!(redelivered.offset(), first_offset);
    assert_eq!(redelivered.delivery_id(), first_delivery_id);
    assert_eq!(redelivered.raw_payload(), first_dlq.payload.as_slice());

    reopened_source_cursor
        .acknowledge_decode_failure(&redelivered)
        .await?;

    let second_failure = receive_decode_failure(&reopened_source_cursor).await?;
    assert_eq!(second_failure.raw_payload(), second_payload.as_slice());
    assert!(second_failure.offset() > first_offset);
    assert_ne!(second_failure.delivery_id(), first_delivery_id);

    reopened_transport
        .move_to_dlq(second_failure.to_dlq_entry(1))
        .await?;
    let second_dlq = receive_cursor_message(&mut dlq_cursor).await?;
    assert_eq!(second_dlq.payload.as_slice(), second_payload.as_slice());
    acknowledge_cursor_message(&mut dlq_cursor, &second_dlq).await?;
    reopened_source_cursor
        .acknowledge_decode_failure(&second_failure)
        .await?;

    fixture_connector.shutdown().await?;
    reopened_transport.shutdown().await?;
    Ok(())
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
            "raw-poison-fixture",
            payload,
            Uuid::new_v4().to_string(),
        ))
        .await?;
    Ok(())
}

async fn receive_decode_failure(
    consumer: &PersistentContractConsumerGroup,
) -> TestResult<ConsumedContractDecodeFailure> {
    let delivery = timeout(RECEIVE_TIMEOUT, consumer.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for an external Iggy source delivery"))??
        .ok_or_else(|| invalid_data("external Iggy source cursor ended before a delivery"))?;

    match delivery {
        PersistentContractDelivery::DecodeFailure(failure) => Ok(failure),
        PersistentContractDelivery::Event(_) => Err(invalid_data(
            "malformed fixture unexpectedly decoded as a registered contract event",
        )
        .into()),
    }
}

async fn receive_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
) -> TestResult<rustok_iggy_connector::SubscriberMessage> {
    let message = timeout(RECEIVE_TIMEOUT, cursor.receive())
        .await
        .map_err(|_| invalid_data("timed out waiting for an external Iggy DLQ delivery"))??
        .ok_or_else(|| invalid_data("external Iggy DLQ cursor ended before a delivery"))?;
    Ok(message)
}

async fn acknowledge_cursor_message(
    cursor: &mut Box<dyn ConsumerCursor>,
    message: &rustok_iggy_connector::SubscriberMessage,
) -> TestResult<()> {
    let ack_token =
        message.metadata.ack_token.as_deref().ok_or_else(|| {
            invalid_data("external Iggy DLQ delivery has no acknowledgement token")
        })?;
    cursor.acknowledge(ack_token).await?;
    Ok(())
}

fn external_test_config() -> TestResult<Option<IggyConfig>> {
    let address = match env::var(ADDRESS_ENV) {
        Ok(value) => bounded_env(ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(
            "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS must be an address such as host:8090 without credentials or query parameters",
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
            stream_name: unique_name("stream"),
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

fn unique_name(scope: &str) -> String {
    format!("rustok-poison-{scope}-{}", Uuid::new_v4().simple())
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
