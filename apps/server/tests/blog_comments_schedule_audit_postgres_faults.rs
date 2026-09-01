use std::{
    io,
    process::{Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use rustok_api::AuthPrincipalKind;
use rustok_migrations::Migrator;
use rustok_server::services::comments_provider_runtime::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
    CommentsTcpDelegationSchedulePersistenceDocument, CommentsTcpDelegationSchedulePersistenceKey,
    CommentsTcpDelegationSchedulePersistenceStartupMode,
    CommentsTcpDelegationScheduleTriggerAuthorizationError,
    CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    CommentsTcpDelegationScheduleTriggerAuthorizer, CommentsTcpDelegationScheduleTriggerContext,
    PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
    SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger,
    SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::MigratorTrait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::oneshot,
    task::JoinHandle,
};
use url::Url;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_DATABASE_URL";
const CHILD_REQUEST_ID_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_REQUEST_ID";
const CHILD_ACTOR_ID_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_ACTOR_ID";
const CHILD_PRIMARY_ACTIVATION_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_PRIMARY_ACTIVATION_MS";
const CHILD_SUCCESSOR_ACTIVATION_ENV: &str =
    "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_SUCCESSOR_ACTIVATION_MS";
const CHILD_PRIMARY_RETIREMENT_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_PRIMARY_RETIREMENT_MS";
const CHILD_ROLE_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_FAULT_CHILD";
const CHILD_TEST_NAME: &str = "blog_comments_schedule_audit_fault_child";

const PROPAGATION_BUDGET_MS: u64 = 1_000;
const MAX_TTL_MS: u64 = 5_000;
const SUCCESSOR_DELAY_MS: u64 = 120_000;
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const PROXY_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const WORKER_SETTLE_DELAY: Duration = Duration::from_millis(200);
const MAX_POSTGRES_PACKET_BYTES: usize = 1024 * 1024;
const POSTGRES_SSL_REQUEST_CODE: u32 = 80_877_103;
const POSTGRES_GSSENC_REQUEST_CODE: u32 = 80_877_104;
const THIRD_STATE_DIGEST_HEX: &str =
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const SECRET_A: &str = "comments-audit-fault-secret-a-000000000001";
const SECRET_B: &str = "comments-audit-fault-secret-b-000000000002";

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitAckFaultMode {
    ExactPair,
    ThirdState,
    UnreadableReconciliation,
}

impl CommitAckFaultMode {
    fn database_prefix(self) -> &'static str {
        match self {
            Self::ExactPair => "rustok_blog_comments_ack_exact",
            Self::ThirdState => "rustok_blog_comments_ack_third",
            Self::UnreadableReconciliation => "rustok_blog_comments_ack_unreadable",
        }
    }

    fn expects_success(self) -> bool {
        matches!(self, Self::ExactPair)
    }
}

#[derive(Clone, Copy)]
struct ScheduleAnchor {
    primary_activation_ms: u64,
    successor_activation_ms: u64,
    primary_retirement_ms: u64,
}

#[derive(Debug)]
struct StateRow {
    generation: i64,
    digest_hex: String,
}

#[derive(Debug)]
struct AuditRow {
    request_id: Uuid,
    previous_generation: i64,
    candidate_generation: i64,
    unpublished: bool,
}

struct AllowAuthorizer;

impl CommentsTcpDelegationScheduleTriggerAuthorizer for AllowAuthorizer {
    fn authorize(
        &self,
        _request: &CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    ) -> Result<(), CommentsTcpDelegationScheduleTriggerAuthorizationError> {
        Ok(())
    }
}

struct ProxyShared {
    upstream_host: String,
    upstream_port: u16,
    mode: CommitAckFaultMode,
    fault_consumed: AtomicBool,
    mutation_db: DatabaseConnection,
}

