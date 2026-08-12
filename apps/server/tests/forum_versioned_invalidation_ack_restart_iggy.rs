use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use rustok_core::{MigrationSource, events::EventTransport};
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_iggy::{
    ConsumedContractEvent, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractConsumerGroup, PersistentContractDelivery, SerializationFormat,
    TopologyConfig,
};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC, ForumSearchContractIngress,
    ForumSearchContractIngressOutcome, SearchModule,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
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
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const EVIDENCE_CONTRACT: &str = "forum_search_versioned_invalidation_ack_restart_evidence_v1";
const EVIDENCE_PATH: &str = "target/forum-search-versioned-invalidation-ack-restart-evidence.json";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search Iggy acknowledgement/restart proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;
        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;

        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct InboxSnapshot {
    event_id: Uuid,
    tenant_id: Uuid,
    scope_key: String,
    event_type: String,
    ingest_sequence: i64,
    envelope_json: JsonValue,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct AckRestartEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    delivery_profile: &'static str,
    consumer_group: &'static str,
    stream: String,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
async fn durable_inbox_survives_failed_ack_and_consumer_restart() -> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!(
            "{IGGY_ADDRESS_ENV} is not set; skipping Forum Search Iggy acknowledgement/restart proof"
        );
        return Ok(());
    };
    let Some(test_db) = PostgresSearchTestDb::setup("forum_ack_restart").await? else {
        return Ok(());
    };

    let stream = config.topology.stream_name.clone();
    let proof = run_ack_restart_proof(&test_db.db, config).await;
    let cleanup = test_db.cleanup().await;
    let scenario = proof?;
    cleanup?;

    write_evidence(AckRestartEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D3",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        delivery_profile: "outbox_iggy",
        consumer_group: FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        stream,
        topic: FORUM_SEARCH_CONTRACT_TOPIC,
        scenario_results: vec![scenario],
    })?;

    Ok(())
}

