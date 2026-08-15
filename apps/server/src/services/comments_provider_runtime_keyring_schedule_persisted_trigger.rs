use std::{
    cell::Cell,
    collections::VecDeque,
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustok_api::AuthPrincipalKind;

use super::{
    keyring, keyring_schedule, keyring_schedule_persistence as persistence,
    keyring_schedule_trigger as trigger,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationPersistedScheduleAuditOutcome {
    PreflightRejected,
    PrincipalIneligible,
    AuthorizationDenied,
    AuthorizationUnavailable,
    CandidateRejected,
    PersistenceStateMismatch,
    PersistenceConflict,
    PersistenceUnavailable,
    ReplacementRejected,
    ReplacementSucceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationPersistedScheduleAuditRecord {
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub request_id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub principal_kind: AuthPrincipalKind,
    pub operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
    pub outcome: CommentsTcpDelegationPersistedScheduleAuditOutcome,
    pub source: Option<keyring::CommentsTcpDelegationKeyringSource>,
    pub previous_generation: Option<u64>,
    pub candidate_generation: Option<u64>,
    pub current_generation: Option<u64>,
}

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationPersistedScheduleTrigger(Arc<PersistedScheduleTriggerState>);

struct PersistedScheduleTriggerState {
    schedule_handle: keyring_schedule::SharedCommentsTcpDelegationScheduleHandle,
    source: PersistedScheduleSource,
    authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
    store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore,
    operation: Mutex<()>,
    persistence_record: Mutex<persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
    audit: Mutex<PersistedScheduleAuditState>,
}

enum PersistedScheduleSource {
    HostProvided {
        max_ttl: Duration,
    },
    File {
        file_path: PathBuf,
        max_ttl: Duration,
    },
}

impl PersistedScheduleSource {
    fn category(&self) -> keyring::CommentsTcpDelegationKeyringSource {
        match self {
            Self::HostProvided { .. } => keyring::CommentsTcpDelegationKeyringSource::HostProvided,
            Self::File { .. } => keyring::CommentsTcpDelegationKeyringSource::File,
        }
    }
}

struct PersistedScheduleAuditState {
    next_sequence: u64,
    capacity: usize,
    records: VecDeque<CommentsTcpDelegationPersistedScheduleAuditRecord>,
}

impl SharedCommentsTcpDelegationPersistedScheduleTrigger {
    pub fn from_host_document(
        document: persistence::CommentsTcpDelegationSchedulePersistenceDocument,
        max_ttl: Duration,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore,
        startup_mode: persistence::CommentsTcpDelegationSchedulePersistenceStartupMode,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        let prepared = document.prepare(
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
            max_ttl,
        )?;
        let handle =
            keyring_schedule::SharedCommentsTcpDelegationScheduleHandle::from_host_schedule(
                prepared.schedule.clone(),
                prepared.generation,
            )?;
        Self::new(
            handle,
            PersistedScheduleSource::HostProvided { max_ttl },
            prepared,
            authorizer,
            store,
            startup_mode,
            audit_capacity,
        )
    }

    pub fn from_file(
        file_path: impl Into<PathBuf>,
        max_ttl: Duration,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore,
        startup_mode: persistence::CommentsTcpDelegationSchedulePersistenceStartupMode,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        let file_path = file_path.into();
        let prepared = persistence::load_prepared_schedule_from_file(&file_path, max_ttl)?;
        let handle =
            keyring_schedule::SharedCommentsTcpDelegationScheduleHandle::from_prepared_file(
                file_path.clone(),
                prepared.schedule.clone(),
                prepared.generation,
            )?;
        Self::new(
            handle,
            PersistedScheduleSource::File { file_path, max_ttl },
            prepared,
            authorizer,
            store,
            startup_mode,
            audit_capacity,
        )
    }

    fn new(
        schedule_handle: keyring_schedule::SharedCommentsTcpDelegationScheduleHandle,
        source: PersistedScheduleSource,
        prepared: persistence::PreparedScheduleCandidate,
        authorizer: trigger::SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        store: persistence::SharedCommentsTcpDelegationSchedulePersistenceStore,
        startup_mode: persistence::CommentsTcpDelegationSchedulePersistenceStartupMode,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        if audit_capacity == 0
            || audit_capacity > trigger::MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY
        {
            return Err(format!(
                "Comments TCP persisted schedule audit capacity must be within 1..={}",
                trigger::MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY
            ));
        }
        let initial_record =
            persistence::CommentsTcpDelegationSchedulePersistenceRecord::from_prepared(&prepared);
        match startup_mode {
            persistence::CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty => {
                store
                    .compare_and_store(None, &initial_record)
                    .map_err(startup_store_error)?;
            }
            persistence::CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact => {
                store
                    .verify_current(&initial_record)
                    .map_err(startup_store_error)?;
            }
        }

        Ok(Self(Arc::new(PersistedScheduleTriggerState {
            schedule_handle,
            source,
            authorizer,
            store,
            operation: Mutex::new(()),
            persistence_record: Mutex::new(initial_record),
            audit: Mutex::new(PersistedScheduleAuditState {
                next_sequence: 1,
                capacity: audit_capacity,
                records: VecDeque::with_capacity(audit_capacity),
            }),
        })))
    }

    pub fn current_selection(
        &self,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleRuntimeSelection, String>
    {
        self.0.schedule_handle.current_selection()
    }

    pub fn current_persistence_record(
        &self,
    ) -> std::result::Result<persistence::CommentsTcpDelegationSchedulePersistenceRecord, String>
    {
        self.0
            .persistence_record
            .lock()
            .map(|record| *record)
            .map_err(|_| "Comments TCP persisted schedule state is unavailable".to_string())
    }

    pub fn audit_records(
        &self,
    ) -> std::result::Result<Vec<CommentsTcpDelegationPersistedScheduleAuditRecord>, String> {
        self.0
            .audit
            .lock()
            .map(|audit| audit.records.iter().copied().collect())
            .map_err(|_| "Comments TCP persisted schedule audit state is unavailable".to_string())
    }

    pub fn audit_capacity(&self) -> std::result::Result<usize, String> {
        self.0
            .audit
            .lock()
            .map(|audit| audit.capacity)
            .map_err(|_| "Comments TCP persisted schedule audit state is unavailable".to_string())
    }

    pub fn reload_file(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        self.execute(
            context,
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
            None,
            |source| match source {
                PersistedScheduleSource::File { file_path, max_ttl } => {
                    persistence::load_prepared_schedule_from_file(file_path, *max_ttl)
                }
                PersistedScheduleSource::HostProvided { .. } => Err(
                    "Comments TCP persisted host schedule must be replaced programmatically"
                        .to_string(),
                ),
            },
        )
    }

    pub fn replace_host_schedule(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        document: persistence::CommentsTcpDelegationSchedulePersistenceDocument,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        let requested_generation = Some(document.generation());
        self.execute(
            context,
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule,
            requested_generation,
            move |source| {
                match source {
                PersistedScheduleSource::HostProvided { max_ttl } => document.prepare(
                    keyring::CommentsTcpDelegationKeyringSource::HostProvided,
                    *max_ttl,
                ),
                PersistedScheduleSource::File { .. } => Err(
                    "Comments TCP persisted file schedule must be reloaded from its configured file"
                        .to_string(),
                ),
            }
            },
        )
    }

    pub(super) fn schedule_handle(
        &self,
    ) -> keyring_schedule::SharedCommentsTcpDelegationScheduleHandle {
        self.0.schedule_handle.clone()
    }

    fn execute<P>(
        &self,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
        requested_generation: Option<u64>,
        prepare: P,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    where
        P: FnOnce(
            &PersistedScheduleSource,
        ) -> std::result::Result<persistence::PreparedScheduleCandidate, String>,
    {
        let _operation = self.0.operation.lock().map_err(|_| {
            "Comments TCP persisted schedule trigger state is unavailable".to_string()
        })?;
        let occurred_at_unix_ms = current_unix_ms()?;
        let selection = match self.0.schedule_handle.current_selection() {
            Ok(selection) => selection,
            Err(error) => {
                self.record_without_mutation(
                    occurred_at_unix_ms,
                    context,
                    operation,
                    CommentsTcpDelegationPersistedScheduleAuditOutcome::PreflightRejected,
                    None,
                    None,
                    None,
                    None,
                )?;
                return Err(error);
            }
        };

        let authorization = if context.principal_kind() == AuthPrincipalKind::DelegatedUser {
            Err(trigger::CommentsTcpDelegationScheduleTriggerAuthorizationError::Denied)
        } else {
            self.0.authorizer.authorize(
                &trigger::CommentsTcpDelegationScheduleTriggerAuthorizationRequest {
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    source: selection.source,
                    current_generation: selection.generation,
                    requested_generation,
                },
            )
        };

        let mut audit = self.0.audit.lock().map_err(|_| {
            "Comments TCP persisted schedule audit state is unavailable".to_string()
        })?;
        let sequence = audit.allocate_sequence()?;

        match authorization {
            Err(trigger::CommentsTcpDelegationScheduleTriggerAuthorizationError::Denied) => {
                let outcome = if context.principal_kind() == AuthPrincipalKind::DelegatedUser {
                    CommentsTcpDelegationPersistedScheduleAuditOutcome::PrincipalIneligible
                } else {
                    CommentsTcpDelegationPersistedScheduleAuditOutcome::AuthorizationDenied
                };
                audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    outcome,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    candidate_generation: requested_generation,
                    current_generation: Some(selection.generation),
                });
                return Err(
                    "Comments TCP persisted schedule trigger authorization was denied".to_string(),
                );
            }
            Err(trigger::CommentsTcpDelegationScheduleTriggerAuthorizationError::Unavailable) => {
                audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    outcome: CommentsTcpDelegationPersistedScheduleAuditOutcome::AuthorizationUnavailable,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    candidate_generation: requested_generation,
                    current_generation: Some(selection.generation),
                });
                return Err(
                    "Comments TCP persisted schedule trigger authorization is unavailable"
                        .to_string(),
                );
            }
            Ok(()) => {}
        }

        let candidate = match prepare(&self.0.source) {
            Ok(candidate) => candidate,
            Err(error) => {
                audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    outcome: CommentsTcpDelegationPersistedScheduleAuditOutcome::CandidateRejected,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    candidate_generation: requested_generation,
                    current_generation: Some(selection.generation),
                });
                return Err(error);
            }
        };
        let candidate_record =
            persistence::CommentsTcpDelegationSchedulePersistenceRecord::from_prepared(&candidate);
        let mut persisted = self
            .0
            .persistence_record
            .lock()
            .map_err(|_| "Comments TCP persisted schedule state is unavailable".to_string())?;
        if persisted.source() != selection.source
            || persisted.generation() != selection.generation
            || candidate.source != self.0.source.category()
        {
            audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                sequence,
                occurred_at_unix_ms,
                request_id: context.request_id(),
                actor_id: context.actor_id(),
                principal_kind: context.principal_kind(),
                operation,
                outcome:
                    CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceStateMismatch,
                source: Some(selection.source),
                previous_generation: Some(selection.generation),
                candidate_generation: Some(candidate.generation),
                current_generation: Some(selection.generation),
            });
            return Err(
                "Comments TCP persisted schedule state does not match the active schedule"
                    .to_string(),
            );
        }

        let expected_record = *persisted;
        let store_error = Cell::new(None);
        let result = self.0.schedule_handle.replace_prepared_with_commit(
            candidate.schedule,
            candidate.generation,
            candidate.source,
            || {
                self.0
                    .store
                    .compare_and_store(Some(&expected_record), &candidate_record)
                    .map_err(|error| {
                        store_error.set(Some(error));
                        "Comments TCP persisted schedule durable commit was rejected".to_string()
                    })
            },
        );

        match result {
            Ok(reload) => {
                *persisted = candidate_record;
                audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    outcome:
                        CommentsTcpDelegationPersistedScheduleAuditOutcome::ReplacementSucceeded,
                    source: Some(reload.current.source),
                    previous_generation: Some(reload.previous_generation),
                    candidate_generation: Some(candidate_record.generation()),
                    current_generation: Some(reload.current.generation),
                });
                Ok(reload)
            }
            Err(error) => {
                let outcome = match store_error.get() {
                    Some(
                        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict,
                    ) => CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceConflict,
                    Some(
                        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
                    ) => CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceUnavailable,
                    None => {
                        CommentsTcpDelegationPersistedScheduleAuditOutcome::ReplacementRejected
                    }
                };
                audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id(),
                    actor_id: context.actor_id(),
                    principal_kind: context.principal_kind(),
                    operation,
                    outcome,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    candidate_generation: Some(candidate_record.generation()),
                    current_generation: Some(selection.generation),
                });
                Err(error)
            }
        }
    }

    fn record_without_mutation(
        &self,
        occurred_at_unix_ms: u64,
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
        outcome: CommentsTcpDelegationPersistedScheduleAuditOutcome,
        source: Option<keyring::CommentsTcpDelegationKeyringSource>,
        previous_generation: Option<u64>,
        candidate_generation: Option<u64>,
        current_generation: Option<u64>,
    ) -> std::result::Result<(), String> {
        let mut audit = self.0.audit.lock().map_err(|_| {
            "Comments TCP persisted schedule audit state is unavailable".to_string()
        })?;
        let sequence = audit.allocate_sequence()?;
        audit.append(CommentsTcpDelegationPersistedScheduleAuditRecord {
            sequence,
            occurred_at_unix_ms,
            request_id: context.request_id(),
            actor_id: context.actor_id(),
            principal_kind: context.principal_kind(),
            operation,
            outcome,
            source,
            previous_generation,
            candidate_generation,
            current_generation,
        });
        Ok(())
    }
}

