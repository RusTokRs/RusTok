use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use std::{
    sync::mpsc::{self, SyncSender},
    thread,
};

use super::{
    keyring_schedule, keyring_schedule_persisted_trigger as persisted_trigger,
    keyring_schedule_persistence as persistence,
    keyring_schedule_persistence_postgres_audit as postgres_audit,
    keyring_schedule_trigger as trigger,
};

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger(
    Arc<PostgresAuditedScheduleTriggerState>,
);

struct PostgresAuditedScheduleTriggerState {
    inner: persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger,
    bridge: Arc<PostgresAuditedPersistenceBridge>,
    operation: Mutex<()>,
}

struct PostgresAuditedPersistenceBridge {
    store: PostgresAuditedPersistenceStore,
    pending: Mutex<Option<postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext>>,
}

enum PostgresAuditedPersistenceStore {
    Production(postgres_audit::PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore),
    #[cfg(test)]
    ResponseDisconnect(AuditedStoreResponseDisconnectHarness),
}

struct PendingAuditGuard<'a> {
    bridge: &'a PostgresAuditedPersistenceBridge,
}

struct AuditedExecutionCompletionGuard {
    completed: bool,
}

#[cfg(test)]
#[derive(Clone)]
struct AuditedStoreResponseDisconnectHarness {
    commands: SyncSender<AuditedStoreResponseDisconnectCommand>,
}

#[cfg(test)]
enum AuditedStoreResponseDisconnectCommand {
    VerifyCurrent {
        response: SyncSender<
            std::result::Result<
                (),
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
            >,
        >,
    },
    BootstrapEmpty {
        response: SyncSender<
            std::result::Result<
                (),
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
            >,
        >,
    },
    CompareAndStoreWithAudit {
        response: SyncSender<
            std::result::Result<
                (),
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
            >,
        >,
    },
}

impl SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger {
    pub fn from_host_document(
        document: persistence::CommentsTcpDelegationSchedulePersistenceDocument,
        max_ttl: Duration,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        store: postgres_audit::PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
        startup_mode: persistence::CommentsTcpDelegationSchedulePersistenceStartupMode,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        let bridge = Arc::new(PostgresAuditedPersistenceBridge {
            store: PostgresAuditedPersistenceStore::Production(store),
            pending: Mutex::new(None),
        });
        let shared_store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore =
            bridge.clone();
        let inner =
            persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger::from_host_document(
                document,
                max_ttl,
                authorizer,
                shared_store,
                startup_mode,
                audit_capacity,
            )?;
        Ok(Self(Arc::new(PostgresAuditedScheduleTriggerState {
            inner,
            bridge,
            operation: Mutex::new(()),
        })))
    }

    pub fn from_file(
        file_path: impl Into<std::path::PathBuf>,
        max_ttl: Duration,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        store: postgres_audit::PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
        startup_mode: persistence::CommentsTcpDelegationSchedulePersistenceStartupMode,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        let bridge = Arc::new(PostgresAuditedPersistenceBridge {
            store: PostgresAuditedPersistenceStore::Production(store),
            pending: Mutex::new(None),
        });
        let shared_store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore =
            bridge.clone();
        let inner =
            persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger::from_file(
                file_path,
                max_ttl,
                authorizer,
                shared_store,
                startup_mode,
                audit_capacity,
            )?;
        Ok(Self(Arc::new(PostgresAuditedScheduleTriggerState {
            inner,
            bridge,
            operation: Mutex::new(()),
        })))
    }

    #[cfg(test)]
    fn from_response_disconnect_harness(
        document: persistence::CommentsTcpDelegationSchedulePersistenceDocument,
        max_ttl: Duration,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        let bridge = Arc::new(PostgresAuditedPersistenceBridge {
            store: PostgresAuditedPersistenceStore::ResponseDisconnect(
                AuditedStoreResponseDisconnectHarness::new()?,
            ),
            pending: Mutex::new(None),
        });
        let shared_store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore =
            bridge.clone();
        let inner =
            persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger::from_host_document(
                document,
                max_ttl,
                authorizer,
                shared_store,
                persistence::CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty,
                audit_capacity,
            )?;
        Ok(Self(Arc::new(PostgresAuditedScheduleTriggerState {
            inner,
            bridge,
            operation: Mutex::new(()),
        })))
    }