async fn run_ack_restart_proof(
    db: &DatabaseConnection,
    config: IggyConfig,
) -> TestResult<ScenarioEvidence> {
    let tenant_id = Uuid::new_v4();
    let first_root_event_id = Uuid::new_v4();
    let second_root_event_id = Uuid::new_v4();
    let first_envelope = typed_invalidation(tenant_id, first_root_event_id, 1)?;
    let second_envelope = typed_invalidation(tenant_id, second_root_event_id, 2)?;
    let first_typed_envelope_id = first_envelope.id();
    let second_typed_envelope_id = second_envelope.id();

    let first_transport = IggyTransport::new(config.clone()).await?;
    let first_group = first_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;
    first_transport.publish_contract(first_envelope).await?;
    first_transport.publish_contract(second_envelope).await?;

    let first_delivery = receive_event(&first_group).await?;
    ensure_delivery_identity(
        &first_delivery,
        first_typed_envelope_id,
        first_root_event_id,
        tenant_id,
    )?;
    let first_offset = first_delivery
        .offset()
        .ok_or_else(|| invalid_data("first Iggy delivery has no broker offset"))?;
    let first_raw_payload = first_delivery.raw_payload().to_vec();

    let ingress = ForumSearchContractIngress::new(db.clone());
    ensure_durable_outcome(
        ingress.ingest(&first_delivery.envelope).await?,
        first_root_event_id,
        1,
    )?;
    let before_restart = load_snapshot(db, first_root_event_id).await?;
    ensure_snapshot(&before_restart, tenant_id, first_root_event_id)?;
    if count_event_rows(db, first_root_event_id).await? != 1 {
        return Err(invalid_data(
            "first typed delivery must create exactly one durable Search inbox row",
        )
        .into());
    }

    let mut rejected_ack_delivery = first_delivery.clone();
    let exact_ack_token = rejected_ack_delivery
        .ack_token()
        .ok_or_else(|| invalid_data("first Iggy delivery has no acknowledgement token"))?
        .to_string();
    rejected_ack_delivery.connector_metadata.ack_token =
        Some(format!("{exact_ack_token}-injected-failure"));
    let acknowledgement_error = first_group
        .acknowledge(&rejected_ack_delivery)
        .await
        .expect_err("mismatched acknowledgement token must fail before offset commit");
    let acknowledgement_error_text = acknowledgement_error.to_string();
    if !acknowledgement_error_text
        .contains("ack token does not match the outstanding Iggy consumer-group delivery")
    {
        return Err(invalid_data(format!(
            "unexpected injected acknowledgement failure: {acknowledgement_error_text}"
        ))
        .into());
    }

    drop(first_group);
    first_transport.shutdown().await?;
    drop(first_transport);

    let restarted_transport = IggyTransport::new(config).await?;
    let restarted_group = restarted_transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;

    let restarted_proof = async {
        let redelivered = receive_event(&restarted_group).await?;
        ensure_delivery_identity(
            &redelivered,
            first_typed_envelope_id,
            first_root_event_id,
            tenant_id,
        )?;
        let redelivered_offset = redelivered
            .offset()
            .ok_or_else(|| invalid_data("redelivered Iggy event has no broker offset"))?;
        if redelivered_offset != first_offset {
            return Err(invalid_data(format!(
                "restart returned offset {redelivered_offset} instead of uncommitted offset {first_offset}"
            ))
            .into());
        }
        if redelivered.raw_payload() != first_raw_payload.as_slice() {
            return Err(invalid_data(
                "restart changed the exact typed-event broker payload bytes",
            )
            .into());
        }

        ensure_durable_outcome(
            ingress.ingest(&redelivered.envelope).await?,
            first_root_event_id,
            1,
        )?;
        let after_restart = load_snapshot(db, first_root_event_id).await?;
        if after_restart != before_restart {
            return Err(invalid_data(
                "redelivery replaced the complete durable Search inbox identity",
            )
            .into());
        }
        if count_event_rows(db, first_root_event_id).await? != 1 {
            return Err(invalid_data(
                "redelivery created a second Search inbox row for the same root identity",
            )
            .into());
        }

        restarted_group.acknowledge(&redelivered).await?;

        let next_delivery = receive_event(&restarted_group).await?;
        ensure_delivery_identity(
            &next_delivery,
            second_typed_envelope_id,
            second_root_event_id,
            tenant_id,
        )?;
        let next_offset = next_delivery
            .offset()
            .ok_or_else(|| invalid_data("second Iggy event has no broker offset"))?;
        if next_offset <= first_offset {
            return Err(invalid_data(format!(
                "successful restart acknowledgement did not advance the consumer group: first={first_offset}, next={next_offset}"
            ))
            .into());
        }
        ensure_durable_outcome(
            ingress.ingest(&next_delivery.envelope).await?,
            second_root_event_id,
            2,
        )?;
        let second_snapshot = load_snapshot(db, second_root_event_id).await?;
        ensure_snapshot(&second_snapshot, tenant_id, second_root_event_id)?;
        if second_snapshot.ingest_sequence <= after_restart.ingest_sequence {
            return Err(invalid_data(format!(
                "second inbox row did not receive a later ingest sequence: first={}, second={}",
                after_restart.ingest_sequence, second_snapshot.ingest_sequence
            ))
            .into());
        }
        restarted_group.acknowledge(&next_delivery).await?;

        Ok::<ScenarioEvidence, Box<dyn Error + Send + Sync>>(ScenarioEvidence {
            id: "acknowledgement_failure_restart",
            result: "passed",
            facts: json!({
                "tenant_id": tenant_id,
                "consumer_group": FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
                "first_root_event_id": first_root_event_id,
                "first_typed_envelope_id": first_typed_envelope_id,
                "first_offset": first_offset,
                "redelivered_offset": redelivered_offset,
                "first_ingest_sequence_before_restart": before_restart.ingest_sequence,
                "first_ingest_sequence_after_restart": after_restart.ingest_sequence,
                "first_inbox_rows_after_redelivery": 1,
                "ack_failure_classification": "ack_token_consumer_identity_mismatch",
                "next_root_event_id": second_root_event_id,
                "next_typed_envelope_id": second_typed_envelope_id,
                "next_offset": next_offset,
                "next_ingest_sequence": second_snapshot.ingest_sequence,
                "restart_acknowledgement_advanced_group": true
            }),
        })
    }
    .await;

    let shutdown = restarted_transport.shutdown().await;
    let scenario = restarted_proof?;
    shutdown?;
    Ok(scenario)
}

fn typed_invalidation(
    tenant_id: Uuid,
    root_event_id: Uuid,
    owner_revision: i64,
) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_caused_by(
        tenant_id,
        None,
        root_event_id,
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision,
            target_type: "forum".to_string(),
            target_id: None,
        },
    )?)
}

