use std::{
    collections::BTreeSet,
    fmt,
    future::Future,
    sync::Arc,
};

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexMutation, LocaleKey, LocaleMode, SchemaRef, SchemaRegistry};

use super::{
    IndexSourceCursor, IndexSourceError, IndexSourceScanRequest, SharedIndexSourceRegistry,
};

const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_DELIVERY_ID_BYTES: usize = 191;
const MAX_FAILURE_CODE_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index replay dependency reported a {kind:?} failure ({code})")]
pub struct IndexReplayFailure {
    kind: IndexReplayFailureKind,
    code: String,
}

impl IndexReplayFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexReplayError> {
        Self::new(IndexReplayFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexReplayError> {
        Self::new(IndexReplayFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexReplayFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexReplayError> {
        let code = code.into();
        if !valid_machine_name(&code, MAX_FAILURE_CODE_BYTES) {
            return Err(IndexReplayError::InvalidFailureCode(code));
        }
        Ok(Self { kind, code })
    }

    pub(crate) fn retryable_static(code: &'static str) -> Self {
        Self::retryable(code).expect("static replay failure code is valid")
    }

    pub(crate) fn permanent_static(code: &'static str) -> Self {
        Self::permanent(code).expect("static replay failure code is valid")
    }

    pub fn kind(&self) -> IndexReplayFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayPageRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
    limit: usize,
}

impl IndexReplayPageRequest {
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        limit: usize,
    ) -> Result<Self, IndexReplayError> {
        IndexSourceScanRequest::new(tenant_id, schema.clone(), None, limit)
            .map_err(IndexReplayError::SourceContract)?;
        Ok(Self {
            tenant_id,
            schema,
            locale: None,
            limit,
        })
    }

    pub(crate) fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        limit: usize,
    ) -> Result<Self, IndexReplayError> {
        IndexSourceScanRequest::for_locale(tenant_id, schema.clone(), locale.clone(), None, limit)
            .map_err(IndexReplayError::SourceContract)?;
        Ok(Self {
            tenant_id,
            schema,
            locale: Some(locale),
            limit,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub(crate) fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexReplayCheckpointKey {
    tenant_id: Uuid,
    source_name: String,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
}

impl IndexReplayCheckpointKey {
    pub fn new(
        tenant_id: Uuid,
        source_name: impl Into<String>,
        schema: SchemaRef,
    ) -> Result<Self, IndexReplayError> {
        Self::new_scoped(tenant_id, source_name, schema, None)
    }

    pub(crate) fn for_locale(
        tenant_id: Uuid,
        source_name: impl Into<String>,
        schema: SchemaRef,
        locale: LocaleKey,
    ) -> Result<Self, IndexReplayError> {
        Self::new_scoped(tenant_id, source_name, schema, Some(locale))
    }

    fn new_scoped(
        tenant_id: Uuid,
        source_name: impl Into<String>,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
    ) -> Result<Self, IndexReplayError> {
        if tenant_id.is_nil() {
            return Err(IndexReplayError::NilTenantId);
        }
        let source_name = source_name.into();
        if !valid_machine_name(&source_name, MAX_SOURCE_NAME_BYTES) {
            return Err(IndexReplayError::InvalidSourceName(source_name));
        }
        Ok(Self {
            tenant_id,
            source_name,
            schema,
            locale,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub(crate) fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayCheckpoint {
    key: IndexReplayCheckpointKey,
    cursor: Option<IndexSourceCursor>,
    source_version: Option<u64>,
    last_delivery_id: Option<String>,
}

impl IndexReplayCheckpoint {
    pub fn new(
        key: IndexReplayCheckpointKey,
        cursor: Option<IndexSourceCursor>,
        source_version: Option<u64>,
        last_delivery_id: Option<String>,
    ) -> Result<Self, IndexReplayError> {
        if source_version == Some(0) {
            return Err(IndexReplayError::ZeroCheckpointSourceVersion);
        }
        if let Some(delivery_id) = &last_delivery_id {
            validate_required_storage_key(
                "last delivery id",
                delivery_id,
                MAX_DELIVERY_ID_BYTES,
            )?;
            let parsed = Uuid::parse_str(delivery_id)
                .map_err(|_| IndexReplayError::InvalidCheckpointDeliveryId(delivery_id.clone()))?;
            if parsed.is_nil() {
                return Err(IndexReplayError::InvalidCheckpointDeliveryId(
                    delivery_id.clone(),
                ));
            }
        }
        Ok(Self {
            key,
            cursor,
            source_version,
            last_delivery_id,
        })
    }

    pub fn key(&self) -> &IndexReplayCheckpointKey {
        &self.key
    }

    pub fn cursor(&self) -> Option<&IndexSourceCursor> {
        self.cursor.as_ref()
    }

    pub fn source_version(&self) -> Option<u64> {
        self.source_version
    }

    pub fn last_delivery_id(&self) -> Option<&str> {
        self.last_delivery_id.as_deref()
    }

    pub fn is_complete(&self) -> bool {
        self.cursor.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayMutationOutcome {
    Applied,
    Duplicate,
    StaleIgnored,
}

#[async_trait]
pub trait IndexReplayMutationSink: Send + Sync {
    async fn apply_replay_mutation(
        &self,
        registry: &SchemaRegistry,
        source_name: &str,
        mutation: &IndexMutation,
    ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure>;
}

#[async_trait]
pub trait IndexReplayCheckpointStore: Send + Sync {
    async fn load_replay_checkpoint(
        &self,
        key: &IndexReplayCheckpointKey,
    ) -> Result<Option<IndexReplayCheckpoint>, IndexReplayFailure>;

    async fn commit_replay_checkpoint(
        &self,
        checkpoint: &IndexReplayCheckpoint,
    ) -> Result<(), IndexReplayFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayPageStatus {
    Advanced,
    Complete,
    AlreadyComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayPageOutcome {
    status: IndexReplayPageStatus,
    checkpoint: IndexReplayCheckpoint,
    mutation_count: usize,
    applied_count: usize,
    duplicate_count: usize,
    stale_count: usize,
}

impl IndexReplayPageOutcome {
    pub fn status(&self) -> IndexReplayPageStatus {
        self.status
    }

    pub fn checkpoint(&self) -> &IndexReplayCheckpoint {
        &self.checkpoint
    }

    pub fn mutation_count(&self) -> usize {
        self.mutation_count
    }

    pub fn applied_count(&self) -> usize {
        self.applied_count
    }

    pub fn duplicate_count(&self) -> usize {
        self.duplicate_count
    }

    pub fn stale_count(&self) -> usize {
        self.stale_count
    }
}

pub struct IndexReplayWorker<M, C> {
    sources: SharedIndexSourceRegistry,
    schema_registry: Arc<SchemaRegistry>,
    mutation_sink: M,
    checkpoint_store: C,
}

impl<M, C> fmt::Debug for IndexReplayWorker<M, C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexReplayWorker")
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

impl<M, C> IndexReplayWorker<M, C>
where
    M: IndexReplayMutationSink,
    C: IndexReplayCheckpointStore,
{
    pub fn new(
        sources: SharedIndexSourceRegistry,
        schema_registry: Arc<SchemaRegistry>,
        mutation_sink: M,
        checkpoint_store: C,
    ) -> Self {
        Self {
            sources,
            schema_registry,
            mutation_sink,
            checkpoint_store,
        }
    }

    pub async fn run_next_page(
        &self,
        request: IndexReplayPageRequest,
    ) -> Result<IndexReplayPageOutcome, IndexReplayError> {
        self.run_next_page_interruptible(request, || async {
            Ok::<bool, IndexReplayFailure>(false)
        })
        .await
    }

    /// Runs one replay page with cooperative interruption checks at durable boundaries.
    ///
    /// The probe is called after checkpoint readiness and before source scan, before every
    /// mutation application, and before checkpoint commit. Returning `true` interrupts the
    /// page without advancing its checkpoint. Mutations already committed before an
    /// interruption may be replayed and must remain safe through inbox deduplication and
    /// monotonic source-version guards.
    pub async fn run_next_page_interruptible<Check, CheckFuture>(
        &self,
        request: IndexReplayPageRequest,
        mut should_interrupt: Check,
    ) -> Result<IndexReplayPageOutcome, IndexReplayError>
    where
        Check: FnMut() -> CheckFuture,
        CheckFuture: Future<Output = Result<bool, IndexReplayFailure>>,
    {
        let descriptor = self
            .sources
            .source_for_schema(request.schema())
            .ok_or_else(|| IndexReplayError::UnknownSchemaSource(request.schema().clone()))?;
        let registered = self
            .schema_registry
            .get(request.schema())
            .ok_or_else(|| IndexReplayError::SchemaNotRegistered(request.schema().clone()))?;
        if request.locale().is_some() && registered.schema.locale_mode == LocaleMode::None {
            return Err(IndexReplayError::LocaleScopeUnsupported(
                request.schema().clone(),
            ));
        }

        let checkpoint_key = match request.locale() {
            Some(locale) => IndexReplayCheckpointKey::for_locale(
                request.tenant_id(),
                descriptor.source_name(),
                request.schema().clone(),
                locale.clone(),
            )?,
            None => IndexReplayCheckpointKey::new(
                request.tenant_id(),
                descriptor.source_name(),
                request.schema().clone(),
            )?,
        };
        let current = self
            .checkpoint_store
            .load_replay_checkpoint(&checkpoint_key)
            .await
            .map_err(IndexReplayError::CheckpointReadFailed)?;

        if let Some(checkpoint) = &current {
            if checkpoint.key() != &checkpoint_key {
                return Err(IndexReplayError::CheckpointIdentityMismatch {
                    expected: checkpoint_key,
                    actual: checkpoint.key().clone(),
                });
            }
            if checkpoint.is_complete() {
                return Ok(IndexReplayPageOutcome {
                    status: IndexReplayPageStatus::AlreadyComplete,
                    checkpoint: checkpoint.clone(),
                    mutation_count: 0,
                    applied_count: 0,
                    duplicate_count: 0,
                    stale_count: 0,
                });
            }
        }

        check_replay_interruption(&mut should_interrupt).await?;
        let cursor = current
            .as_ref()
            .and_then(|checkpoint| checkpoint.cursor().cloned());
        let scan_request = match request.locale() {
            Some(locale) => IndexSourceScanRequest::for_locale(
                request.tenant_id(),
                request.schema().clone(),
                locale.clone(),
                cursor,
                request.limit(),
            ),
            None => IndexSourceScanRequest::new(
                request.tenant_id(),
                request.schema().clone(),
                cursor,
                request.limit(),
            ),
        }
        .map_err(IndexReplayError::SourceContract)?;
        let page = self
            .sources
            .scan(scan_request)
            .await
            .map_err(IndexReplayError::SourceContract)?;

        let mut event_ids = BTreeSet::new();
        for (position, mutation) in page.mutations().iter().enumerate() {
            let event_id = mutation.event_id();
            if event_id.is_nil() {
                return Err(IndexReplayError::NilReplayEventId { position });
            }
            if !event_ids.insert(event_id) {
                return Err(IndexReplayError::DuplicateReplayEventId { position, event_id });
            }
        }

        let mut applied_count = 0;
        let mut duplicate_count = 0;
        let mut stale_count = 0;
        let mut max_source_version = current
            .as_ref()
            .and_then(IndexReplayCheckpoint::source_version);
        let mut last_delivery_id = current
            .as_ref()
            .and_then(|checkpoint| checkpoint.last_delivery_id().map(str::to_owned));

        for (position, mutation) in page.mutations().iter().enumerate() {
            check_replay_interruption(&mut should_interrupt).await?;
            let outcome = self
                .mutation_sink
                .apply_replay_mutation(
                    self.schema_registry.as_ref(),
                    descriptor.source_name(),
                    mutation,
                )
                .await
                .map_err(|failure| IndexReplayError::MutationFailed { position, failure })?;
            match outcome {
                IndexReplayMutationOutcome::Applied => applied_count += 1,
                IndexReplayMutationOutcome::Duplicate => duplicate_count += 1,
                IndexReplayMutationOutcome::StaleIgnored => stale_count += 1,
            }
            max_source_version = Some(max_source_version.map_or(
                mutation.source_version(),
                |current| current.max(mutation.source_version()),
            ));
            last_delivery_id = Some(mutation.event_id().to_string());
        }

        let (mutations, next_cursor) = page.into_parts();
        let checkpoint = IndexReplayCheckpoint::new(
            checkpoint_key,
            next_cursor,
            max_source_version,
            last_delivery_id,
        )?;
        check_replay_interruption(&mut should_interrupt).await?;
        self.checkpoint_store
            .commit_replay_checkpoint(&checkpoint)
            .await
            .map_err(IndexReplayError::CheckpointCommitFailed)?;

        Ok(IndexReplayPageOutcome {
            status: if checkpoint.is_complete() {
                IndexReplayPageStatus::Complete
            } else {
                IndexReplayPageStatus::Advanced
            },
            checkpoint,
            mutation_count: mutations.len(),
            applied_count,
            duplicate_count,
            stale_count,
        })
    }
}

async fn check_replay_interruption<Check, CheckFuture>(
    should_interrupt: &mut Check,
) -> Result<(), IndexReplayError>
where
    Check: FnMut() -> CheckFuture,
    CheckFuture: Future<Output = Result<bool, IndexReplayFailure>>,
{
    match should_interrupt().await {
        Ok(false) => Ok(()),
        Ok(true) => Err(IndexReplayError::Interrupted),
        Err(failure) => Err(IndexReplayError::InterruptionCheckFailed(failure)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexReplayError {
    #[error("Index replay tenant cannot be nil")]
    NilTenantId,
    #[error("Index replay source name is invalid: {0}")]
    InvalidSourceName(String),
    #[error("Index replay failure code is invalid: {0}")]
    InvalidFailureCode(String),
    #[error("Index replay {field} is invalid: {reason}")]
    InvalidStorageKey {
        field: &'static str,
        reason: &'static str,
    },
    #[error("Index replay checkpoint source version must be greater than zero")]
    ZeroCheckpointSourceVersion,
    #[error("Index replay checkpoint delivery id is not a non-nil UUID: {0}")]
    InvalidCheckpointDeliveryId(String),
    #[error("No Index replay source owns schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index replay schema is not registered in the active runtime: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("Index replay schema does not support locale-scoped pages: {0}")]
    LocaleScopeUnsupported(SchemaRef),
    #[error(transparent)]
    SourceContract(IndexSourceError),
    #[error("Index replay page was cooperatively interrupted")]
    Interrupted,
    #[error("Index replay interruption check failed")]
    InterruptionCheckFailed(#[source] IndexReplayFailure),
    #[error("Index replay checkpoint identity does not match the requested replay scope")]
    CheckpointIdentityMismatch {
        expected: IndexReplayCheckpointKey,
        actual: IndexReplayCheckpointKey,
    },
    #[error("Index replay mutation at position {position} has a nil event id")]
    NilReplayEventId { position: usize },
    #[error("Index replay mutation at position {position} duplicates event id {event_id}")]
    DuplicateReplayEventId { position: usize, event_id: Uuid },
    #[error("Index replay mutation at position {position} failed")]
    MutationFailed {
        position: usize,
        #[source]
        failure: IndexReplayFailure,
    },
    #[error("Index replay checkpoint read failed")]
    CheckpointReadFailed(#[source] IndexReplayFailure),
    #[error("Index replay checkpoint commit failed")]
    CheckpointCommitFailed(#[source] IndexReplayFailure),
}

fn validate_required_storage_key(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), IndexReplayError> {
    if value.is_empty() {
        return Err(IndexReplayError::InvalidStorageKey {
            field,
            reason: "must not be empty",
        });
    }
    if value.len() > max_bytes {
        return Err(IndexReplayError::InvalidStorageKey {
            field,
            reason: "exceeds the storage limit",
        });
    }
    if value.trim() != value {
        return Err(IndexReplayError::InvalidStorageKey {
            field,
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(IndexReplayError::InvalidStorageKey {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn valid_machine_name(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.')
        })
}
