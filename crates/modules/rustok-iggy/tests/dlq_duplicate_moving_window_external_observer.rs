#![cfg(feature = "iggy")]

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use iggy::prelude::{Client, Consumer, ConsumerKind, ConsumerOffsetClient, Identifier, IggyClient};
use rustok_iggy::{
    DlqDuplicateSummary, DlqEntry, ExternalConfig, IggyConfig,
    IggyDlqDuplicateAlertMovingWindowConfig, IggyDlqDuplicateAlertObserver,
    IggyDlqDuplicateAlertScanMode, IggyMode, IggyTransport, SerializationFormat, TopologyConfig,
};
use uuid::Uuid;

const ADDRESS_ENV: &str = "RUSTOK_IGGY_MOVING_OBSERVER_TEST_ADDRESS";
const USERNAME_ENV: &str = "RUSTOK_IGGY_MOVING_OBSERVER_TEST_USERNAME";
const PASSWORD_ENV: &str = "RUSTOK_IGGY_MOVING_OBSERVER_TEST_PASSWORD";
const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-moving-readonly-v1";
const PARTITION: u32 = 1;
const PARTITION_COUNT: u32 = 1;
const INITIAL_OFFSET: u64 = 0;
const PER_PARTITION_MESSAGES: u32 = 1;
const BATCH_SIZE: u32 = 1;
const ROLLING_MAX_CYCLES: u32 = 3;
const ROLLING_MAX_OBSERVATIONS_PER_CYCLE: u32 = 1;

const EVIDENCE_MARKER: &str = "RUSTOK_MOVING_OBSERVER_EVIDENCE first_total=1 first_duplicates=0 second_total=2 second_duplicates=1 second_groups=1 second_max_copies=2 third_equal=true replacement_equal_first=true stored_offsets=0";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn moving_observer_retains_duplicate_across_advancing_cycles() -> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!(
            "{ADDRESS_ENV} is not set; skipping external Iggy moving-observer duplicate evidence"
        );
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let transport = IggyTransport::new(config.clone()).await?;
    let offset_client = connect_sdk_observer(&config).await?;
    let stream_id: Identifier = stream.clone().try_into()?;
    let topic_id: Identifier = "dlq".to_string().try_into()?;
    let consumer_id: Identifier = READ_ONLY_CONSUMER.to_string().try_into()?;
    let read_only_consumer = Consumer {
        kind: ConsumerKind::Consumer,
        id: consumer_id,
    };

    assert_no_stored_offset(&offset_client, &read_only_consumer, &stream_id, &topic_id).await?;

    let moving = moving_config(&config)?;
    let mut observer =
        IggyDlqDuplicateAlertObserver::connect_moving_window(&config, moving).await?;
    assert_eq!(
        observer.scan_mode(),
        IggyDlqDuplicateAlertScanMode::MovingWindow
    );
    assert!(observer.preserves_process_local_state_after_scan_error());

    let broker_message_id = broker_message_id_for_partition(1, PARTITION);
    let payload = vec![0xff, 0x00, 0x81, 0x01];

    publish_physical(&transport, broker_message_id, payload.clone(), 1).await?;
    let first = observer.summarize().await?;
    assert_first_summary(&first);
    assert_no_stored_offset(&offset_client, &read_only_consumer, &stream_id, &topic_id).await?;

    publish_physical(&transport, broker_message_id, payload, 2).await?;
    let second = observer.summarize().await?;
    assert_second_summary(&second);
    assert_no_stored_offset(&offset_client, &read_only_consumer, &stream_id, &topic_id).await?;

    let third = observer.summarize().await?;
    assert_eq!(third, second);
    assert_no_stored_offset(&offset_client, &read_only_consumer, &stream_id, &topic_id).await?;

    let replacement = moving_config(&config)?;
    let mut replacement_observer =
        IggyDlqDuplicateAlertObserver::connect_moving_window(&config, replacement).await?;
    let replacement_first = replacement_observer.summarize().await?;
    assert_eq!(replacement_first, first);
    assert_first_summary(&replacement_first);
    assert_no_stored_offset(&offset_client, &read_only_consumer, &stream_id, &topic_id).await?;

    println!("{EVIDENCE_MARKER}");

    drop(observer);
    drop(replacement_observer);
    offset_client.shutdown().await?;
    transport.shutdown().await?;
    Ok(())
}

fn moving_config(
    config: &IggyConfig,
) -> Result<IggyDlqDuplicateAlertMovingWindowConfig, rustok_iggy::IggyDlqDuplicateAlertObserverError>
{
    IggyDlqDuplicateAlertMovingWindowConfig::new(
        config,
        INITIAL_OFFSET,
        PER_PARTITION_MESSAGES,
        BATCH_SIZE,
        ROLLING_MAX_CYCLES,
        ROLLING_MAX_OBSERVATIONS_PER_CYCLE,
    )
}

fn assert_first_summary(summary: &DlqDuplicateSummary) {
    assert_eq!(summary.total_messages(), 1);
    assert_eq!(summary.unique_message_ids(), 1);
    assert_eq!(summary.duplicate_messages(), 0);
    assert_eq!(summary.duplicate_groups(), 0);
    assert_eq!(summary.conflicting_payload_groups(), 0);
    assert_eq!(summary.max_copies_per_message_id(), 1);
    assert!(!summary.has_physical_duplicates());
    assert!(!summary.has_identity_conflicts());
    assert!(!summary.requires_manual_review());
}

fn assert_second_summary(summary: &DlqDuplicateSummary) {
    assert_eq!(summary.total_messages(), 2);
    assert_eq!(summary.unique_message_ids(), 1);
    assert_eq!(summary.duplicate_messages(), 1);
    assert_eq!(summary.duplicate_groups(), 1);
    assert_eq!(summary.conflicting_payload_groups(), 0);
    assert_eq!(summary.max_copies_per_message_id(), 2);
    assert!(summary.has_physical_duplicates());
    assert!(!summary.has_identity_conflicts());
    assert!(!summary.requires_manual_review());
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

fn broker_message_id_for_partition(value: u128, expected_partition: u32) -> Uuid {
    assert!(value > 0);
    assert_eq!(expected_partition, PARTITION);
    let candidate = Uuid::from_u128(value);
    let selected = (candidate.as_u128() % u128::from(PARTITION_COUNT)) as u32 + 1;
    assert_eq!(selected, expected_partition);
    candidate
}

async fn assert_no_stored_offset(
    client: &IggyClient,
    consumer: &Consumer,
    stream_id: &Identifier,
    topic_id: &Identifier,
) -> TestResult<()> {
    let stored = client
        .get_consumer_offset(consumer, stream_id, topic_id, Some(PARTITION))
        .await?;
    if stored.is_some() {
        return Err(invalid_data(
            "read-only external moving observer unexpectedly stored a consumer offset",
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
        .ok_or_else(|| invalid_data("external moving observer evidence address is missing"))?;
    if config.protocol != "tcp" {
        return Err(invalid_data(
            "external moving observer evidence requires TCP",
        ));
    }
    if config.tls_enabled || config.tls_domain.is_some() || config.tls_ca_file.is_some() {
        return Err(invalid_data(
            "external moving observer source harness does not claim TLS coverage",
        ));
    }
    if config.username.is_empty() != config.password.is_empty() {
        return Err(invalid_data(
            "external moving observer username and password must both be set or both be empty",
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
            "external moving observer username and password must both be set or both be empty",
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
            stream_name: unique_name("moving-observer"),
            domain_partitions: PARTITION_COUNT,
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
