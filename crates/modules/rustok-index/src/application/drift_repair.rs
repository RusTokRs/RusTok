use std::{collections::BTreeMap, fmt, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{EntityKey, LinkName, LinkedEntityKey};

use super::IndexDriftFindingLifecycleActor;

const DIGEST_BYTES: usize = 64;
const MAX_REASON_BYTES: usize = 512;
const MAX_MACHINE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexDriftRepairTargetKind {
    MissingEntity,
    OrphanLink,
}

#[derive(Clone, PartialEq, Eq)]
pub enum IndexDriftRepairTarget {
    MissingEntity {
        key: EntityKey,
        indexed_source_version: u64,
        absence_source_version: u64,
    },
    OrphanLink {
        source_key: EntityKey,
        indexed_source_version: u64,
        link_name: LinkName,
        ordinal: u32,
        target: LinkedEntityKey,
        target_absence_source_version: u64,
    },
}

impl IndexDriftRepairTarget {
    pub fn missing_entity(
        key: EntityKey,
        indexed_source_version: u64,
        absence_source_version: u64,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        validate_key_and_versions(&key, indexed_source_version, absence_source_version)?;
        Ok(Self::MissingEntity {
            key,
            indexed_source_version,
            absence_source_version,
        })
    }

    pub fn orphan_link(
        source_key: EntityKey,
        indexed_source_version: u64,
        link_name: LinkName,
        ordinal: u32,
        target: LinkedEntityKey,
        target_absence_source_version: u64,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        validate_key_and_versions(
            &source_key,
            indexed_source_version,
            target_absence_source_version,
        )?;
        if target.entity_id.is_nil() || target.schema.version.get() == 0 {
            return Err(IndexDriftRepairValidationError::InvalidTargetIdentity);
        }
        Ok(Self::OrphanLink {
            source_key,
            indexed_source_version,
            link_name,
            ordinal,
            target,
            target_absence_source_version,
        })
    }

    pub fn kind(&self) -> IndexDriftRepairTargetKind {
        match self {
            Self::MissingEntity { .. } => IndexDriftRepairTargetKind::MissingEntity,
            Self::OrphanLink { .. } => IndexDriftRepairTargetKind::OrphanLink,
        }
    }

    pub fn tenant_id(&self) -> Uuid {
        self.source_key().tenant_id
    }

    pub fn source_key(&self) -> &EntityKey {
        match self {
            Self::MissingEntity { key, .. } => key,
            Self::OrphanLink { source_key, .. } => source_key,
        }
    }

    pub fn indexed_source_version(&self) -> u64 {
        match self {
            Self::MissingEntity {
                indexed_source_version,
                ..
            }
            | Self::OrphanLink {
                indexed_source_version,
                ..
            } => *indexed_source_version,
        }
    }

    pub fn absence_source_version(&self) -> u64 {
        match self {
            Self::MissingEntity {
                absence_source_version,
                ..
            } => *absence_source_version,
            Self::OrphanLink {
                target_absence_source_version,
                ..
            } => *target_absence_source_version,
        }
    }

    pub fn link_name(&self) -> Option<&LinkName> {
        match self {
            Self::MissingEntity { .. } => None,
            Self::OrphanLink { link_name, .. } => Some(link_name),
        }
    }

    pub fn ordinal(&self) -> Option<u32> {
        match self {
            Self::MissingEntity { .. } => None,
            Self::OrphanLink { ordinal, .. } => Some(*ordinal),
        }
    }

    pub fn linked_target(&self) -> Option<&LinkedEntityKey> {
        match self {
            Self::MissingEntity { .. } => None,
            Self::OrphanLink { target, .. } => Some(target),
        }
    }
}

impl fmt::Debug for IndexDriftRepairTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairTarget")
            .field("kind", &self.kind())
            .field("tenant_id", &self.tenant_id())
            .field("indexed_source_version", &self.indexed_source_version())
            .field("absence_source_version", &self.absence_source_version())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IndexDriftRepairCommand {
    tenant_id: Uuid,
    finding_id: Uuid,
    command_id: Uuid,
    target: IndexDriftRepairTarget,
    actor: IndexDriftFindingLifecycleActor,
    reason: String,
}

impl IndexDriftRepairCommand {
    pub fn new(
        tenant_id: Uuid,
        finding_id: Uuid,
        command_id: Uuid,
        target: IndexDriftRepairTarget,
        actor: IndexDriftFindingLifecycleActor,
        reason: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilTenantId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilFindingId);
        }
        if command_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilCommandId);
        }
        if target.tenant_id() != tenant_id {
            return Err(IndexDriftRepairValidationError::TargetTenantMismatch);
        }
        let reason = reason.into();
        if !valid_bounded_text(&reason, MAX_REASON_BYTES) {
            return Err(IndexDriftRepairValidationError::InvalidReason);
        }
        Ok(Self {
            tenant_id,
            finding_id,
            command_id,
            target,
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

    pub fn target(&self) -> &IndexDriftRepairTarget {
        &self.target
    }

    pub fn actor(&self) -> &IndexDriftFindingLifecycleActor {
        &self.actor
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Debug for IndexDriftRepairCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairCommand")
            .field("tenant_id", &self.tenant_id)
            .field("finding_id", &self.finding_id)
            .field("command_id", &self.command_id)
            .field("target_kind", &self.target.kind())
            .field("actor_kind", &self.actor.kind())
            .field("actor_subject_len", &self.actor.subject().len())
            .field("reason_len", &self.reason.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct IndexDriftAuthorizedRepairCommand {
    command: IndexDriftRepairCommand,
}

impl IndexDriftAuthorizedRepairCommand {
    fn new(command: &IndexDriftRepairCommand) -> Self {
        Self {
            command: command.clone(),
        }
    }

    pub fn command(&self) -> &IndexDriftRepairCommand {
        &self.command
    }
}

impl fmt::Debug for IndexDriftAuthorizedRepairCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftAuthorizedRepairCommand")
            .field("tenant_id", &self.command.tenant_id())
            .field("finding_id", &self.command.finding_id())
            .field("command_id", &self.command.command_id())
            .field("target_kind", &self.command.target().kind())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftRepairValidationError {
    #[error("Index drift repair tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift repair finding id must not be nil")]
    NilFindingId,
    #[error("Index drift repair command id must not be nil")]
    NilCommandId,
    #[error("Index drift repair target identity is invalid")]
    InvalidTargetIdentity,
    #[error("Index drift repair target tenant must match the command tenant")]
    TargetTenantMismatch,
    #[error("Index drift repair source versions must be positive")]
    InvalidSourceVersion,
    #[error("Index drift repair reason is invalid")]
    InvalidReason,
    #[error("Index drift repair digest is invalid")]
    InvalidDigest,
    #[error("Index drift repair machine name is invalid")]
    InvalidMachineName,
    #[error("Index drift repair owner kind is registered more than once")]
    DuplicateOwnerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairAuthorization {
    Allowed,
    Denied,
}

#[async_trait]
pub trait IndexDriftRepairAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        command: &IndexDriftRepairCommand,
    ) -> Result<IndexDriftRepairAuthorization, IndexDriftRepairFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairFinding {
    finding_id: Uuid,
    finding_key: String,
    expected_digest: String,
    actual_digest: String,
    target: IndexDriftRepairTarget,
}

impl IndexDriftRepairFinding {
    pub(crate) fn new(
        finding_id: Uuid,
        finding_key: String,
        expected_digest: String,
        actual_digest: String,
        target: IndexDriftRepairTarget,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        if finding_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilFindingId);
        }
        if !valid_digest(&finding_key)
            || !valid_digest(&expected_digest)
            || !valid_digest(&actual_digest)
            || expected_digest == actual_digest
        {
            return Err(IndexDriftRepairValidationError::InvalidDigest);
        }
        Ok(Self {
            finding_id,
            finding_key,
            expected_digest,
            actual_digest,
            target,
        })
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn finding_key(&self) -> &str {
        &self.finding_key
    }

    pub fn expected_digest(&self) -> &str {
        &self.expected_digest
    }

    pub fn actual_digest(&self) -> &str {
        &self.actual_digest
    }

    pub fn target(&self) -> &IndexDriftRepairTarget {
        &self.target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairEvidenceState {
    Repairable,
    Converged,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairEvidence {
    state: IndexDriftRepairEvidenceState,
    digest: String,
}

impl IndexDriftRepairEvidence {
    pub fn new(
        state: IndexDriftRepairEvidenceState,
        digest: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        let digest = digest.into();
        if !valid_digest(&digest) {
            return Err(IndexDriftRepairValidationError::InvalidDigest);
        }
        Ok(Self { state, digest })
    }

    pub fn state(&self) -> IndexDriftRepairEvidenceState {
        self.state
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[async_trait]
pub trait IndexDriftRepairEvidenceReader: Send + Sync {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure>;

    async fn capture_after(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairOwnerOutcome {
    Applied { receipt_digest: String },
    NotApplied { code: String },
}

impl IndexDriftRepairOwnerOutcome {
    pub fn applied(
        receipt_digest: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        let receipt_digest = receipt_digest.into();
        if !valid_digest(&receipt_digest) {
            return Err(IndexDriftRepairValidationError::InvalidDigest);
        }
        Ok(Self::Applied { receipt_digest })
    }

    pub fn not_applied(code: impl Into<String>) -> Result<Self, IndexDriftRepairValidationError> {
        let code = code.into();
        validate_machine_name(&code)?;
        Ok(Self::NotApplied { code })
    }
}

#[async_trait]
pub trait IndexDriftRepairOwner: Send + Sync {
    fn owner_name(&self) -> &str;
    fn target_kind(&self) -> IndexDriftRepairTargetKind;

    async fn repair(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairOwnerOutcome, IndexDriftRepairFailure>;
}

#[derive(Clone, Default)]
pub struct IndexDriftRepairOwnerRegistry {
    owners: Arc<BTreeMap<IndexDriftRepairTargetKind, Arc<dyn IndexDriftRepairOwner>>>,
}

impl IndexDriftRepairOwnerRegistry {
    pub fn new<I>(owners: I) -> Result<Self, IndexDriftRepairValidationError>
    where
        I: IntoIterator<Item = Arc<dyn IndexDriftRepairOwner>>,
    {
        let mut by_kind = BTreeMap::new();
        for owner in owners {
            validate_machine_name(owner.owner_name())?;
            if by_kind.insert(owner.target_kind(), owner).is_some() {
                return Err(IndexDriftRepairValidationError::DuplicateOwnerKind);
            }
        }
        Ok(Self {
            owners: Arc::new(by_kind),
        })
    }

    pub fn owner_for(
        &self,
        kind: IndexDriftRepairTargetKind,
    ) -> Option<Arc<dyn IndexDriftRepairOwner>> {
        self.owners.get(&kind).cloned()
    }
}

impl fmt::Debug for IndexDriftRepairOwnerRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairOwnerRegistry")
            .field("owner_count", &self.owners.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairTicket {
    tenant_id: Uuid,
    command_id: Uuid,
    finding_id: Uuid,
    reservation_digest: String,
}

impl IndexDriftRepairTicket {
    pub(crate) fn new(
        tenant_id: Uuid,
        command_id: Uuid,
        finding_id: Uuid,
        reservation_digest: String,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilTenantId);
        }
        if command_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilCommandId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilFindingId);
        }
        if !valid_digest(&reservation_digest) {
            return Err(IndexDriftRepairValidationError::InvalidDigest);
        }
        Ok(Self {
            tenant_id,
            command_id,
            finding_id,
            reservation_digest,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn reservation_digest(&self) -> &str {
        &self.reservation_digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairNotStartedReason {
    FindingNotFound,
    FindingNotOpen,
    FindingBusy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairReceiptOutcome {
    Repaired,
    NotRepaired { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairReceipt {
    command_id: Uuid,
    finding_id: Uuid,
    outcome: IndexDriftRepairReceiptOutcome,
    before_digest: String,
    after_digest: Option<String>,
    owner_receipt_digest: Option<String>,
}

impl IndexDriftRepairReceipt {
    pub(crate) fn new(
        command_id: Uuid,
        finding_id: Uuid,
        outcome: IndexDriftRepairReceiptOutcome,
        before_digest: String,
        after_digest: Option<String>,
        owner_receipt_digest: Option<String>,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        validate_completion_fields(
            &outcome,
            &before_digest,
            after_digest.as_deref(),
            owner_receipt_digest.as_deref(),
        )?;
        if command_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilCommandId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftRepairValidationError::NilFindingId);
        }
        Ok(Self {
            command_id,
            finding_id,
            outcome,
            before_digest,
            after_digest,
            owner_receipt_digest,
        })
    }

    pub fn command_id(&self) -> Uuid {
        self.command_id
    }

    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn outcome(&self) -> &IndexDriftRepairReceiptOutcome {
        &self.outcome
    }

    pub fn before_digest(&self) -> &str {
        &self.before_digest
    }

    pub fn after_digest(&self) -> Option<&str> {
        self.after_digest.as_deref()
    }

    pub fn owner_receipt_digest(&self) -> Option<&str> {
        self.owner_receipt_digest.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairReservationOutcome {
    Reserved {
        ticket: IndexDriftRepairTicket,
        finding: IndexDriftRepairFinding,
    },
    Resumed {
        ticket: IndexDriftRepairTicket,
        finding: IndexDriftRepairFinding,
    },
    AlreadyCompleted(IndexDriftRepairReceipt),
    NotReserved(IndexDriftRepairNotStartedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftRepairCompletion {
    owner_name: String,
    outcome: IndexDriftRepairReceiptOutcome,
    before_digest: String,
    after_digest: Option<String>,
    owner_receipt_digest: Option<String>,
}

impl IndexDriftRepairCompletion {
    pub(crate) fn new(
        owner_name: String,
        outcome: IndexDriftRepairReceiptOutcome,
        before_digest: String,
        after_digest: Option<String>,
        owner_receipt_digest: Option<String>,
    ) -> Result<Self, IndexDriftRepairValidationError> {
        validate_machine_name(&owner_name)?;
        validate_completion_fields(
            &outcome,
            &before_digest,
            after_digest.as_deref(),
            owner_receipt_digest.as_deref(),
        )?;
        Ok(Self {
            owner_name,
            outcome,
            before_digest,
            after_digest,
            owner_receipt_digest,
        })
    }

    pub fn owner_name(&self) -> &str {
        &self.owner_name
    }

    pub fn outcome(&self) -> &IndexDriftRepairReceiptOutcome {
        &self.outcome
    }

    pub fn before_digest(&self) -> &str {
        &self.before_digest
    }

    pub fn after_digest(&self) -> Option<&str> {
        self.after_digest.as_deref()
    }

    pub fn owner_receipt_digest(&self) -> Option<&str> {
        self.owner_receipt_digest.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairStoreCompletionOutcome {
    Completed(IndexDriftRepairReceipt),
    AlreadyCompleted(IndexDriftRepairReceipt),
}

#[async_trait]
pub trait IndexDriftRepairStore: Send + Sync {
    async fn reserve(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure>;

    async fn complete(
        &self,
        ticket: &IndexDriftRepairTicket,
        completion: &IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftRepairOutcome {
    Denied,
    Repaired(IndexDriftRepairReceipt),
    NotRepaired(IndexDriftRepairReceipt),
    AlreadyCompleted(IndexDriftRepairReceipt),
    NotStarted(IndexDriftRepairNotStartedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftRepairFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index drift repair reported a {kind:?} failure ({code})")]
pub struct IndexDriftRepairFailure {
    kind: IndexDriftRepairFailureKind,
    code: String,
}

impl IndexDriftRepairFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexDriftRepairFailureError> {
        Self::new(IndexDriftRepairFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexDriftRepairFailureError> {
        Self::new(IndexDriftRepairFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexDriftRepairFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexDriftRepairFailureError> {
        let code = code.into();
        if !valid_machine_name(&code) {
            return Err(IndexDriftRepairFailureError::InvalidCode);
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexDriftRepairFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftRepairFailureError {
    #[error("Index drift repair failure code is invalid")]
    InvalidCode,
}

#[derive(Clone)]
pub struct IndexDriftRepairService {
    authorizer: Arc<dyn IndexDriftRepairAuthorizer>,
    evidence: Arc<dyn IndexDriftRepairEvidenceReader>,
    owners: IndexDriftRepairOwnerRegistry,
    store: Arc<dyn IndexDriftRepairStore>,
}

impl IndexDriftRepairService {
    pub fn new_boxed(
        authorizer: Arc<dyn IndexDriftRepairAuthorizer>,
        evidence: Arc<dyn IndexDriftRepairEvidenceReader>,
        owners: IndexDriftRepairOwnerRegistry,
        store: Arc<dyn IndexDriftRepairStore>,
    ) -> Self {
        Self {
            authorizer,
            evidence,
            owners,
            store,
        }
    }

    pub async fn execute(
        &self,
        command: &IndexDriftRepairCommand,
    ) -> Result<IndexDriftRepairOutcome, IndexDriftRepairFailure> {
        if self.authorizer.authorize(command).await? != IndexDriftRepairAuthorization::Allowed {
            return Ok(IndexDriftRepairOutcome::Denied);
        }
        let authorized = IndexDriftAuthorizedRepairCommand::new(command);
        let (ticket, finding) = match self.store.reserve(&authorized).await? {
            IndexDriftRepairReservationOutcome::Reserved { ticket, finding }
            | IndexDriftRepairReservationOutcome::Resumed { ticket, finding } => (ticket, finding),
            IndexDriftRepairReservationOutcome::AlreadyCompleted(receipt) => {
                return Ok(IndexDriftRepairOutcome::AlreadyCompleted(receipt));
            }
            IndexDriftRepairReservationOutcome::NotReserved(reason) => {
                return Ok(IndexDriftRepairOutcome::NotStarted(reason));
            }
        };

        let owner = self
            .owners
            .owner_for(finding.target().kind())
            .ok_or_else(|| permanent_failure("index_drift_repair_owner_unavailable"))?;
        let before = self.evidence.capture_before(&authorized, &finding).await?;
        if before.state() != IndexDriftRepairEvidenceState::Repairable {
            let completion = not_repaired_completion(
                owner.owner_name(),
                &before,
                None,
                None,
                "before_not_repairable",
            )
            .map_err(|_| permanent_failure("index_drift_repair_completion_invalid"))?;
            return self.complete_ticket(&ticket, &completion).await;
        }

        let owner_outcome = owner.repair(&authorized, &finding, &before).await?;
        let after = self
            .evidence
            .capture_after(&authorized, &finding, &before)
            .await?;
        let completion = match owner_outcome {
            IndexDriftRepairOwnerOutcome::Applied { receipt_digest }
                if after.state() == IndexDriftRepairEvidenceState::Converged =>
            {
                IndexDriftRepairCompletion::new(
                    owner.owner_name().to_owned(),
                    IndexDriftRepairReceiptOutcome::Repaired,
                    before.digest().to_owned(),
                    Some(after.digest().to_owned()),
                    Some(receipt_digest),
                )
            }
            IndexDriftRepairOwnerOutcome::Applied { receipt_digest } => not_repaired_completion(
                owner.owner_name(),
                &before,
                Some(&after),
                Some(receipt_digest),
                "after_not_converged",
            ),
            IndexDriftRepairOwnerOutcome::NotApplied { code } => {
                not_repaired_completion(owner.owner_name(), &before, Some(&after), None, &code)
            }
        }
        .map_err(|_| permanent_failure("index_drift_repair_completion_invalid"))?;
        self.complete_ticket(&ticket, &completion).await
    }

    async fn complete_ticket(
        &self,
        ticket: &IndexDriftRepairTicket,
        completion: &IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairOutcome, IndexDriftRepairFailure> {
        let (receipt, replayed) = match self.store.complete(ticket, completion).await? {
            IndexDriftRepairStoreCompletionOutcome::Completed(receipt) => (receipt, false),
            IndexDriftRepairStoreCompletionOutcome::AlreadyCompleted(receipt) => (receipt, true),
        };
        if replayed {
            return Ok(IndexDriftRepairOutcome::AlreadyCompleted(receipt));
        }
        let repaired = matches!(receipt.outcome(), IndexDriftRepairReceiptOutcome::Repaired);
        Ok(if repaired {
            IndexDriftRepairOutcome::Repaired(receipt)
        } else {
            IndexDriftRepairOutcome::NotRepaired(receipt)
        })
    }
}

impl fmt::Debug for IndexDriftRepairService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftRepairService")
            .field("owners", &self.owners)
            .finish_non_exhaustive()
    }
}

fn not_repaired_completion(
    owner_name: &str,
    before: &IndexDriftRepairEvidence,
    after: Option<&IndexDriftRepairEvidence>,
    owner_receipt_digest: Option<String>,
    code: &str,
) -> Result<IndexDriftRepairCompletion, IndexDriftRepairValidationError> {
    IndexDriftRepairCompletion::new(
        owner_name.to_owned(),
        IndexDriftRepairReceiptOutcome::NotRepaired {
            code: code.to_owned(),
        },
        before.digest().to_owned(),
        after.map(|value| value.digest().to_owned()),
        owner_receipt_digest,
    )
}

fn validate_key_and_versions(
    key: &EntityKey,
    indexed_source_version: u64,
    absence_source_version: u64,
) -> Result<(), IndexDriftRepairValidationError> {
    if key.tenant_id.is_nil() || key.entity_id.is_nil() || key.schema.version.get() == 0 {
        return Err(IndexDriftRepairValidationError::InvalidTargetIdentity);
    }
    if indexed_source_version == 0 || absence_source_version == 0 {
        return Err(IndexDriftRepairValidationError::InvalidSourceVersion);
    }
    Ok(())
}

fn validate_completion_fields(
    outcome: &IndexDriftRepairReceiptOutcome,
    before_digest: &str,
    after_digest: Option<&str>,
    owner_receipt_digest: Option<&str>,
) -> Result<(), IndexDriftRepairValidationError> {
    if !valid_digest(before_digest)
        || after_digest.is_some_and(|value| !valid_digest(value))
        || owner_receipt_digest.is_some_and(|value| !valid_digest(value))
    {
        return Err(IndexDriftRepairValidationError::InvalidDigest);
    }
    match outcome {
        IndexDriftRepairReceiptOutcome::Repaired => {
            if after_digest.is_none() || owner_receipt_digest.is_none() {
                return Err(IndexDriftRepairValidationError::InvalidDigest);
            }
        }
        IndexDriftRepairReceiptOutcome::NotRepaired { code } => validate_machine_name(code)?,
    }
    Ok(())
}

fn validate_machine_name(value: &str) -> Result<(), IndexDriftRepairValidationError> {
    if valid_machine_name(value) {
        Ok(())
    } else {
        Err(IndexDriftRepairValidationError::InvalidMachineName)
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

fn permanent_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::permanent(code).expect("static repair failure code is valid")
}
