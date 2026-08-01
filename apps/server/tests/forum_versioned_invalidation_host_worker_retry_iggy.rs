use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use rustok_core::{
    MigrationSource,
    events::{EventTransport, MemoryTransport},
};
use rustok_events::{ContractEventEnvelope, ForumSearchProjectionEvent};
use rustok_iggy::{
    ExternalConfig, IggyConfig, IggyMode, IggyTransport, PersistentContractDelivery,
    SerializationFormat, TopologyConfig,
};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC, SearchModule,
};
use rustok_server::common::settings::{EventDeliveryProfile, RustokSettings};
use rustok_server::services::app_lifecycle::StopHandle;
use rustok_server::services::event_transport_factory::EventRuntime;
use rustok_server::services::forum_search_inbox_worker::{
    ForumSearchContractConsumerWorkerHandle, start_forum_search_contract_consumer_if_enabled,
};
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use serial_test::serial;
use tokio::time::timeout;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const IGGY_ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";
const IGGY_USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";
const IGGY_PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";
const ENABLE_ENV: &str = "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED";
const IDLE_POLL_ENV: &str = "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_IDLE_POLL_MS";
const CONFIGURED_MAX_ATTEMPTS: i32 = 3;
const CONFIGURED_IDLE_POLL_MS: u64 = 5_000;
const WORKER_STATE_TIMEOUT: Duration = Duration::from_secs(20);
const ROW_POLL_TIMEOUT: Duration = Duration::from_secs(20);
const EMPTY_GROUP_TIMEOUT: Duration = Duration::from_millis(750);
const STOP_TIMEOUT: Duration = Duration::from_secs(1);
const EVIDENCE_CONTRACT: &str =
    "forum_search_versioned_invalidation_host_worker_retry_evidence_v1";
const EVIDENCE_PATH: &str =
    "target/forum-search-versioned-invalidation-host-worker-retry-evidence.json";
const RETRY_SEQUENCE: &str = "forum_search_worker_retry_attempts";
const FAILURE_FUNCTION: &str = "forum_search_fail_inbox_insert";
const FAILURE_TRIGGER: &str = "forum_search_fail_inbox_insert";

