use std::{fmt, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::IndexDriftFindingLifecycleActor;

const DIGEST_BYTES: usize = 64;
const MAX_REASON_BYTES: usize = 512;
const MAX_MACHINE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryState {
    Active,
    Paused,
    Abandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryAction {
    Resume,
    Pause,
    Abandon,
}

#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftRepairRecoveryCommand {
    tenant_id: Uuid,
    finding_id: Uuid,
    command_id: Uuid,
    payload_digest: String,
    decision_id: Uuid,
    expected_revision: Option<u64>,
    action: IndexDriftRepairRecoveryAction,
    actor: IndexDriftFindingLifecycleActor,
    reason: String,
}

impl IndexDriftRepairRecoveryCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: Uuid,
        finding_id: Uuid,
        command_id: Uuid,
        payload_digest: impl Into<String>,
        decision_id: Uuid,
        expected_revision: Option<u64>,
        action: IndexDriftRepairRecoveryAction,
        actor: IndexDriftFindingLifecycleActor,
        reason: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairRecoveryValidationError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilTenantId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilFindingId);
        }
        if command_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilCommandId);
        }
        if decision_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilDecisionId);
        }
        let payload_digest = payload_digest.into();
        if !valid_digest(&payload_digest) {
            return Err(IndexDriftRepairRecoveryValidationError::InvalidDigest);
        }
        let reason = reason.into();
        if !valid_bounded_text(&reason, MAX_REASON_BYTES) {
            return Err(IndexDriftRepairRecoveryValidationError::InvalidReason);
        }
        Ok(Self {
            tenant_id,
            finding_id,
            command_id,
            payload_digest,
            decision_id,
            expected_revision,
            action,
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

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn decision_id(&self) -> Uuid {
        self.decision_id
    }

    pub fn expected_revision(&self) -> Option<u64> {
        self.expected_revision
    }

    pub fn action(&self) -> IndexDriftRepairRecoveryAction {
        self.action
    }

    pub fn actor(&self) -> &IndexDriftFindingLifecycleActor {
        &self.actor
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Debug for IndexDriftRepairRecoveryCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairRecoveryCommand")
            .field("tenant_id", &self.tenant_id)
            .field("finding_id", &self.finding_id)
            .field("command_id", &self.command_id)
            .field("decision_id", &self.decision_id)
            .field("expected_revision", &self.expected_revision)
            .field("action", &self.action)
            .field("actor_kind", &self.actor.kind())
            .field("actor_subject_len", &self.actor.subject().len())
            .field("reason_len", &self.reason.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct IndexDriftAuthorizedRepairRecoveryCommand {
    command: IndexDriftRepairRecoveryCommand,
}

impl IndexDriftAuthorizedRepairRecoveryCommand {
    fn new(command: &IndexDriftRepairRecoveryCommand) -> Self {
        Self {
            command: command.clone(),
        }
    }

    pub fn command(&self) -> &IndexDriftRepairRecoveryCommand {
        &self.command
    }
}

impl fmt::Debug for IndexDriftAuthorizedRepairRecoveryCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftAuthorizedRepairRecoveryCommand")
            .field("tenant_id", &self.command.tenant_id())
            .field("finding_id", &self.command.finding_id())
            .field("command_id", &self.command.command_id())
            .field("decision_id", &self.command.decision_id())
            .field("action", &self.command.action())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftRepairRecoveryValidationError {
    #[error("Index drift repair recovery tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift repair recovery finding id must not be nil")]
    NilFindingId,
    #[error("Index drift repair recovery command id must not be nil")]
    NilCommandId,
    #[error("Index drift repair recovery decision id must not be nil")]
    NilDecisionId,
    #[error("Index drift repair recovery payload digest is invalid")]
    InvalidDigest,
    #[error("Index drift repair recovery reason is invalid")]
    InvalidReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryAuthorization {
    Allowed,
    Denied,
}

#[async_trait]
pub trait IndexDriftRepairRecoveryAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        command: &IndexDriftRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryAuthorization, IndexDriftRepairRecoveryFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairRecoveryReceipt {
    decision_id: Uuid,
    command_id: Uuid,
    finding_id: Uuid,
    revision: u64,
    action: IndexDriftRepairRecoveryAction,
    previous_state: Option<IndexDriftRepairRecoveryState>,
    current_state: IndexDriftRepairRecoveryState,
}

impl IndexDriftRepairRecoveryReceipt {
    pub(crate) fn new(
        decision_id: Uuid,
        command_id: Uuid,
        finding_id: Uuid,
        revision: u64,
        action: IndexDriftRepairRecoveryAction,
        previous_state: Option<IndexDriftRepairRecoveryState>,
        current_state: IndexDriftRepairRecoveryState,
    ) -> Result<Self, IndexDriftRepairRecoveryValidationError> {
        if decision_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilDecisionId);
        }
        if command_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilCommandId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftRepairRecoveryValidationError::NilFindingId);
        }
        Ok(Self {
            decision_id,
            command_id,
            finding_id,
            revision,
            action,
            previous_state,
            current_state,
        })
    }

    pub fn decision_id(&self) -> Uuid {
        self.decision_id
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn action(&self) -> IndexDriftRepairRecoveryAction {
        self.action
    }

    pub fn previous_state(&self) -> Option<IndexDriftRepairRecoveryState> {
        self.previous_state
    }

    pub fn current_state(&self) -> IndexDriftRepairRecoveryState {
        self.current_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryStoreOutcome {
    Applied(IndexDriftRepairRecoveryReceipt),
    AlreadyApplied(IndexDriftRepairRecoveryReceipt),
    AlreadyCompleted,
    NotFound,
    FindingNotOpen,
    StaleRevision {
        current_revision: Option<u64>,
    },
    InvalidTransition {
        current_state: Option<IndexDriftRepairRecoveryState>,
    },
}

#[async_trait]
pub trait IndexDriftRepairRecoveryStore: Send + Sync {
    async fn apply(
        &self,
        authorized: &IndexDriftAuthorizedRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryStoreOutcome, IndexDriftRepairRecoveryFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryOutcome {
    Denied,
    Applied(IndexDriftRepairRecoveryReceipt),
    AlreadyApplied(IndexDriftRepairRecoveryReceipt),
    AlreadyCompleted,
    NotFound,
    FindingNotOpen,
    StaleRevision {
        current_revision: Option<u64>,
    },
    InvalidTransition {
        current_state: Option<IndexDriftRepairRecoveryState>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairRecoveryFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift repair recovery reported a {kind:?} failure ({code})")]
pub struct IndexDriftRepairRecoveryFailure {
    kind: IndexDriftRepairRecoveryFailureKind,
    code: String,
}

impl IndexDriftRepairRecoveryFailure {
    pub fn retryable(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairRecoveryFailureError> {
        Self::new(IndexDriftRepairRecoveryFailureKind::Retryable, code)
    }

    pub fn permanent(
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairRecoveryFailureError> {
        Self::new(IndexDriftRepairRecoveryFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftRepairRecoveryFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairRecoveryFailureError> {
        let code = code.into();
        if !valid_machine_name(&code) {
            return Err(IndexDriftRepairRecoveryFailureError::InvalidCode);
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftRepairRecoveryFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftRepairRecoveryFailureError {
    #[error("Index drift repair recovery failure code is invalid")]
    InvalidCode,
}

#[derive(Clone)]
pub struct IndexDriftRepairRecoveryService {
    authorizer: Arc<dyn IndexDriftRepairRecoveryAuthorizer>,
    store: Arc<dyn IndexDriftRepairRecoveryStore>,
}

impl IndexDriftRepairRecoveryService {
    pub fn new_boxed(
        authorizer: Arc<dyn IndexDriftRepairRecoveryAuthorizer>,
        store: Arc<dyn IndexDriftRepairRecoveryStore>,
    ) -> Self {
        Self { authorizer, store }
    }

    pub async fn execute(
        &self,
        command: &IndexDriftRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryOutcome, IndexDriftRepairRecoveryFailure> {
        if self.authorizer.authorize(command).await?
            != IndexDriftRepairRecoveryAuthorization::Allowed
        {
            return Ok(IndexDriftRepairRecoveryOutcome::Denied);
        }
        let authorized = IndexDriftAuthorizedRepairRecoveryCommand::new(command);
        Ok(match self.store.apply(&authorized).await? {
            IndexDriftRepairRecoveryStoreOutcome::Applied(receipt) => {
                IndexDriftRepairRecoveryOutcome::Applied(receipt)
            }
            IndexDriftRepairRecoveryStoreOutcome::AlreadyApplied(receipt) => {
                IndexDriftRepairRecoveryOutcome::AlreadyApplied(receipt)
            }
            IndexDriftRepairRecoveryStoreOutcome::AlreadyCompleted => {
                IndexDriftRepairRecoveryOutcome::AlreadyCompleted
            }
            IndexDriftRepairRecoveryStoreOutcome::NotFound => {
                IndexDriftRepairRecoveryOutcome::NotFound
            }
            IndexDriftRepairRecoveryStoreOutcome::FindingNotOpen => {
                IndexDriftRepairRecoveryOutcome::FindingNotOpen
            }
            IndexDriftRepairRecoveryStoreOutcome::StaleRevision { current_revision } => {
                IndexDriftRepairRecoveryOutcome::StaleRevision { current_revision }
            }
            IndexDriftRepairRecoveryStoreOutcome::InvalidTransition { current_state } => {
                IndexDriftRepairRecoveryOutcome::InvalidTransition { current_state }
            }
        })
    }
}

impl fmt::Debug for IndexDriftRepairRecoveryService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairRecoveryService")
            .finish_non_exhaustive()
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == DIGEST_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_machine_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MACHINE_NAME_BYTES
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
