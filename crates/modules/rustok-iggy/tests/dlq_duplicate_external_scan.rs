#![cfg(feature = "iggy")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use iggy::prelude::{Client, Consumer, ConsumerKind, ConsumerOffsetClient, Identifier, IggyClient};
use rustok_iggy::{
    DlqEntry, ExternalConfig, IggyConfig, IggyDlqDuplicateScanRequest, IggyDlqDuplicateScanner,
    IggyMode, IggyTransport, SerializationFormat, TopologyConfig,
};
use uuid::Uuid;

const ADDRESS_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_PASSWORD";
const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-readonly-v1";
const PARTITION_ID: u32 = 1;
const PHYSICAL_MESSAGE_COUNT: u32 = 4;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset() -> TestResult<()>
{
    let Some(config) = external_test_config()? else {
        eprintln!("{ADDRESS_ENV} is not set; skipping external Iggy DLQ duplicate scan evidence");
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let transport = IggyTransport::new(config.clone()).await?;
    let client = connect_sdk_observer(&config).await?;
    let scanner = IggyDlqDuplicateScanner::new(&client, &stream)?;
    let request = IggyDlqDuplicateScanRequest::new(
        vec![PARTITION_ID],
        0,
        PHYSICAL_MESSAGE_COUNT,
        PHYSICAL_MESSAGE_COUNT,
    )?;

    let stream_id: Identifier = stream.clone().try_into()?;
    let topic_id: Identifier = "dlq".to_string().try_into()?;
    let consumer_id: Identifier = READ_ONLY_CONSUMER.to_string().try_into()?;
    let read_only_consumer = Consumer {
        kind: ConsumerKind::Consumer,
        id: consumer_id,
    };

    assert_no_stored_offset(&client, &read_only_consumer, &stream_id, &topic_id).await?;

    let ordinary_duplicate_id = Uuid::new_v4();
    let identity_conflict_id = Uuid::new_v4();
    let ordinary_payload = vec![0xff, 0x00, 0x61, 0x01];
    let conflict_payload_first = vec![0xff, 0x00, 0x62, 0x01];
    let conflict_payload_second = vec![0xff, 0x00, 0x62, 0x02];

    publish_physical(
        &transport,
        ordinary_duplicate_id,
        ordinary_payload.clone(),
        1,
    )
    .await?;
    publish_physical(&transport, ordinary_duplicate_id, ordinary_payload, 2).await?;
    publish_physical(&transport, identity_conflict_id, conflict_payload_first, 1).await?;
    publish_physical(&transport, identity_conflict_id, conflict_payload_second, 2).await?;

    let first = scanner.summarize(&request).await?;
    assert_summary(&first);
    assert_no_stored_offset(&client, &read_only_consumer, &stream_id, &topic_id).await?;

    let second = scanner.summarize(&request).await?;
    assert_eq!(second, first);
    assert_summary(&second);
    assert_no_stored_offset(&client, &read_only_consumer, &stream_id, &topic_id).await?;

    drop(scanner);
    client.shutdown().await?;
    transport.shutdown().await?;
    Ok(())
}

fn assert_summary(summary: &rustok_iggy::DlqDuplicateSummary) {
    assert_eq!(summary.total_messages(), 4);
    assert_eq!(summary.unique_message_ids(), 2);
    assert_eq!(summary.duplicate_messages(), 2);
    assert_eq!(summary.duplicate_groups(), 2);
    assert_eq!(summary.conflicting_payload_groups(), 1);
    assert_eq!(summary.max_copies_per_message_id(), 2);
    assert!(summary.has_physical_duplicates());
    assert!(summary.has_identity_conflicts());
    assert!(summary.requires_manual_review());
}

async fn publish_physical(
    transport: &IggyTransport,
    broker_message_id: Uuid,
    payload: Vec<u8>,
    retry_count: u32,
) -> TestResult<()> {
    let entry = DlqEntry::new(
        broker_message_id,
        "domain",
        payload,
        "iggy.contract.decode_invalid",
        retry_count,
    )
    .with_broker_message_id(broker_message_id);
    transport.move_to_dlq(entry).await?;
    Ok(())
}

async fn assert_no_stored_offset(
    client: &IggyClient,
    consumer: &Consumer,
    stream_id: &Identifier,
    topic_id: &Identifier,
) -> TestResult<()> {
    let stored = client
        .get_consumer_offset(consumer, stream_id, topic_id, Some(PARTITION_ID))
        .await?;
    if stored.is_some() {
        return Err(invalid_data(
            "read-only external duplicate scan unexpectedly stored a consumer offset",
        )
        .into());
    }
    Ok(())
}

async fn connect_sdk_observer(config: &IggyConfig) -> TestResult<IggyClient> {
    let client = IggyClient::from_connection_string(&connection_string(&config.external)?)?;
    client.connect().await?;
    Ok(client)
}

fn connection_string(config: &ExternalConfig) -> Result<String, IoError> {
    let address = config
        .addresses
        .first()
        .ok_or_else(|| invalid_data("external duplicate scan evidence address is missing"))?;
    if config.protocol != "tcp" {
        return Err(invalid_data(
            "external duplicate scan evidence requires TCP",
        ));
    }
    if config.tls_enabled || config.tls_domain.is_some() || config.tls_ca_file.is_some() {
        return Err(invalid_data(
            "external duplicate scan source harness does not claim TLS coverage",
        ));
    }
    if config.username.is_empty() != config.password.is_empty() {
        return Err(invalid_data(
            "external duplicate scan username and password must both be set or both be empty",
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
    if address.contains("://")
        || address.contains('@')
        || address.contains('?')
        || address.contains('#')
    {
        return Err(invalid_data(format!(
            "{ADDRESS_ENV} must be host:port without URL or credential delimiters"
        ))
        .into());
    }

    let username = optional_bounded_env(USERNAME_ENV, 191)?;
    let password = optional_bounded_env(PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data(
            "external duplicate scan username and password must both be set or both be empty",
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
            stream_name: unique_name("duplicate-scan"),
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