    pub fn current_selection(
        &self,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleRuntimeSelection, String>
    {
        self.0.inner.current_selection()
    }

    pub fn current_persistence_record(
        &self,
    ) -> std::result::Result<persistence::CommentsTcpDelegationSchedulePersistenceRecord, String>
    {
        self.0.inner.current_persistence_record()
    }

    pub fn audit_records(
        &self,
    ) -> std::result::Result<
        Vec<persisted_trigger::CommentsTcpDelegationPersistedScheduleAuditRecord>,
        String,
    > {
        self.0.inner.audit_records()
    }

    pub fn audit_capacity(&self) -> std::result::Result<usize, String> {
        self.0.inner.audit_capacity()
    }

    pub fn reload_file(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        self.execute_with_audit_context(
            context,
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
            |inner| inner.reload_file(context),
        )
    }

    pub fn replace_host_schedule(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        document: persistence::CommentsTcpDelegationSchedulePersistenceDocument,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        self.execute_with_audit_context(
            context,
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule,
            move |inner| inner.replace_host_schedule(context, document),
        )
    }

    pub(super) fn schedule_handle(
        &self,
    ) -> keyring_schedule::SharedCommentsTcpDelegationScheduleHandle {
        self.0.inner.schedule_handle()
    }

    fn execute_with_audit_context<T>(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
        execute: impl FnOnce(
            &persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger,
        ) -> std::result::Result<T, String>,
    ) -> std::result::Result<T, String> {
        let _operation = self.0.operation.lock().map_err(|_| {
            "Comments TCP delegation schedule audited trigger state is unavailable".to_string()
        })?;
        let audit_context = postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext::new(
            context,
            operation,
            current_unix_ms()?,
        )?;
        let _pending = self.0.bridge.install(audit_context)?;
        let mut completion = AuditedExecutionCompletionGuard { completed: false };
        let result = execute(&self.0.inner);
        completion.completed = true;
        result
    }
}

impl PostgresAuditedPersistenceStore {
    fn verify_current(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        match self {
            Self::Production(store) => store.verify_current(expected),
            #[cfg(test)]
            Self::ResponseDisconnect(store) => store.verify_current(),
        }
    }

    fn bootstrap_empty(
        &self,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        match self {
            Self::Production(store) => store.bootstrap_empty(candidate),
            #[cfg(test)]
            Self::ResponseDisconnect(store) => store.bootstrap_empty(),
        }
    }

    fn compare_and_store_with_audit(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        audit: &postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        match self {
            Self::Production(store) => {
                store.compare_and_store_with_audit(expected, candidate, audit)
            }
            #[cfg(test)]
            Self::ResponseDisconnect(store) => store.compare_and_store_with_audit(),
        }
    }
}

impl PostgresAuditedPersistenceBridge {
    fn install(
        &self,
        context: postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext,
    ) -> std::result::Result<PendingAuditGuard<'_>, String> {
        let mut pending = self.pending.lock().map_err(|_| {
            "Comments TCP delegation schedule durable audit context is unavailable".to_string()
        })?;
        if pending.is_some() {
            return Err(
                "Comments TCP delegation schedule durable audit context is already active"
                    .to_string(),
            );
        }
        *pending = Some(context);
        Ok(PendingAuditGuard { bridge: self })
    }

    fn current_context(
        &self,
    ) -> std::result::Result<
        postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext,
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
    > {
        self.pending
            .lock()
            .map_err(|_| {
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
            })?
            .as_ref()
            .copied()
            .ok_or(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable)
    }
}

impl persistence::CommentsTcpDelegationSchedulePersistenceStore
    for PostgresAuditedPersistenceBridge
{
    fn verify_current(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        self.store.verify_current(expected)
    }

    fn compare_and_store(
        &self,
        expected: Option<&persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        let result = match expected {
            None => self.store.bootstrap_empty(candidate),
            Some(expected) => {
                let audit = self.current_context()?;
                self.store
                    .compare_and_store_with_audit(expected, candidate, &audit)
            }
        };
        match result {
            Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable) => {
                abort_on_indeterminate_audited_store_response()
            }
            other => other,
        }
    }
}

