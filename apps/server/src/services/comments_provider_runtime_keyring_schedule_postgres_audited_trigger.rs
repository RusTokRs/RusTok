use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    keyring_schedule,
    keyring_schedule_persisted_trigger as persisted_trigger,
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
    store: postgres_audit::PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
    pending: Mutex<
        Option<postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext>,
    >,
}

struct PendingAuditGuard<'a> {
    bridge: &'a PostgresAuditedPersistenceBridge,
}

struct AuditedExecutionCompletionGuard {
    completed: bool,
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
            store,
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
            store,
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

    pub fn current_selection(
        &self,
    ) -> std::result::Result<
        keyring_schedule::CommentsTcpDelegationScheduleRuntimeSelection,
        String,
    > {
        self.0.inner.current_selection()
    }

    pub fn current_persistence_record(
        &self,
    ) -> std::result::Result<
        persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        String,
    > {
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
    ) -> std::result::Result<
        keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome,
        String,
    > {
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
    ) -> std::result::Result<
        keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome,
        String,
    > {
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
            "Comments TCP delegation schedule audited trigger state is unavailable"
                .to_string()
        })?;
        let audit_context =
            postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext::new(
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

impl PostgresAuditedPersistenceBridge {
    fn install(
        &self,
        context: postgres_audit::CommentsTcpDelegationSchedulePostgresAuditContext,
    ) -> std::result::Result<PendingAuditGuard<'_>, String> {
        let mut pending = self.pending.lock().map_err(|_| {
            "Comments TCP delegation schedule durable audit context is unavailable"
                .to_string()
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
            .ok_or(
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            )
    }
}

impl persistence::CommentsTcpDelegationSchedulePersistenceStore
    for PostgresAuditedPersistenceBridge
{
    fn verify_current(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<
        (),
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
    > {
        self.store.verify_current(expected)
    }

    fn compare_and_store(
        &self,
        expected: Option<&persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<
        (),
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
    > {
        let result = match expected {
            None => self.store.bootstrap_empty(candidate),
            Some(expected) => {
                let audit = self.current_context()?;
                self.store
                    .compare_and_store_with_audit(expected, candidate, &audit)
            }
        };
        match result {
            Err(
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            ) => abort_on_indeterminate_audited_store_response(),
            other => other,
        }
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
            .debug_struct(
                "SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger",
            )
            .field("persisted_trigger", &self.0.inner)
            .field("postgres_audit_outbox", &"[CONFIGURED]")
            .field("pending_context", &"[REDACTED]")
            .finish()
    }
}

fn current_unix_ms() -> std::result::Result<u64, String> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        "Comments TCP delegation schedule durable audit clock is not available"
            .to_string()
    })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| {
        "Comments TCP delegation schedule durable audit clock is not available"
            .to_string()
    })
}

fn abort_on_indeterminate_audited_store_response() -> ! {
    std::process::abort()
}