struct PostgresSearchEvidence {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchEvidence {
    async fn setup(scope: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum Search host-worker retry proof"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_forum_worker_{}_{}",
            sanitize_identifier(scope),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;
        let setup = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            install_retry_failure(&db).await
        }
        .await;
        if let Err(error) = setup {
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

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = env::var_os(key);
        // SAFETY: this integration target is serialized and restores every changed variable.
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: this integration target is serialized and restores the prior process value.
        unsafe {
            match self.original.take() {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: &'static str,
    result: &'static str,
    facts: JsonValue,
}

#[derive(Debug, Serialize)]
struct HostWorkerEvidenceArtifact {
    contract: &'static str,
    task: &'static str,
    source_commit: String,
    generated_at: String,
    database_backend: &'static str,
    broker_backend: &'static str,
    delivery_profile: &'static str,
    consumer_group: &'static str,
    stream: String,
    topic: &'static str,
    scenario_results: Vec<ScenarioEvidence>,
}

#[tokio::test]
#[serial]
async fn host_worker_exhausts_retry_then_recovers_redelivery_and_stops_promptly() -> TestResult<()> {
    let Some(config) = external_test_config()? else {
        eprintln!(
            "{IGGY_ADDRESS_ENV} is not set; skipping Forum Search host-worker retry proof"
        );
        return Ok(());
    };
    let Some(database) = PostgresSearchEvidence::setup("retry_lifecycle").await? else {
        return Ok(());
    };

    let _enable = EnvVarGuard::set(ENABLE_ENV, "true");
    let _idle_poll = EnvVarGuard::set(IDLE_POLL_ENV, &CONFIGURED_IDLE_POLL_MS.to_string());

    let stream = config.topology.stream_name.clone();
    let transport = Arc::new(IggyTransport::new(config).await?);
    let context = worker_context(database.db.clone(), Arc::clone(&transport));
    let proof = run_host_worker_proof(&context, &database.db, Arc::clone(&transport)).await;

    if let Some(stop) = context.shared_get::<StopHandle>() {
        stop.stop().await;
    }
    let shutdown = transport.shutdown().await;
    let cleanup = database.cleanup().await;
    let scenario = proof?;
    shutdown?;
    cleanup?;

    write_evidence(HostWorkerEvidenceArtifact {
        contract: EVIDENCE_CONTRACT,
        task: "FORUM-23B2G2B3D8",
        source_commit: source_commit()?,
        generated_at: Utc::now().to_rfc3339(),
        database_backend: "postgresql",
        broker_backend: "external_iggy",
        delivery_profile: "outbox_iggy",
        consumer_group: FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        stream,
        topic: FORUM_SEARCH_CONTRACT_TOPIC,
        scenario_results: vec![scenario],
    })?;

    Ok(())
}

async fn run_host_worker_proof(
    context: &ServerRuntimeContext,
    db: &DatabaseConnection,
    transport: Arc<IggyTransport>,
) -> TestResult<ScenarioEvidence> {
    let tenant_id = Uuid::new_v4();
    let deliveries = (1_i64..=4)
        .map(|owner_revision| {
            let root_event_id = Uuid::new_v4();
            Ok((
                root_event_id,
                ContractEventEnvelope::new_caused_by(
                    tenant_id,
                    None,
                    root_event_id,
                    ForumSearchProjectionEvent::InvalidationIssued {
                        owner_revision,
                        target_type: "forum".to_string(),
                        target_id: None,
                    },
                )?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;

    for (_, envelope) in &deliveries {
        transport.publish_contract(envelope.clone()).await?;
    }

    start_forum_search_contract_consumer_if_enabled(context).await?;
    let first_instance_id = worker_instance_id(context)?;
    wait_for_worker_finished(context).await?;
    let observed_retry_attempts = retry_attempt_count(db).await?;
    if observed_retry_attempts != i64::from(CONFIGURED_MAX_ATTEMPTS) {
        return Err(invalid_data(format!(
            "host worker attempted retryable ingress {observed_retry_attempts} times instead of {CONFIGURED_MAX_ATTEMPTS}"
        ))
        .into());
    }
    if inbox_row_count(db).await? != 0 {
        return Err(invalid_data(
            "retry exhaustion unexpectedly committed a Search inbox row",
        )
        .into());
    }

    context
        .shared_take::<ForumSearchContractConsumerWorkerHandle>()
        .ok_or_else(|| invalid_data("finished Forum Search worker handle was not removable"))?;
    remove_retry_failure(db).await?;

    start_forum_search_contract_consumer_if_enabled(context).await?;
    let second_instance_id = worker_instance_id(context)?;
    if second_instance_id == first_instance_id {
        return Err(invalid_data(
            "Forum Search worker restart reused the previous lifecycle instance ID",
        )
        .into());
    }

    let root_event_ids = deliveries
        .iter()
        .map(|(root_event_id, _)| *root_event_id)
        .collect::<Vec<_>>();
    wait_for_exact_inbox_rows(db, &root_event_ids).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    if worker_finished(context)? {
        return Err(invalid_data(
            "restarted Forum Search worker stopped before the idle lifecycle check",
        )
        .into());
    }

    let stop = context
        .shared_get::<StopHandle>()
        .ok_or_else(|| invalid_data("Forum Search worker did not publish StopHandle"))?;
    let stop_started = Instant::now();
    stop.stop().await;
    timeout(STOP_TIMEOUT, wait_for_worker_finished(context))
        .await
        .map_err(|_| {
            invalid_data(
                "Forum Search worker did not stop within the bounded idle-poll preemption window",
            )
        })??;
    let stop_elapsed_ms = u64::try_from(stop_started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if stop_elapsed_ms >= CONFIGURED_IDLE_POLL_MS {
        return Err(invalid_data(
            "Forum Search worker shutdown waited for the full configured idle poll",
        )
        .into());
    }

    context
        .shared_take::<ForumSearchContractConsumerWorkerHandle>()
        .ok_or_else(|| invalid_data("stopped Forum Search worker handle was not removable"))?;
    assert_consumer_group_empty(&transport).await?;

    Ok(ScenarioEvidence {
        id: "host_worker_retry_exhaustion_restart_and_stop",
        result: "passed",
        facts: json!({
            "tenant_id": tenant_id,
            "first_worker_instance_id": first_instance_id,
            "second_worker_instance_id": second_instance_id,
            "configured_max_attempts": CONFIGURED_MAX_ATTEMPTS,
            "observed_retry_attempts": observed_retry_attempts,
            "rows_after_retry_exhaustion": 0,
            "rows_after_restart": root_event_ids.len(),
            "root_event_ids": root_event_ids,
            "consumer_group_empty_after_shutdown": true,
            "configured_idle_poll_ms": CONFIGURED_IDLE_POLL_MS,
            "stop_elapsed_ms": stop_elapsed_ms,
            "stop_preempted_idle_poll": true
        }),
    })
}

fn worker_context(
    db: DatabaseConnection,
    iggy_transport: Arc<IggyTransport>,
) -> ServerRuntimeContext {
    let mut settings = RustokSettings::default();
    settings.events.delivery_profile = EventDeliveryProfile::OutboxIggy;
    settings.events.dlq.enabled = false;
    settings.events.relay_retry_policy.max_attempts = CONFIGURED_MAX_ATTEMPTS;
    settings.events.relay_retry_policy.base_backoff_ms = 25;
    settings.events.relay_retry_policy.max_backoff_ms = 50;

    let context = ServerRuntimeContext::new(db, settings);
    let local_transport = MemoryTransport::with_capacity(16);
    let listener_bus = local_transport.event_bus();
    let event_transport: Arc<dyn EventTransport> = Arc::new(local_transport);
    context.shared_insert(Arc::new(EventRuntime {
        delivery_profile: EventDeliveryProfile::OutboxIggy,
        iggy_mode: Some(IggyMode::External),
        transport: event_transport,
        listener_bus,
        relay_config: None,
        channel_capacity: 16,
        relay_fallback_active: false,
    }));
    context.shared_insert(iggy_transport);
    context
}

async fn install_retry_failure(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
        CREATE SEQUENCE {RETRY_SEQUENCE} START WITH 1;
        CREATE FUNCTION {FAILURE_FUNCTION}() RETURNS trigger
        LANGUAGE plpgsql AS $$
        BEGIN
            PERFORM nextval('{RETRY_SEQUENCE}');
            RAISE EXCEPTION 'injected Forum Search host-worker retry failure'
                USING ERRCODE = '40001';
        END;
        $$;
        CREATE TRIGGER {FAILURE_TRIGGER}
        BEFORE INSERT ON search_projection_inbox
        FOR EACH ROW EXECUTE FUNCTION {FAILURE_FUNCTION}();
        "#
    ))
    .await?;
    Ok(())
}

async fn remove_retry_failure(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
        DROP TRIGGER {FAILURE_TRIGGER} ON search_projection_inbox;
        DROP FUNCTION {FAILURE_FUNCTION}();
        "#
    ))
    .await?;
    Ok(())
}

async fn retry_attempt_count(db: &DatabaseConnection) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!("SELECT last_value::BIGINT AS value FROM {RETRY_SEQUENCE}"),
        ))
        .await?
        .ok_or_else(|| invalid_data("retry-attempt sequence query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn inbox_row_count(db: &DatabaseConnection) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::BIGINT AS value FROM search_projection_inbox".to_string(),
        ))
        .await?
        .ok_or_else(|| invalid_data("Search inbox count query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn wait_for_exact_inbox_rows(
    db: &DatabaseConnection,
    root_event_ids: &[Uuid],
) -> TestResult<()> {
    timeout(ROW_POLL_TIMEOUT, async {
        loop {
            let mut all_present = true;
            for root_event_id in root_event_ids {
                let row = db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COUNT(*)::BIGINT AS value FROM search_projection_inbox WHERE event_id = $1",
                        vec![(*root_event_id).into()],
                    ))
                    .await?
                    .ok_or_else(|| invalid_data("Search inbox identity count returned no row"))?;
                let count: i64 = row.try_get("", "value")?;
                if count != 1 {
                    all_present = false;
                    break;
                }
            }
            if all_present {
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(|_| invalid_data("timed out waiting for restarted worker inbox rows"))??;
    Ok(())
}

fn worker_instance_id(context: &ServerRuntimeContext) -> TestResult<u64> {
    context
        .shared_map::<ForumSearchContractConsumerWorkerHandle, _>(|handle| handle.instance_id())
        .ok_or_else(|| invalid_data("Forum Search worker handle is unavailable").into())
}

fn worker_finished(context: &ServerRuntimeContext) -> TestResult<bool> {
    context
        .shared_map::<ForumSearchContractConsumerWorkerHandle, _>(|handle| handle.is_finished())
        .ok_or_else(|| invalid_data("Forum Search worker handle is unavailable").into())
}

async fn wait_for_worker_finished(context: &ServerRuntimeContext) -> TestResult<()> {
    timeout(WORKER_STATE_TIMEOUT, async {
        loop {
            if worker_finished(context)? {
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(|_| invalid_data("timed out waiting for Forum Search worker to finish"))??;
    Ok(())
}

async fn assert_consumer_group_empty(transport: &IggyTransport) -> TestResult<()> {
    let group = transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await?;
    match timeout(EMPTY_GROUP_TIMEOUT, group.receive_delivery()).await {
        Err(_) | Ok(Ok(None)) => Ok(()),
        Ok(Ok(Some(PersistentContractDelivery::Event(event)))) => Err(invalid_data(format!(
            "Forum Search consumer group retained typed event {} after worker shutdown",
            event.envelope.id()
        ))
        .into()),
        Ok(Ok(Some(PersistentContractDelivery::DecodeFailure(failure)))) => Err(invalid_data(
            format!(
                "Forum Search consumer group retained poison {} after worker shutdown",
                failure.delivery_id()
            ),
        )
        .into()),
        Ok(Err(error)) => Err(error.into()),
    }
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
            stream_name: unique_name("host-worker-retry"),
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
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
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

fn write_evidence(artifact: HostWorkerEvidenceArtifact) -> TestResult<()> {
    let path = workspace_root().join(EVIDENCE_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("evidence path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
    eprintln!(
        "wrote Forum Search host-worker retry evidence to {}",
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
        return Err(invalid_data("git rev-parse HEAD failed for evidence generation").into());
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