#[cfg(test)]
impl AuditedStoreResponseDisconnectHarness {
    fn new() -> std::result::Result<Self, String> {
        let (commands, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("comments-delegation-schedule-audit-response-disconnect".to_string())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    match command {
                        AuditedStoreResponseDisconnectCommand::VerifyCurrent { response }
                        | AuditedStoreResponseDisconnectCommand::BootstrapEmpty { response } => {
                            let _ = response.send(Ok(()));
                        }
                        AuditedStoreResponseDisconnectCommand::CompareAndStoreWithAudit {
                            response,
                        } => {
                            drop(response);
                            return;
                        }
                    }
                }
            })
            .map_err(|_| {
                "Comments TCP delegation schedule response-disconnect harness could not start"
                    .to_string()
            })?;
        Ok(Self { commands })
    }

    fn verify_current(
        &self,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        self.request(|response| AuditedStoreResponseDisconnectCommand::VerifyCurrent { response })
    }

    fn bootstrap_empty(
        &self,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        self.request(|response| AuditedStoreResponseDisconnectCommand::BootstrapEmpty { response })
    }

    fn compare_and_store_with_audit(
        &self,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        self.request(
            |response| AuditedStoreResponseDisconnectCommand::CompareAndStoreWithAudit { response },
        )
    }

    fn request(
        &self,
        build: impl FnOnce(
            SyncSender<
                std::result::Result<
                    (),
                    persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
                >,
            >,
        ) -> AuditedStoreResponseDisconnectCommand,
    ) -> std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>
    {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.commands.send(build(response_sender)).map_err(|_| {
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
        })?;
        response_receiver.recv().unwrap_or(Err(
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
        ))
    }
}

impl Drop for PendingAuditGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.bridge.pending.lock() {
            *pending = None;
        }
    }
}

impl Drop for AuditedExecutionCompletionGuard {
    fn drop(&mut self) {
        if !self.completed {
            abort_on_indeterminate_audited_store_response();
        }
    }
}

impl fmt::Debug for SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger")
            .field("persisted_trigger", &self.0.inner)
            .field("postgres_audit_outbox", &"[CONFIGURED]")
            .field("pending_context", &"[REDACTED]")
            .finish()
    }
}

fn current_unix_ms() -> std::result::Result<u64, String> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        "Comments TCP delegation schedule durable audit clock is not available".to_string()
    })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| {
        "Comments TCP delegation schedule durable audit clock is not available".to_string()
    })
}

fn abort_on_indeterminate_audited_store_response() -> ! {
    std::process::abort()
}

#[cfg(test)]
mod tests {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    use rustok_api::AuthPrincipalKind;
    use uuid::Uuid;

    use super::*;

    const CHILD_ENV: &str = "RUSTOK_BLOG_AUDITED_WORKER_RESPONSE_DISCONNECT_CHILD";
    const CHILD_FILTER: &str = "audited_worker_response_disconnect_child";
    const CHILD_READY_MARKER: &str = "rustok-blog-audited-worker-response-disconnect-ready";
    const CHILD_TIMEOUT: Duration = Duration::from_secs(10);
    const PROPAGATION_BUDGET_MS: u64 = 1_000;
    const MAX_TTL_MS: u64 = 5_000;
    const SUCCESSOR_DELAY_MS: u64 = 120_000;
    const SECRET_A: &str = "comments-worker-disconnect-secret-a-000000000001";
    const SECRET_B: &str = "comments-worker-disconnect-secret-b-000000000002";

    struct AllowAuthorizer;

    impl trigger::CommentsTcpDelegationScheduleTriggerAuthorizer for AllowAuthorizer {
        fn authorize(
            &self,
            _request: &trigger::CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
        ) -> std::result::Result<(), trigger::CommentsTcpDelegationScheduleTriggerAuthorizationError>
        {
            Ok(())
        }
    }

    #[test]
    #[ignore = "requires subprocess abort and signal observation"]
    fn audited_worker_response_disconnect_aborts() {
        let mut child = Command::new(
            std::env::current_exe().expect("resolve the current rustok-server test binary"),
        )
        .arg(CHILD_FILTER)
        .arg("--ignored")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ENV, "replace")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the audited worker-response disconnect child");