struct CommitAckProxy {
    address: std::net::SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<io::Result<()>>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn commit_ack_loss_exact_pair_reconciles_successfully() {
    run_fault_scenario(CommitAckFaultMode::ExactPair)
        .await
        .unwrap_or_else(|error| {
            panic!("exact-pair commit reconciliation evidence failed: {error}")
        });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn commit_ack_loss_third_state_fail_stops() {
    run_fault_scenario(CommitAckFaultMode::ThirdState)
        .await
        .unwrap_or_else(|error| {
            panic!("third-state commit reconciliation evidence failed: {error}")
        });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn commit_ack_loss_unreadable_reconciliation_fail_stops() {
    run_fault_scenario(CommitAckFaultMode::UnreadableReconciliation)
        .await
        .unwrap_or_else(|error| {
            panic!("unreadable commit reconciliation evidence failed: {error}")
        });
}

#[test]
#[ignore = "subprocess entry point for audited PostgreSQL fault harness"]
fn blog_comments_schedule_audit_fault_child() {
    if std::env::var(CHILD_ROLE_ENV).as_deref() != Ok("replace") {
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build Comments audited fault child runtime");
    runtime
        .block_on(run_fault_child())
        .unwrap_or_else(|error| panic!("Comments audited fault child failed: {error}"));
}

async fn run_fault_scenario(mode: CommitAckFaultMode) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name(mode.database_prefix());
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("PostgreSQL admin database must be reachable: {error}"))?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let scenario_result = async {
        let database = connect_postgres(&target_url).await?;
        Migrator::up(&database, None).await?;

        let anchor = schedule_anchor()?;
        bootstrap_generation_one(database.clone(), anchor)?;
        tokio::time::sleep(WORKER_SETTLE_DELAY).await;

        let request_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let proxy = CommitAckProxy::start(&target_url, mode, database.clone()).await?;
        let proxy_url = proxy_database_url(&target_url, proxy.address)?;

        let child_result = spawn_fault_child(&proxy_url, request_id, actor_id, anchor).await;
        let proxy_stop_result = proxy.stop().await;
        proxy_stop_result?;
        let output = child_result?;

        if mode.expects_success() {
            require_child_success(&output)?;
        } else {
            require_child_abort(&output)?;
        }

        let state = read_state(&database).await?;
        let audit = read_audit(&database, request_id)
            .await?
            .ok_or("fault child did not retain its durable outbox row")?;
        if audit.request_id != request_id
            || audit.previous_generation != 1
            || audit.candidate_generation != 2
            || !audit.unpublished
        {
            return Err(format!("fault child retained an invalid audit row: {audit:?}").into());
        }
        if count_outbox(&database).await? != 1 {
            return Err("fault scenario must retain exactly one outbox row".into());
        }

        match mode {
            CommitAckFaultMode::ExactPair | CommitAckFaultMode::UnreadableReconciliation => {
                if state.generation != 2 || state.digest_hex == THIRD_STATE_DIGEST_HEX {
                    return Err(
                        format!("expected committed generation-2 pair, found {state:?}").into(),
                    );
                }
            }
            CommitAckFaultMode::ThirdState => {
                if state.generation != 3 || state.digest_hex != THIRD_STATE_DIGEST_HEX {
                    return Err(format!(
                        "third-state injector did not advance the state row: {state:?}"
                    )
                    .into());
                }
            }
        }

        tokio::time::sleep(WORKER_SETTLE_DELAY).await;
        database.close().await?;
        Ok(())
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    scenario_result
}

async fn run_fault_child() -> TestResult<()> {
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let request_id = Uuid::parse_str(&required_env(CHILD_REQUEST_ID_ENV)?)?;
    let actor_id = Uuid::parse_str(&required_env(CHILD_ACTOR_ID_ENV)?)?;
    let anchor = ScheduleAnchor {
        primary_activation_ms: required_env(CHILD_PRIMARY_ACTIVATION_ENV)?.parse()?,
        successor_activation_ms: required_env(CHILD_SUCCESSOR_ACTIVATION_ENV)?.parse()?,
        primary_retirement_ms: required_env(CHILD_PRIMARY_RETIREMENT_ENV)?.parse()?,
    };

    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_millis(500))
        .acquire_timeout(Duration::from_millis(500))
        .sqlx_logging(false);
    let database = Database::connect(options).await?;

    let initial = schedule_document(1, anchor, false)?;
    let replacement = schedule_document(2, anchor, true)?;
    let trigger = audited_trigger(
        database.clone(),
        initial,
        CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact,
    )?;
    let outcome =
        trigger.replace_host_schedule(trigger_context(request_id, actor_id)?, replacement)?;
    if outcome.previous_generation != 1
        || outcome.current.generation != 2
        || trigger.current_selection()?.generation != 2
    {
        return Err(
            format!("exact-pair reconciliation returned an invalid outcome: {outcome:?}").into(),
        );
    }

    drop(trigger);
    tokio::time::sleep(WORKER_SETTLE_DELAY).await;
    database.close().await?;
    Ok(())
}

fn bootstrap_generation_one(
    database: DatabaseConnection,
    anchor: ScheduleAnchor,
) -> TestResult<()> {
    let trigger = audited_trigger(
        database,
        schedule_document(1, anchor, false)?,
        CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty,
    )?;
    if trigger.current_selection()?.generation != 1 {
        return Err("bootstrap did not publish generation 1".into());
    }
    drop(trigger);
    Ok(())
}

fn audited_trigger(
    database: DatabaseConnection,
    document: CommentsTcpDelegationSchedulePersistenceDocument,
    startup_mode: CommentsTcpDelegationSchedulePersistenceStartupMode,
) -> TestResult<SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger> {
    let store = PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore::new(database)?;
    Ok(
        SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_host_document(
            document,
            Duration::from_millis(MAX_TTL_MS),
            shared_authorizer(),
            store,
            startup_mode,
            32,
        )?,
    )
}

fn shared_authorizer() -> SharedCommentsTcpDelegationScheduleTriggerAuthorizer {
    Arc::new(AllowAuthorizer)
}

fn trigger_context(
    request_id: Uuid,
    actor_id: Uuid,
) -> TestResult<CommentsTcpDelegationScheduleTriggerContext> {
    Ok(CommentsTcpDelegationScheduleTriggerContext::new(
        request_id,
        actor_id,
        AuthPrincipalKind::Service,
    )?)
}

fn schedule_anchor() -> TestResult<ScheduleAnchor> {
    let now = unix_ms()?;
    let successor_activation_ms = now
        .checked_add(SUCCESSOR_DELAY_MS)
        .ok_or("successor activation overflow")?;
    let primary_retirement_ms = successor_activation_ms
        .checked_add(PROPAGATION_BUDGET_MS)
        .and_then(|value| value.checked_add(MAX_TTL_MS))
        .and_then(|value| {
            value.checked_add(rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS)
        })
        .and_then(|value| value.checked_add(1_000))
        .ok_or("primary retirement overflow")?;
    Ok(ScheduleAnchor {
        primary_activation_ms: now.saturating_sub(60_000).max(1),
        successor_activation_ms,
        primary_retirement_ms,
    })
}

fn schedule_document(
    generation: u64,
    anchor: ScheduleAnchor,
    include_successor: bool,
) -> TestResult<CommentsTcpDelegationSchedulePersistenceDocument> {
    let primary = CommentsTcpDelegationSchedulePersistenceKey::new(
        "audit-fault-key-a",
        SECRET_A,
        anchor.primary_activation_ms,
        include_successor.then_some(anchor.primary_retirement_ms),
    )?;
    let mut keys = vec![primary];
    if include_successor {
        keys.push(CommentsTcpDelegationSchedulePersistenceKey::new(
            "audit-fault-key-b",
            SECRET_B,
            anchor.successor_activation_ms,
            None,
        )?);
    }
    Ok(CommentsTcpDelegationSchedulePersistenceDocument::new(
        generation,
        Duration::from_millis(PROPAGATION_BUDGET_MS),
        keys,
        None,
    )?)
}

impl CommitAckProxy {
    async fn start(
        target_url: &str,
        mode: CommitAckFaultMode,
        mutation_db: DatabaseConnection,
    ) -> TestResult<Self> {
        let parsed = Url::parse(target_url)?;
        let upstream_host = parsed
            .host_str()
            .ok_or("PostgreSQL target URL is missing a host")?
            .to_string();
        let upstream_port = parsed
            .port_or_known_default()
            .ok_or("PostgreSQL target URL is missing a port")?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let shared = Arc::new(ProxyShared {
            upstream_host,
            upstream_port,
            mode,
            fault_consumed: AtomicBool::new(false),
            mutation_db,
        });
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let task = tokio::spawn(run_proxy(listener, shared, shutdown_receiver));
        Ok(Self {
            address,
            shutdown: Some(shutdown_sender),
            task,
        })
    }

    async fn stop(mut self) -> TestResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let proxy_result = tokio::time::timeout(PROXY_STOP_TIMEOUT, &mut self.task)
            .await
            .map_err(|_| "PostgreSQL fault proxy did not stop")??;
        proxy_result?;
        Ok(())
    }
}

async fn run_proxy(
    listener: TcpListener,
    shared: Arc<ProxyShared>,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let (client, _) = accepted?;
                if shared.mode
                    == CommitAckFaultMode::UnreadableReconciliation
                    && shared.fault_consumed.load(Ordering::Acquire)
                {
                    drop(client);
                    continue;
                }
                let connection_shared = Arc::clone(&shared);
                tokio::spawn(async move {
                    let _ =
                        proxy_connection(client, connection_shared).await;
                });
            }
        }
    }
}

