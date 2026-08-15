use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rustok_api::AuthPrincipalKind;
use rustok_comments::CommentsTcpDelegationSchedule;
use uuid::Uuid;

use super::{keyring, keyring_schedule};

pub const DEFAULT_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY: usize = 256;
pub const MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleTriggerOperation {
    ReloadFile,
    ReplaceHostSchedule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleTriggerAuthorizationError {
    Denied,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleTriggerAuditOutcome {
    PreflightRejected,
    PrincipalIneligible,
    AuthorizationDenied,
    AuthorizationUnavailable,
    ReplacementRejected,
    ReplacementSucceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleTriggerContext {
    request_id: Uuid,
    actor_id: Uuid,
    principal_kind: AuthPrincipalKind,
}

impl CommentsTcpDelegationScheduleTriggerContext {
    pub fn new(
        request_id: Uuid,
        actor_id: Uuid,
        principal_kind: AuthPrincipalKind,
    ) -> std::result::Result<Self, String> {
        if request_id.is_nil() {
            return Err(
                "Comments TCP delegation schedule trigger request ID must be non-nil".to_string(),
            );
        }
        if actor_id.is_nil() {
            return Err(
                "Comments TCP delegation schedule trigger actor ID must be non-nil".to_string(),
            );
        }
        Ok(Self {
            request_id,
            actor_id,
            principal_kind,
        })
    }

    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn principal_kind(&self) -> AuthPrincipalKind {
        self.principal_kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleTriggerAuthorizationRequest {
    pub request_id: Uuid,
    pub actor_id: Uuid,
    pub principal_kind: AuthPrincipalKind,
    pub operation: CommentsTcpDelegationScheduleTriggerOperation,
    pub source: keyring::CommentsTcpDelegationKeyringSource,
    pub current_generation: u64,
    pub requested_generation: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleTriggerAuditRecord {
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub request_id: Uuid,
    pub actor_id: Uuid,
    pub principal_kind: AuthPrincipalKind,
    pub operation: CommentsTcpDelegationScheduleTriggerOperation,
    pub outcome: CommentsTcpDelegationScheduleTriggerAuditOutcome,
    pub source: Option<keyring::CommentsTcpDelegationKeyringSource>,
    pub previous_generation: Option<u64>,
    pub requested_generation: Option<u64>,
    pub current_generation: Option<u64>,
}

pub trait CommentsTcpDelegationScheduleTriggerAuthorizer: Send + Sync {
    fn authorize(
        &self,
        request: &CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    ) -> std::result::Result<(), CommentsTcpDelegationScheduleTriggerAuthorizationError>;
}

pub type SharedCommentsTcpDelegationScheduleTriggerAuthorizer =
    Arc<dyn CommentsTcpDelegationScheduleTriggerAuthorizer>;

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationScheduleTrigger(Arc<DelegationScheduleTriggerState>);

struct DelegationScheduleTriggerState {
    schedule_handle: keyring_schedule::SharedCommentsTcpDelegationScheduleHandle,
    authorizer: SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
    operation: Mutex<()>,
    audit: Mutex<DelegationScheduleTriggerAuditState>,
}

struct DelegationScheduleTriggerAuditState {
    next_sequence: u64,
    capacity: usize,
    records: VecDeque<CommentsTcpDelegationScheduleTriggerAuditRecord>,
}

impl SharedCommentsTcpDelegationScheduleTrigger {
    pub fn new(
        schedule_handle: keyring_schedule::SharedCommentsTcpDelegationScheduleHandle,
        authorizer: SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
        audit_capacity: usize,
    ) -> std::result::Result<Self, String> {
        if audit_capacity == 0
            || audit_capacity > MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY
        {
            return Err(format!(
                "Comments TCP delegation schedule audit capacity must be within 1..={MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY}"
            ));
        }
        Ok(Self(Arc::new(DelegationScheduleTriggerState {
            schedule_handle,
            authorizer,
            operation: Mutex::new(()),
            audit: Mutex::new(DelegationScheduleTriggerAuditState {
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

    pub fn audit_records(
        &self,
    ) -> std::result::Result<Vec<CommentsTcpDelegationScheduleTriggerAuditRecord>, String> {
        self.0
            .audit
            .lock()
            .map(|audit| audit.records.iter().copied().collect())
            .map_err(|_| "Comments TCP delegation schedule audit state is unavailable".to_string())
    }

    pub fn audit_capacity(&self) -> std::result::Result<usize, String> {
        self.0
            .audit
            .lock()
            .map(|audit| audit.capacity)
            .map_err(|_| "Comments TCP delegation schedule audit state is unavailable".to_string())
    }

    pub fn reload_file(
        &self,
        context: CommentsTcpDelegationScheduleTriggerContext,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        self.execute(
            context,
            CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
            None,
            |handle| handle.reload_file(),
        )
    }

    pub fn replace_host_schedule(
        &self,
        context: CommentsTcpDelegationScheduleTriggerContext,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    {
        self.execute(
            context,
            CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule,
            Some(generation),
            move |handle| handle.replace_host_schedule(schedule, generation),
        )
    }

    pub(super) fn schedule_handle(
        &self,
    ) -> keyring_schedule::SharedCommentsTcpDelegationScheduleHandle {
        self.0.schedule_handle.clone()
    }

    fn execute<F>(
        &self,
        context: CommentsTcpDelegationScheduleTriggerContext,
        operation: CommentsTcpDelegationScheduleTriggerOperation,
        requested_generation: Option<u64>,
        mutation: F,
    ) -> std::result::Result<keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome, String>
    where
        F: FnOnce(
            &keyring_schedule::SharedCommentsTcpDelegationScheduleHandle,
        ) -> std::result::Result<
            keyring_schedule::CommentsTcpDelegationScheduleReloadOutcome,
            String,
        >,
    {
        let _operation = self.0.operation.lock().map_err(|_| {
            "Comments TCP delegation schedule trigger state is unavailable".to_string()
        })?;
        let occurred_at_unix_ms = current_unix_ms()?;
        let selection = match self.0.schedule_handle.current_selection() {
            Ok(selection) => selection,
            Err(error) => {
                self.record_without_mutation(
                    occurred_at_unix_ms,
                    context,
                    operation,
                    CommentsTcpDelegationScheduleTriggerAuditOutcome::PreflightRejected,
                    None,
                    requested_generation,
                    None,
                )?;
                return Err(error);
            }
        };

        let authorization = if context.principal_kind == AuthPrincipalKind::DelegatedUser {
            Err(CommentsTcpDelegationScheduleTriggerAuthorizationError::Denied)
        } else {
            self.0
                .authorizer
                .authorize(&CommentsTcpDelegationScheduleTriggerAuthorizationRequest {
                    request_id: context.request_id,
                    actor_id: context.actor_id,
                    principal_kind: context.principal_kind,
                    operation,
                    source: selection.source,
                    current_generation: selection.generation,
                    requested_generation,
                })
        };

        let mut audit = self.0.audit.lock().map_err(|_| {
            "Comments TCP delegation schedule audit state is unavailable".to_string()
        })?;
        let sequence = audit.allocate_sequence()?;

        match authorization {
            Err(CommentsTcpDelegationScheduleTriggerAuthorizationError::Denied) => {
                let outcome = if context.principal_kind == AuthPrincipalKind::DelegatedUser {
                    CommentsTcpDelegationScheduleTriggerAuditOutcome::PrincipalIneligible
                } else {
                    CommentsTcpDelegationScheduleTriggerAuditOutcome::AuthorizationDenied
                };
                audit.append(CommentsTcpDelegationScheduleTriggerAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id,
                    actor_id: context.actor_id,
                    principal_kind: context.principal_kind,
                    operation,
                    outcome,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    requested_generation,
                    current_generation: Some(selection.generation),
                });
                return Err(
                    "Comments TCP delegation schedule trigger authorization was denied".to_string(),
                );
            }
            Err(CommentsTcpDelegationScheduleTriggerAuthorizationError::Unavailable) => {
                audit.append(CommentsTcpDelegationScheduleTriggerAuditRecord {
                    sequence,
                    occurred_at_unix_ms,
                    request_id: context.request_id,
                    actor_id: context.actor_id,
                    principal_kind: context.principal_kind,
                    operation,
                    outcome:
                        CommentsTcpDelegationScheduleTriggerAuditOutcome::AuthorizationUnavailable,
                    source: Some(selection.source),
                    previous_generation: Some(selection.generation),
                    requested_generation,
                    current_generation: Some(selection.generation),
                });
                return Err(
                    "Comments TCP delegation schedule trigger authorization is unavailable"
                        .to_string(),
                );
            }
            Ok(()) => {}
        }

        let result = mutation(&self.0.schedule_handle);
        match &result {
            Ok(reload) => audit.append(CommentsTcpDelegationScheduleTriggerAuditRecord {
                sequence,
                occurred_at_unix_ms,
                request_id: context.request_id,
                actor_id: context.actor_id,
                principal_kind: context.principal_kind,
                operation,
                outcome: CommentsTcpDelegationScheduleTriggerAuditOutcome::ReplacementSucceeded,
                source: Some(reload.current.source),
                previous_generation: Some(reload.previous_generation),
                requested_generation,
                current_generation: Some(reload.current.generation),
            }),
            Err(_) => audit.append(CommentsTcpDelegationScheduleTriggerAuditRecord {
                sequence,
                occurred_at_unix_ms,
                request_id: context.request_id,
                actor_id: context.actor_id,
                principal_kind: context.principal_kind,
                operation,
                outcome: CommentsTcpDelegationScheduleTriggerAuditOutcome::ReplacementRejected,
                source: Some(selection.source),
                previous_generation: Some(selection.generation),
                requested_generation,
                current_generation: Some(selection.generation),
            }),
        }
        result
    }

    fn record_without_mutation(
        &self,
        occurred_at_unix_ms: u64,
        context: CommentsTcpDelegationScheduleTriggerContext,
        operation: CommentsTcpDelegationScheduleTriggerOperation,
        outcome: CommentsTcpDelegationScheduleTriggerAuditOutcome,
        source: Option<keyring::CommentsTcpDelegationKeyringSource>,
        requested_generation: Option<u64>,
        current_generation: Option<u64>,
    ) -> std::result::Result<(), String> {
        let mut audit = self.0.audit.lock().map_err(|_| {
            "Comments TCP delegation schedule audit state is unavailable".to_string()
        })?;
        let sequence = audit.allocate_sequence()?;
        audit.append(CommentsTcpDelegationScheduleTriggerAuditRecord {
            sequence,
            occurred_at_unix_ms,
            request_id: context.request_id,
            actor_id: context.actor_id,
            principal_kind: context.principal_kind,
            operation,
            outcome,
            source,
            previous_generation: current_generation,
            requested_generation,
            current_generation,
        });
        Ok(())
    }
}

impl DelegationScheduleTriggerAuditState {
    fn allocate_sequence(&mut self) -> std::result::Result<u64, String> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.checked_add(1).ok_or_else(|| {
            "Comments TCP delegation schedule audit sequence is exhausted".to_string()
        })?;
        Ok(sequence)
    }

    fn append(&mut self, record: CommentsTcpDelegationScheduleTriggerAuditRecord) {
        if self.records.len() == self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }
}

impl fmt::Debug for SharedCommentsTcpDelegationScheduleTrigger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let audit = self.0.audit.lock().ok();
        formatter
            .debug_struct("SharedCommentsTcpDelegationScheduleTrigger")
            .field("schedule_handle", &self.0.schedule_handle)
            .field("authorizer", &"[CONFIGURED]")
            .field(
                "audit_capacity",
                &audit.as_ref().map(|state| state.capacity),
            )
            .field(
                "audit_record_count",
                &audit.as_ref().map(|state| state.records.len()),
            )
            .field("audit_actor_ids", &"[REDACTED]")
            .field("audit_request_ids", &"[REDACTED]")
            .finish()
    }
}

fn current_unix_ms() -> std::result::Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Comments TCP delegation schedule audit clock is not available".to_string())?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| "Comments TCP delegation schedule audit clock is not available".to_string())
}
