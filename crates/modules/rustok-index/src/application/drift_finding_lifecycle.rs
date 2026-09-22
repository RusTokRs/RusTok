use std::{fmt, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

const MAX_ACTOR_KIND_BYTES: usize = 32;
const MAX_ACTOR_SUBJECT_BYTES: usize = 191;
const MAX_REASON_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingState {
    Open,
    Resolved,
    Ignored,
}

impl IndexDriftFindingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Ignored => "ignored",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleAction {
    Resolve,
    Ignore,
}

impl IndexDriftFindingLifecycleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Ignore => "ignore",
        }
    }

    pub fn target_state(self) -> IndexDriftFindingState {
        match self {
            Self::Resolve => IndexDriftFindingState::Resolved,
            Self::Ignore => IndexDriftFindingState::Ignored,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftFindingLifecycleActor {
    kind: String,
    subject: String,
}

impl IndexDriftFindingLifecycleActor {
    pub fn new(
        kind: impl Into<String>,
        subject: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingLifecycleValidationError> {
        let kind = kind.into();
        if !valid_machine_name(&kind, MAX_ACTOR_KIND_BYTES) {
            return Err(IndexDriftFindingLifecycleValidationError::InvalidActorKind);
        }
        let subject = subject.into();
        if !valid_bounded_text(&subject, MAX_ACTOR_SUBJECT_BYTES) {
            return Err(IndexDriftFindingLifecycleValidationError::InvalidActorSubject);
        }
        Ok(Self { kind, subject })
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl fmt::Debug for IndexDriftFindingLifecycleActor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftFindingLifecycleActor")
            .field("kind", &self.kind)
            .field("subject_len", &self.subject.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftFindingLifecycleCommand {
    tenant_id: Uuid,
    finding_id: Uuid,
    command_id: Uuid,
    action: IndexDriftFindingLifecycleAction,
    expected_state: IndexDriftFindingState,
    actor: IndexDriftFindingLifecycleActor,
    reason: String,
}

impl IndexDriftFindingLifecycleCommand {
    pub fn new(
        tenant_id: Uuid,
        finding_id: Uuid,
        command_id: Uuid,
        action: IndexDriftFindingLifecycleAction,
        expected_state: IndexDriftFindingState,
        actor: IndexDriftFindingLifecycleActor,
        reason: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingLifecycleValidationError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftFindingLifecycleValidationError::NilTenantId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftFindingLifecycleValidationError::NilFindingId);
        }
        if command_id.is_nil() {
            return Err(IndexDriftFindingLifecycleValidationError::NilCommandId);
        }
        if expected_state != IndexDriftFindingState::Open {
            return Err(IndexDriftFindingLifecycleValidationError::UnsupportedExpectedState);
        }
        let reason = reason.into();
        if !valid_bounded_text(&reason, MAX_REASON_BYTES) {
            return Err(IndexDriftFindingLifecycleValidationError::InvalidReason);
        }
        Ok(Self {
            tenant_id,
            finding_id,
            command_id,
            action,
            expected_state,
            actor,
            reason,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn action(&self) -> IndexDriftFindingLifecycleAction {
        self.action
    }

    pub fn expected_state(&self) -> IndexDriftFindingState {
        self.expected_state
    }

    pub fn target_state(&self) -> IndexDriftFindingState {
        self.action.target_state()
    }

    pub fn actor(&self) -> &IndexDriftFindingLifecycleActor {
        &self.actor
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Debug for IndexDriftFindingLifecycleCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftFindingLifecycleCommand")
            .field("tenant_id", &self.tenant_id)
            .field("finding_id", &self.finding_id)
            .field("command_id", &self.command_id)
            .field("action", &self.action)
            .field("expected_state", &self.expected_state)
            .field("actor", &self.actor)
            .field("reason_len", &self.reason.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct IndexDriftFindingAuthorizedLifecycleCommand {
    command: IndexDriftFindingLifecycleCommand,
}

impl IndexDriftFindingAuthorizedLifecycleCommand {
    fn new(command: &IndexDriftFindingLifecycleCommand) -> Self {
        Self {
            command: command.clone(),
        }
    }

    pub fn command(&self) -> &IndexDriftFindingLifecycleCommand {
        &self.command
    }
}

impl fmt::Debug for IndexDriftFindingAuthorizedLifecycleCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftFindingAuthorizedLifecycleCommand")
            .field("tenant_id", &self.command.tenant_id())
            .field("finding_id", &self.command.finding_id())
            .field("command_id", &self.command.command_id())
            .field("action", &self.command.action())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftFindingLifecycleValidationError {
    #[error("Index drift finding lifecycle tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift finding lifecycle finding id must not be nil")]
    NilFindingId,
    #[error("Index drift finding lifecycle command id must not be nil")]
    NilCommandId,
    #[error("Index drift finding lifecycle actor kind is invalid")]
    InvalidActorKind,
    #[error("Index drift finding lifecycle actor subject is invalid")]
    InvalidActorSubject,
    #[error("Index drift finding lifecycle reason is invalid")]
    InvalidReason,
    #[error("Index drift finding lifecycle currently requires expected state open")]
    UnsupportedExpectedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleAuthorization {
    Allowed,
    Denied,
}

#[async_trait]
pub trait IndexDriftFindingLifecycleAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        command: &IndexDriftFindingLifecycleCommand,
    ) -> Result<IndexDriftFindingLifecycleAuthorization, IndexDriftFindingLifecycleFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleNotAppliedReason {
    FindingNotFound,
    StateChanged { current: IndexDriftFindingState },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftFindingLifecycleReceipt {
    command_id: Uuid,
    finding_id: Uuid,
    state: IndexDriftFindingState,
}

impl IndexDriftFindingLifecycleReceipt {
    pub fn new(command_id: Uuid, finding_id: Uuid, state: IndexDriftFindingState) -> Self {
        Self {
            command_id,
            finding_id,
            state,
        }
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn state(&self) -> IndexDriftFindingState {
        self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleStoreOutcome {
    Applied(IndexDriftFindingLifecycleReceipt),
    AlreadyApplied(IndexDriftFindingLifecycleReceipt),
    NotApplied(IndexDriftFindingLifecycleNotAppliedReason),
}

#[async_trait]
pub trait IndexDriftFindingLifecycleStore: Send + Sync {
    async fn apply_authorized_lifecycle_command(
        &self,
        authorized: &IndexDriftFindingAuthorizedLifecycleCommand,
    ) -> Result<IndexDriftFindingLifecycleStoreOutcome, IndexDriftFindingLifecycleFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleOutcome {
    Denied,
    Applied(IndexDriftFindingLifecycleReceipt),
    AlreadyApplied(IndexDriftFindingLifecycleReceipt),
    NotApplied(IndexDriftFindingLifecycleNotAppliedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingLifecycleFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift finding lifecycle reported a {kind:?} failure ({code})")]
pub struct IndexDriftFindingLifecycleFailure {
    kind: IndexDriftFindingLifecycleFailureKind,
    code: String,
}

impl IndexDriftFindingLifecycleFailure {
    pub fn retryable(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingLifecycleFailureError> {
        Self::new(IndexDriftFindingLifecycleFailureKind::Retryable, code)
    }

    pub fn permanent(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingLifecycleFailureError> {
        Self::new(IndexDriftFindingLifecycleFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftFindingLifecycleFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingLifecycleFailureError> {
        let code = code.into();
        if !valid_machine_name(&code, MAX_FAILURE_CODE_BYTES) {
            return Err(IndexDriftFindingLifecycleFailureError::InvalidCode);
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftFindingLifecycleFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftFindingLifecycleFailureError {
    #[error("Index drift finding lifecycle failure code is invalid")]
    InvalidCode,
}

#[derive(Clone)]
pub struct IndexDriftFindingLifecycleService {
    authorizer: Arc<dyn IndexDriftFindingLifecycleAuthorizer>,
    store: Arc<dyn IndexDriftFindingLifecycleStore>,
}

impl IndexDriftFindingLifecycleService {
    pub fn new<A, S>(authorizer: A, store: S) -> Self
    where
        A: IndexDriftFindingLifecycleAuthorizer + 'static,
        S: IndexDriftFindingLifecycleStore + 'static,
    {
        Self {
            authorizer: Arc::new(authorizer),
            store: Arc::new(store),
        }
    }

    pub fn new_boxed(
        authorizer: Arc<dyn IndexDriftFindingLifecycleAuthorizer>,
        store: Arc<dyn IndexDriftFindingLifecycleStore>,
    ) -> Self {
        Self { authorizer, store }
    }

    pub async fn execute(
        &self,
        command: &IndexDriftFindingLifecycleCommand,
    ) -> Result<IndexDriftFindingLifecycleOutcome, IndexDriftFindingLifecycleFailure> {
        if self.authorizer.authorize(command).await?
            != IndexDriftFindingLifecycleAuthorization::Allowed
        {
            return Ok(IndexDriftFindingLifecycleOutcome::Denied);
        }
        let authorized = IndexDriftFindingAuthorizedLifecycleCommand::new(command);
        Ok(
            match self
                .store
                .apply_authorized_lifecycle_command(&authorized)
                .await?
            {
                IndexDriftFindingLifecycleStoreOutcome::Applied(receipt) => {
                    IndexDriftFindingLifecycleOutcome::Applied(receipt)
                }
                IndexDriftFindingLifecycleStoreOutcome::AlreadyApplied(receipt) => {
                    IndexDriftFindingLifecycleOutcome::AlreadyApplied(receipt)
                }
                IndexDriftFindingLifecycleStoreOutcome::NotApplied(reason) => {
                    IndexDriftFindingLifecycleOutcome::NotApplied(reason)
                }
            },
        )
    }
}

impl fmt::Debug for IndexDriftFindingLifecycleService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftFindingLifecycleService")
            .finish_non_exhaustive()
    }
}

fn valid_machine_name(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