async fn proxy_connection(mut client: TcpStream, shared: Arc<ProxyShared>) -> io::Result<()> {
    let mut upstream =
        TcpStream::connect((shared.upstream_host.as_str(), shared.upstream_port)).await?;
    relay_startup(&mut client, &mut upstream).await?;

    let (client_reader, client_writer) = client.into_split();
    let (upstream_reader, upstream_writer) = upstream.into_split();
    let intercept_commit = Arc::new(AtomicBool::new(false));
    let mut frontend = tokio::spawn(relay_frontend(
        client_reader,
        upstream_writer,
        Arc::clone(&intercept_commit),
        Arc::clone(&shared),
    ));
    let mut backend = tokio::spawn(relay_backend(
        upstream_reader,
        client_writer,
        intercept_commit,
        shared,
    ));

    tokio::select! {
        result = &mut frontend => {
            backend.abort();
            flatten_proxy_task(result)
        }
        result = &mut backend => {
            frontend.abort();
            flatten_proxy_task(result)
        }
    }
}

fn flatten_proxy_task(result: Result<io::Result<()>, tokio::task::JoinError>) -> io::Result<()> {
    match result {
        Ok(result) => result,
        Err(error) if error.is_cancelled() => Ok(()),
        Err(error) => Err(io::Error::other(format!(
            "PostgreSQL proxy relay task failed: {error}"
        ))),
    }
}