impl PersistedScheduleAuditState {
    fn allocate_sequence(&mut self) -> std::result::Result<u64, String> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.checked_add(1).ok_or_else(|| {
            "Comments TCP persisted schedule audit sequence is exhausted".to_string()
        })?;
        Ok(sequence)
    }

    fn append(&mut self, record: CommentsTcpDelegationPersistedScheduleAuditRecord) {
        if self.records.len() == self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }
}

impl fmt::Debug for SharedCommentsTcpDelegationPersistedScheduleTrigger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let audit = self.0.audit.lock().ok();
        let persistence = self.0.persistence_record.lock().ok();
        formatter
            .debug_struct("SharedCommentsTcpDelegationPersistedScheduleTrigger")
            .field("schedule_handle", &self.0.schedule_handle)
            .field("source", &self.0.source.category())
            .field("authorizer", &"[CONFIGURED]")
            .field("persistence_store", &"[CONFIGURED]")
            .field(
                "persisted_generation",
                &persistence.as_ref().map(|record| record.generation()),
            )
            .field("persisted_digest", &"[CONFIGURED]")
            .field(
                "audit_capacity",
                &audit.as_ref().map(|state| state.capacity),
            )
            .field(
                "audit_record_count",
                &audit.as_ref().map(|state| state.records.len()),
            )
            .field("file_path", &"[REDACTED]")
            .field("audit_actor_ids", &"[REDACTED]")
            .field("audit_request_ids", &"[REDACTED]")
            .finish()
    }
}

fn startup_store_error(
    error: persistence::CommentsTcpDelegationSchedulePersistenceStoreError,
) -> String {
    match error {
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict => {
            "Comments TCP persisted schedule does not match the durable generation and digest"
                .to_string()
        }
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable => {
            "Comments TCP persisted schedule durable state is unavailable".to_string()
        }
    }
}

fn current_unix_ms() -> std::result::Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Comments TCP persisted schedule audit clock is not available".to_string())?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| "Comments TCP persisted schedule audit clock is not available".to_string())
}