async fn receive_event(
    group: &PersistentContractConsumerGroup,
) -> TestResult<ConsumedContractEvent> {
    let delivery = timeout(RECEIVE_TIMEOUT, group.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for a Forum Search Iggy delivery"))??
        .ok_or_else(|| invalid_data("Iggy consumer group ended before a delivery"))?;

    match delivery {
        PersistentContractDelivery::Event(consumed) => Ok(*consumed),
        PersistentContractDelivery::DecodeFailure(failure) => Err(invalid_data(format!(
            "published Forum Search contract event decoded as poison: {}",
            failure.stable_error_code()
        ))
        .into()),
    }
}

fn ensure_delivery_identity(
    delivery: &ConsumedContractEvent,
    expected_typed_envelope_id: Uuid,
    expected_root_event_id: Uuid,
    expected_tenant_id: Uuid,
) -> TestResult<()> {
    if delivery.topic != FORUM_SEARCH_CONTRACT_TOPIC
        || delivery.envelope.id() != expected_typed_envelope_id
        || delivery.envelope.causation_id() != Some(expected_root_event_id)
        || delivery.envelope.tenant_id() != expected_tenant_id
        || delivery.envelope.event_type() != "forum.search_projection.invalidation_issued"
        || delivery.envelope.schema_version() != 1
        || delivery.offset().is_none()
        || delivery.ack_token().is_none()
        || delivery.raw_payload().is_empty()
    {
        return Err(invalid_data(format!(
            "unexpected Forum Search Iggy delivery identity: {delivery:?}"
        ))
        .into());
    }
    delivery.validate_connector_metadata()?;
    delivery.envelope.validate_registered_schema()?;
    Ok(())
}

fn ensure_durable_outcome(
    outcome: ForumSearchContractIngressOutcome,
    expected_root_event_id: Uuid,
    expected_owner_revision: i64,
) -> TestResult<()> {
    match outcome {
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id,
            owner_revision,
        } if root_event_id == expected_root_event_id
            && owner_revision == expected_owner_revision =>
        {
            Ok(())
        }
        other => Err(invalid_data(format!(
            "unexpected Forum Search durable ingress outcome: {other:?}"
        ))
        .into()),
    }
}

async fn load_snapshot(db: &DatabaseConnection, event_id: Uuid) -> TestResult<InboxSnapshot> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT event_id, tenant_id, scope_key, event_type,
                   ingest_sequence, envelope_json
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| invalid_data(format!("Search inbox row {event_id} was not found")))?;

    Ok(InboxSnapshot {
        event_id: row.try_get("", "event_id")?,
        tenant_id: row.try_get("", "tenant_id")?,
        scope_key: row.try_get("", "scope_key")?,
        event_type: row.try_get("", "event_type")?,
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        envelope_json: row.try_get("", "envelope_json")?,
    })
}

fn ensure_snapshot(
    snapshot: &InboxSnapshot,
    expected_tenant_id: Uuid,
    expected_root_event_id: Uuid,
) -> TestResult<()> {
    let stored_envelope: EventEnvelope = serde_json::from_value(snapshot.envelope_json.clone())?;
    let expected_event = DomainEvent::ReindexRequested {
        target_type: "forum".to_string(),
        target_id: None,
    };
    if snapshot.event_id != expected_root_event_id
        || snapshot.tenant_id != expected_tenant_id
        || snapshot.scope_key != "forum"
        || snapshot.event_type != ROOT_EVENT_TYPE
        || snapshot.ingest_sequence <= 0
        || stored_envelope.id != expected_root_event_id
        || stored_envelope.tenant_id != expected_tenant_id
        || stored_envelope.event_type != ROOT_EVENT_TYPE
        || stored_envelope.schema_version != 1
        || stored_envelope.correlation_id != expected_root_event_id
        || stored_envelope.causation_id.is_some()
        || stored_envelope.event != expected_event
    {
        return Err(invalid_data(format!(
            "unexpected durable Search inbox snapshot: {snapshot:?}"
        ))
        .into());
    }
    stored_envelope.validate_registered_schema()?;
    Ok(())
}

async fn count_event_rows(db: &DatabaseConnection, event_id: Uuid) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_projection_inbox WHERE event_id = $1",
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| invalid_data("Search inbox count query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn external_test_config() -> TestResult<Option<IggyConfig>> {
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

fn postgres_database_url() -> Option<String> {
    env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
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

fn write_evidence(artifact: AckRestartEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
    eprintln!(
        "wrote Forum Search Iggy acknowledgement/restart evidence to {}",
        path.display()
    );
    Ok(())
}

fn source_commit() -> TestResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root())
        .output()?;
    if !output.status.success() {
        return Err(invalid_data(format!(
            "git rev-parse HEAD failed with status {}",
            output.status
        ))
        .into());
    }
    let commit = String::from_utf8(output.stdout)?.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_data(format!(
            "git rev-parse HEAD returned an invalid source commit: {commit}"
        ))
        .into());
    }
    Ok(commit)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