async fn relay_startup(client: &mut TcpStream, upstream: &mut TcpStream) -> io::Result<()> {
    let first = read_startup_packet(client).await?;
    let request_code = startup_request_code(&first);
    upstream.write_all(&first).await?;
    if request_code.is_some_and(|code| {
        code == POSTGRES_SSL_REQUEST_CODE || code == POSTGRES_GSSENC_REQUEST_CODE
    }) {
        let mut response = [0u8; 1];
        upstream.read_exact(&mut response).await?;
        client.write_all(&response).await?;
        if response[0] != b'N' {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "fault proxy requires plaintext PostgreSQL",
            ));
        }
        let startup = read_startup_packet(client).await?;
        upstream.write_all(&startup).await?;
    }
    Ok(())
}

async fn read_startup_packet(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if !(8..=MAX_POSTGRES_PACKET_BYTES).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid PostgreSQL startup packet length",
        ));
    }
    let mut packet = vec![0u8; length];
    packet[..4].copy_from_slice(&length_bytes);
    stream.read_exact(&mut packet[4..]).await?;
    Ok(packet)
}

fn startup_request_code(packet: &[u8]) -> Option<u32> {
    (packet.len() == 8).then(|| u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]))
}

async fn relay_frontend(
    mut client: tokio::net::tcp::OwnedReadHalf,
    mut upstream: tokio::net::tcp::OwnedWriteHalf,
    intercept_commit: Arc<AtomicBool>,
    shared: Arc<ProxyShared>,
) -> io::Result<()> {
    loop {
        let Some((message_type, body, header)) = read_protocol_message(&mut client).await? else {
            return Ok(());
        };
        if frontend_query(message_type, &body).is_some_and(is_commit_query)
            && !shared.fault_consumed.load(Ordering::Acquire)
        {
            intercept_commit.store(true, Ordering::Release);
        }
        upstream.write_all(&header).await?;
        upstream.write_all(&body).await?;
    }
}