        let deadline = Instant::now() + CHILD_TIMEOUT;
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .expect("poll the audited worker-response disconnect child")
            {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("audited worker-response disconnect child exceeded {CHILD_TIMEOUT:?}");
            }
            thread::sleep(Duration::from_millis(10));
        };

        let mut stdout = String::new();
        let mut stderr = String::new();
        child
            .stdout
            .take()
            .expect("child stdout pipe")
            .read_to_string(&mut stdout)
            .expect("read child stdout");
        child
            .stderr
            .take()
            .expect("child stderr pipe")
            .read_to_string(&mut stderr)
            .expect("read child stderr");

        assert!(
            stderr.contains(CHILD_READY_MARKER),
            "child did not reach the audited replacement boundary; status={status:?}, stdout={stdout:?}, stderr={stderr:?}"
        );

        #[cfg(unix)]
        assert_eq!(
            status.signal(),
            Some(6),
            "audited worker-response disconnect must end in SIGABRT; stdout={stdout:?}, stderr={stderr:?}"
        );

        #[cfg(not(unix))]
        assert!(
            !status.success(),
            "audited worker-response disconnect must terminate abnormally; stdout={stdout:?}, stderr={stderr:?}"
        );
    }

    #[test]
    #[ignore = "subprocess entry point for audited worker-response disconnect"]
    fn audited_worker_response_disconnect_child() {
        if std::env::var(CHILD_ENV).as_deref() != Ok("replace") {
            return;
        }

        let now = current_unix_ms().expect("read the audited schedule clock");
        let successor_activation_ms = now
            .checked_add(SUCCESSOR_DELAY_MS)
            .expect("successor activation overflow");
        let primary_retirement_ms = successor_activation_ms
            .checked_add(PROPAGATION_BUDGET_MS)
            .and_then(|value| value.checked_add(MAX_TTL_MS))
            .and_then(|value| {
                value.checked_add(rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS)
            })
            .and_then(|value| value.checked_add(1_000))
            .expect("primary retirement overflow");
        let primary_activation_ms = now.saturating_sub(60_000).max(1);

        let initial = schedule_document(
            1,
            primary_activation_ms,
            successor_activation_ms,
            primary_retirement_ms,
            false,
        );
        let replacement = schedule_document(
            2,
            primary_activation_ms,
            successor_activation_ms,
            primary_retirement_ms,
            true,
        );
        let authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer =
            Arc::new(AllowAuthorizer);
        let audited =
            SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_response_disconnect_harness(
                initial,
                Duration::from_millis(MAX_TTL_MS),
                authorizer,
                16,
            )
            .expect("construct the audited response-disconnect harness");

        eprintln!("{CHILD_READY_MARKER}");
        let result = audited.replace_host_schedule(
            trigger::CommentsTcpDelegationScheduleTriggerContext::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                AuthPrincipalKind::Service,
            )
            .expect("construct the audited trigger context"),
            replacement,
        );
        panic!("audited worker-response disconnect returned instead of aborting: {result:?}");
    }

    fn schedule_document(
        generation: u64,
        primary_activation_ms: u64,
        successor_activation_ms: u64,
        primary_retirement_ms: u64,
        include_successor: bool,
    ) -> persistence::CommentsTcpDelegationSchedulePersistenceDocument {
        let primary = persistence::CommentsTcpDelegationSchedulePersistenceKey::new(
            "worker-disconnect-key-a",
            SECRET_A,
            primary_activation_ms,
            include_successor.then_some(primary_retirement_ms),
        )
        .expect("construct the primary audited schedule key");
        let mut keys = vec![primary];
        if include_successor {
            keys.push(
                persistence::CommentsTcpDelegationSchedulePersistenceKey::new(
                    "worker-disconnect-key-b",
                    SECRET_B,
                    successor_activation_ms,
                    None,
                )
                .expect("construct the successor audited schedule key"),
            );
        }
        persistence::CommentsTcpDelegationSchedulePersistenceDocument::new(
            generation,
            Duration::from_millis(PROPAGATION_BUDGET_MS),
            keys,
            None,
        )
        .expect("construct the audited schedule document")
    }
}