async fn relay_backend(
    mut upstream: tokio::net::tcp::OwnedReadHalf,
    mut client: tokio::net::tcp::OwnedWriteHalf,
    intercept_commit: Arc<AtomicBool>,
    shared: Arc<ProxyShared>,
) -> io::Result<()> {
    loop {
        let Some((message_type, body, header)) = read_protocol_message(&mut upstream).await? else {
            return Ok(());
        };

        if intercept_commit.load(Ordering::Acquire) {
            if message_type == b'Z' {
                let claimed = shared
                    .fault_consumed
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok();
                if claimed && shared.mode == CommitAckFaultMode::ThirdState {
                    inject_third_state(&shared.mutation_db)
                        .await
                        .map_err(|error| {
                            io::Error::other(format!("third-state injection failed: {error}"))
                        })?;
                }
                let _ = client.shutdown().await;
                return Ok(());
            }
            continue;
        }

        client.write_all(&header).await?;
        client.write_all(&body).await?;
    }
}

async fn read_protocol_message<R>(reader: &mut R) -> io::Result<Option<(u8, Vec<u8>, [u8; 5])>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut header = [0u8; 5];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    }
    let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    if !(4..=MAX_POSTGRES_PACKET_BYTES).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid PostgreSQL protocol message length",
        ));
    }
    let mut body = vec![0u8; length - 4];
    reader.read_exact(&mut body).await?;
    Ok(Some((header[0], body, header)))
}

fn frontend_query(message_type: u8, body: &[u8]) -> Option<&[u8]> {
    match message_type {
        b'Q' => c_string(body).map(|(query, _)| query),
        b'P' => {
            let (_, remaining) = c_string(body)?;
            c_string(remaining).map(|(query, _)| query)
        }
        _ => None,
    }
}

fn c_string(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = bytes.iter().position(|byte| *byte == 0)?;
    Some((&bytes[..end], &bytes[end + 1..]))
}

fn is_commit_query(query: &[u8]) -> bool {
    let Ok(query) = std::str::from_utf8(query) else {
        return false;
    };
    let normalized = query
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_ascii_uppercase();
    matches!(normalized.as_str(), "COMMIT" | "COMMIT TRANSACTION")
}

async fn inject_third_state(database: &DatabaseConnection) -> TestResult<()> {
    let result = database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "UPDATE {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
                 SET generation = 3, schedule_digest_hex = $2, \
                 updated_at = NOW() \
                 WHERE state_key = $1 AND generation = 2"
            ),
            vec![
                COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
                THIRD_STATE_DIGEST_HEX.into(),
            ],
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err("third-state injection affected an unexpected row count".into());
    }
    Ok(())
}

async fn spawn_fault_child(
    database_url: &str,
    request_id: Uuid,
    actor_id: Uuid,
    anchor: ScheduleAnchor,
) -> TestResult<Output> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--ignored")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ROLE_ENV, "replace")
        .env(CHILD_DATABASE_URL_ENV, database_url)
        .env(CHILD_REQUEST_ID_ENV, request_id.to_string())
        .env(CHILD_ACTOR_ID_ENV, actor_id.to_string())
        .env(
            CHILD_PRIMARY_ACTIVATION_ENV,
            anchor.primary_activation_ms.to_string(),
        )
        .env(
            CHILD_SUCCESSOR_ACTIVATION_ENV,
            anchor.successor_activation_ms.to_string(),
        )
        .env(
            CHILD_PRIMARY_RETIREMENT_ENV,
            anchor.primary_retirement_ms.to_string(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn()?;
    let output = tokio::time::timeout(CHILD_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "audited PostgreSQL fault child timed out")??;
    Ok(output)
}

fn require_child_success(output: &Output) -> TestResult<()> {
    if !output.status.success() {
        return Err(format!(
            "exact-pair child failed: status={:?}, stdout={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
        .into());
    }
    Ok(())
}

fn require_child_abort(output: &Output) -> TestResult<()> {
    #[cfg(unix)]
    {
        if output.status.signal() != Some(6) {
            return Err(unexpected_abort_status(output));
        }
    }
    #[cfg(not(unix))]
    {
        if output.status.success() {
            return Err(unexpected_abort_status(output));
        }
    }
    Ok(())
}

fn unexpected_abort_status(output: &Output) -> Box<dyn std::error::Error + Send + Sync> {
    format!(
        "fault child did not terminate through process abort: status={:?}, stdout={}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
    .into()
}

fn proxy_database_url(target_url: &str, address: std::net::SocketAddr) -> TestResult<String> {
    let mut url = Url::parse(target_url)?;
    url.set_host(Some("127.0.0.1"))
        .map_err(|_| "failed to set PostgreSQL proxy host")?;
    url.set_port(Some(address.port()))
        .map_err(|_| "failed to set PostgreSQL proxy port")?;
    let retained = url
        .query_pairs()
        .filter(|(key, _)| key != "sslmode")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in retained {
            query.append_pair(&key, &value);
        }
        query.append_pair("sslmode", "disable");
    }
    Ok(url.to_string())
}

async fn read_state(database: &DatabaseConnection) -> TestResult<StateRow> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT generation, schedule_digest_hex \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
                 WHERE state_key = $1"
            ),
            vec![COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into()],
        ))
        .await?
        .ok_or("schedule state row is missing")?;
    Ok(StateRow {
        generation: row.try_get("", "generation")?,
        digest_hex: row.try_get("", "schedule_digest_hex")?,
    })
}

async fn read_audit(
    database: &DatabaseConnection,
    request_id: Uuid,
) -> TestResult<Option<AuditRow>> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT request_id, previous_generation, \
                 candidate_generation, (published_at IS NULL) AS unpublished \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
                 WHERE request_id = $1"
            ),
            vec![request_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(AuditRow {
            request_id: row.try_get("", "request_id")?,
            previous_generation: row.try_get("", "previous_generation")?,
            candidate_generation: row.try_get("", "candidate_generation")?,
            unpublished: row.try_get("", "unpublished")?,
        })
    })
    .transpose()
}

async fn count_outbox(database: &DatabaseConnection) -> TestResult<i64> {
    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT COUNT(*)::BIGINT AS row_count \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE}"
            ),
        ))
        .await?
        .ok_or("outbox count query returned no row")?;
    Ok(row.try_get("", "row_count")?)
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name).map_err(|_| format!("{name} is required").into())
}

fn unix_ms() -> TestResult<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
