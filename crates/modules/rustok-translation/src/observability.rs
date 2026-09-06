use std::{
    future::Future,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::{DateTime, FixedOffset, Utc};
use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Opts,
    core::{Collector, Desc},
    proto::MetricFamily,
};
use rustok_translation_targets::{TranslationPatchIssueSeverity, TranslationPatchValidation};
use tracing::Instrument;

use crate::{
    JobProgressRecord, MemoryMatchKind, MemorySuggestion, ProviderProjectionFreshness,
    TranslationError, TranslationInterchangeArtifactRecord, TranslationInterchangeConflictReport,
    TranslationResult,
};

static METRICS: OnceLock<TranslationObservabilityMetrics> = OnceLock::new();
static METRICS_REGISTERED: AtomicBool = AtomicBool::new(false);
static METRICS_REGISTERING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProviderOperation {
    ChangeSync,
    InventoryRebuild,
    AggregateProgress,
}

impl ProviderOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ChangeSync => "change_sync",
            Self::InventoryRebuild => "inventory_rebuild",
            Self::AggregateProgress => "aggregate_progress",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WorkflowOperation {
    ProgressRead,
    ApplyProposal,
    ApplyRecovery,
}

impl WorkflowOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ProgressRead => "progress_read",
            Self::ApplyProposal => "apply_proposal",
            Self::ApplyRecovery => "apply_recovery",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum InterchangeOperation {
    ExportCreate,
    ImportStore,
    ImportProcess,
}

impl InterchangeOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExportCreate => "export_create",
            Self::ImportStore => "import_store",
            Self::ImportProcess => "import_process",
        }
    }
}

#[derive(Clone)]
struct TranslationObservabilityMetrics {
    provider_operations_total: IntCounterVec,
    provider_operation_duration_seconds: HistogramVec,
    provider_failures_total: IntCounterVec,
    provider_projection_freshness_total: IntCounterVec,
    provider_checkpoint_age_seconds: HistogramVec,
    workflow_operations_total: IntCounterVec,
    workflow_operation_duration_seconds: HistogramVec,
    workflow_failures_total: IntCounterVec,
    workflow_progress_snapshots_total: IntCounter,
    workflow_apply_attempts_total: IntCounterVec,
    workflow_owner_apply_duration_seconds: HistogramVec,
    workflow_owner_errors_total: IntCounterVec,
    memory_lookups_total: IntCounterVec,
    qa_validations_total: IntCounterVec,
    qa_violations_total: IntCounterVec,
    interchange_operations_total: IntCounterVec,
    interchange_operation_duration_seconds: HistogramVec,
    interchange_rejections_total: IntCounterVec,
    interchange_artifact_bytes: HistogramVec,
    interchange_import_items_total: IntCounterVec,
    interchange_expiry_cleanup_total: IntCounterVec,
}

impl TranslationObservabilityMetrics {
    fn new() -> Self {
        Self {
            provider_operations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_provider_operations_total",
                    "Translation provider operations by bounded operation and outcome",
                ),
                &["operation", "outcome"],
            )
            .expect("translation provider operation metric must be valid"),
            provider_operation_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_provider_operation_duration_seconds",
                    "Duration of Translation provider operations",
                )
                .buckets(operation_duration_buckets()),
                &["operation", "outcome"],
            )
            .expect("translation provider duration metric must be valid"),
            provider_failures_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_provider_failures_total",
                    "Translation provider operation failures by bounded category",
                ),
                &["operation", "category"],
            )
            .expect("translation provider failure metric must be valid"),
            provider_projection_freshness_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_provider_projection_freshness_total",
                    "Observed Translation provider projection freshness states",
                ),
                &["freshness"],
            )
            .expect("translation provider freshness metric must be valid"),
            provider_checkpoint_age_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_provider_checkpoint_age_seconds",
                    "Age of an observed Translation provider checkpoint; this is not cursor distance",
                )
                .buckets(checkpoint_age_buckets()),
                &["freshness"],
            )
            .expect("translation provider checkpoint age metric must be valid"),
            workflow_operations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_workflow_operations_total",
                    "Translation workflow operations by bounded operation and outcome",
                ),
                &["operation", "outcome"],
            )
            .expect("translation workflow operation metric must be valid"),
            workflow_operation_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_workflow_operation_duration_seconds",
                    "Duration of Translation workflow operations",
                )
                .buckets(operation_duration_buckets()),
                &["operation", "outcome"],
            )
            .expect("translation workflow duration metric must be valid"),
            workflow_failures_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_workflow_failures_total",
                    "Translation workflow operation failures by bounded category",
                ),
                &["operation", "category"],
            )
            .expect("translation workflow failure metric must be valid"),
            workflow_progress_snapshots_total: IntCounter::new(
                "rustok_translation_workflow_progress_snapshots_total",
                "Content-free Translation job-progress snapshots observed by an authorized caller",
            )
            .expect("translation workflow progress snapshot metric must be valid"),
            workflow_apply_attempts_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_workflow_apply_attempts_total",
                    "Translation owner-apply lifecycle observations by bounded outcome",
                ),
                &["outcome"],
            )
            .expect("translation workflow apply attempt metric must be valid"),
            workflow_owner_apply_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_workflow_owner_apply_duration_seconds",
                    "Duration of a Translation owner-apply call",
                )
                .buckets(operation_duration_buckets()),
                &["outcome"],
            )
            .expect("translation workflow owner apply duration metric must be valid"),
            workflow_owner_errors_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_workflow_owner_errors_total",
                    "Translation owner-apply errors by bounded kind and retryability",
                ),
                &["kind", "retryable"],
            )
            .expect("translation workflow owner error metric must be valid"),
            memory_lookups_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_memory_lookups_total",
                    "Translation Memory lookups by the strongest returned match kind",
                ),
                &["outcome"],
            )
            .expect("translation memory lookup metric must be valid"),
            qa_validations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_qa_validations_total",
                    "Translation QA validations by bounded outcome",
                ),
                &["outcome"],
            )
            .expect("translation QA validation metric must be valid"),
            qa_violations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_qa_violations_total",
                    "Translation QA violations by bounded source family and severity",
                ),
                &["family", "severity"],
            )
            .expect("translation QA violation metric must be valid"),
            interchange_operations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_interchange_operations_total",
                    "Translation interchange artifact operations by bounded operation and outcome",
                ),
                &["operation", "outcome"],
            )
            .expect("translation interchange operation metric must be valid"),
            interchange_operation_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_interchange_operation_duration_seconds",
                    "Duration of Translation interchange artifact operations",
                )
                .buckets(operation_duration_buckets()),
                &["operation", "outcome"],
            )
            .expect("translation interchange duration metric must be valid"),
            interchange_rejections_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_interchange_rejections_total",
                    "Translation interchange artifact rejections by bounded category",
                ),
                &["operation", "category"],
            )
            .expect("translation interchange rejection metric must be valid"),
            interchange_artifact_bytes: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_translation_interchange_artifact_bytes",
                    "Serialized byte size of successful Translation interchange artifacts",
                )
                .buckets(artifact_size_buckets()),
                &["operation"],
            )
            .expect("translation interchange size metric must be valid"),
            interchange_import_items_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_interchange_import_items_total",
                    "Translation interchange import items by aggregate outcome",
                ),
                &["outcome"],
            )
            .expect("translation interchange item outcome metric must be valid"),
            interchange_expiry_cleanup_total: IntCounterVec::new(
                Opts::new(
                    "rustok_translation_interchange_expiry_cleanup_total",
                    "Translation interchange artifact expiry cleanup by bounded storage outcome",
                ),
                &["outcome"],
            )
            .expect("translation interchange expiry cleanup metric must be valid"),
        }
    }

    fn record_provider_operation<T>(
        &self,
        operation: ProviderOperation,
        result: &TranslationResult<T>,
        duration: Duration,
    ) {
        let operation = operation.as_str();
        let outcome = outcome(result);
        self.provider_operations_total
            .with_label_values(&[operation, outcome])
            .inc();
        self.provider_operation_duration_seconds
            .with_label_values(&[operation, outcome])
            .observe(duration.as_secs_f64());
        if let Err(error) = result {
            self.provider_failures_total
                .with_label_values(&[operation, error_category(error)])
                .inc();
        }
    }

    fn record_provider_projection(
        &self,
        freshness: ProviderProjectionFreshness,
        checkpoint_age: Option<Duration>,
    ) {
        let freshness = freshness_label(freshness);
        self.provider_projection_freshness_total
            .with_label_values(&[freshness])
            .inc();
        if let Some(age) = checkpoint_age {
            self.provider_checkpoint_age_seconds
                .with_label_values(&[freshness])
                .observe(age.as_secs_f64());
        }
    }

    fn record_workflow_operation<T>(
        &self,
        operation: WorkflowOperation,
        result: &TranslationResult<T>,
        duration: Duration,
    ) {
        let operation = operation.as_str();
        let outcome = outcome(result);
        self.workflow_operations_total
            .with_label_values(&[operation, outcome])
            .inc();
        self.workflow_operation_duration_seconds
            .with_label_values(&[operation, outcome])
            .observe(duration.as_secs_f64());
        if let Err(error) = result {
            self.workflow_failures_total
                .with_label_values(&[operation, error_category(error)])
                .inc();
        }
    }

    fn record_interchange_operation(
        &self,
        operation: InterchangeOperation,
        result: &TranslationResult<TranslationInterchangeArtifactRecord>,
        duration: Duration,
    ) {
        let operation_label = operation.as_str();
        let outcome = outcome(result);
        self.interchange_operations_total
            .with_label_values(&[operation_label, outcome])
            .inc();
        self.interchange_operation_duration_seconds
            .with_label_values(&[operation_label, outcome])
            .observe(duration.as_secs_f64());
        match result {
            Ok(artifact) => self
                .interchange_artifact_bytes
                .with_label_values(&[operation_label])
                .observe(artifact.content_length as f64),
            Err(error) => self
                .interchange_rejections_total
                .with_label_values(&[operation_label, error_category(error)])
                .inc(),
        }
    }
}

impl Collector for TranslationObservabilityMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut descriptions = Vec::new();
        descriptions.extend(self.provider_operations_total.desc());
        descriptions.extend(self.provider_operation_duration_seconds.desc());
        descriptions.extend(self.provider_failures_total.desc());
        descriptions.extend(self.provider_projection_freshness_total.desc());
        descriptions.extend(self.provider_checkpoint_age_seconds.desc());
        descriptions.extend(self.workflow_operations_total.desc());
        descriptions.extend(self.workflow_operation_duration_seconds.desc());
        descriptions.extend(self.workflow_failures_total.desc());
        descriptions.extend(self.workflow_progress_snapshots_total.desc());
        descriptions.extend(self.workflow_apply_attempts_total.desc());
        descriptions.extend(self.workflow_owner_apply_duration_seconds.desc());
        descriptions.extend(self.workflow_owner_errors_total.desc());
        descriptions.extend(self.memory_lookups_total.desc());
        descriptions.extend(self.qa_validations_total.desc());
        descriptions.extend(self.qa_violations_total.desc());
        descriptions.extend(self.interchange_operations_total.desc());
        descriptions.extend(self.interchange_operation_duration_seconds.desc());
        descriptions.extend(self.interchange_rejections_total.desc());
        descriptions.extend(self.interchange_artifact_bytes.desc());
        descriptions.extend(self.interchange_import_items_total.desc());
        descriptions.extend(self.interchange_expiry_cleanup_total.desc());
        descriptions
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut families = Vec::new();
        families.extend(self.provider_operations_total.collect());
        families.extend(self.provider_operation_duration_seconds.collect());
        families.extend(self.provider_failures_total.collect());
        families.extend(self.provider_projection_freshness_total.collect());
        families.extend(self.provider_checkpoint_age_seconds.collect());
        families.extend(self.workflow_operations_total.collect());
        families.extend(self.workflow_operation_duration_seconds.collect());
        families.extend(self.workflow_failures_total.collect());
        families.extend(self.workflow_progress_snapshots_total.collect());
        families.extend(self.workflow_apply_attempts_total.collect());
        families.extend(self.workflow_owner_apply_duration_seconds.collect());
        families.extend(self.workflow_owner_errors_total.collect());
        families.extend(self.memory_lookups_total.collect());
        families.extend(self.qa_validations_total.collect());
        families.extend(self.qa_violations_total.collect());
        families.extend(self.interchange_operations_total.collect());
        families.extend(self.interchange_operation_duration_seconds.collect());
        families.extend(self.interchange_rejections_total.collect());
        families.extend(self.interchange_artifact_bytes.collect());
        families.extend(self.interchange_import_items_total.collect());
        families.extend(self.interchange_expiry_cleanup_total.collect());
        families
    }
}

pub(crate) async fn observe_provider_operation<T>(
    operation: ProviderOperation,
    work: impl Future<Output = TranslationResult<T>>,
) -> TranslationResult<T> {
    let (result, duration) = observe_operation("provider", operation.as_str(), work).await;
    metrics().record_provider_operation(operation, &result, duration);
    record_result_trace("provider", operation.as_str(), &result, duration);
    result
}

pub(crate) async fn observe_workflow_operation<T>(
    operation: WorkflowOperation,
    work: impl Future<Output = TranslationResult<T>>,
) -> TranslationResult<T> {
    let (result, duration) = observe_operation("workflow", operation.as_str(), work).await;
    metrics().record_workflow_operation(operation, &result, duration);
    record_result_trace("workflow", operation.as_str(), &result, duration);
    result
}

pub(crate) async fn observe_interchange_operation(
    operation: InterchangeOperation,
    work: impl Future<Output = TranslationResult<TranslationInterchangeArtifactRecord>>,
) -> TranslationResult<TranslationInterchangeArtifactRecord> {
    let (result, duration) = observe_operation("interchange", operation.as_str(), work).await;
    metrics().record_interchange_operation(operation, &result, duration);
    record_result_trace("interchange", operation.as_str(), &result, duration);
    result
}

pub(crate) fn record_provider_projection(
    freshness: ProviderProjectionFreshness,
    checkpoint_updated_at: Option<DateTime<FixedOffset>>,
) {
    ensure_registered();
    let checkpoint_age = checkpoint_updated_at.and_then(checkpoint_age);
    metrics().record_provider_projection(freshness, checkpoint_age);
    tracing::debug!(
        component = "translation",
        operation = "provider_projection",
        freshness = freshness_label(freshness),
        checkpoint_age_seconds = checkpoint_age.map(|age| age.as_secs_f64()),
        "Observed Translation provider projection freshness"
    );
}

pub(crate) fn record_workflow_progress_snapshot(progress: &JobProgressRecord) {
    ensure_registered();
    metrics().workflow_progress_snapshots_total.inc();
    tracing::debug!(
        component = "translation",
        operation = "workflow_progress_snapshot",
        total_items = progress.total_items,
        missing_items = progress.missing_items,
        draft_items = progress.draft_items,
        in_review_items = progress.in_review_items,
        approved_items = progress.approved_items,
        applying_items = progress.applying_items,
        applied_items = progress.applied_items,
        stale_items = progress.stale_items,
        conflict_items = progress.conflict_items,
        blocked_items = progress.blocked_items,
        excluded_items = progress.excluded_items,
        cancelled_items = progress.cancelled_items,
        "Observed content-free Translation workflow progress snapshot"
    );
}

pub(crate) fn record_apply_attempt_started() {
    ensure_registered();
    metrics()
        .workflow_apply_attempts_total
        .with_label_values(&["started"])
        .inc();
}

pub(crate) fn record_apply_replay() {
    ensure_registered();
    metrics()
        .workflow_apply_attempts_total
        .with_label_values(&["replayed"])
        .inc();
}

pub(crate) fn record_owner_apply_success(duration: Duration) {
    ensure_registered();
    metrics()
        .workflow_owner_apply_duration_seconds
        .with_label_values(&["success"])
        .observe(duration.as_secs_f64());
}

pub(crate) fn record_owner_apply_failure(kind: &'static str, retryable: bool, duration: Duration) {
    ensure_registered();
    let kind = owner_error_kind(kind);
    let retryable_label = if retryable { "true" } else { "false" };
    let outcome = if retryable {
        "retryable_owner_error"
    } else if kind == "conflict" {
        "conflict"
    } else {
        "blocked"
    };
    let metrics = metrics();
    metrics
        .workflow_owner_apply_duration_seconds
        .with_label_values(&["failure"])
        .observe(duration.as_secs_f64());
    metrics
        .workflow_owner_errors_total
        .with_label_values(&[kind, retryable_label])
        .inc();
    metrics
        .workflow_apply_attempts_total
        .with_label_values(&[outcome])
        .inc();
    tracing::warn!(
        component = "translation",
        operation = "owner_apply",
        owner_error_kind = kind,
        retryable,
        "Translation owner-apply failed"
    );
}

pub(crate) fn record_owner_apply_invalid_receipt() {
    ensure_registered();
    let metrics = metrics();
    metrics
        .workflow_apply_attempts_total
        .with_label_values(&["invalid_receipt"])
        .inc();
    metrics
        .workflow_owner_errors_total
        .with_label_values(&["invariant_violation", "true"])
        .inc();
}

pub(crate) fn record_owner_apply_completed() {
    ensure_registered();
    metrics()
        .workflow_apply_attempts_total
        .with_label_values(&["completed"])
        .inc();
}

pub(crate) fn record_owner_apply_finalization_failure() {
    ensure_registered();
    metrics()
        .workflow_apply_attempts_total
        .with_label_values(&["finalization_failure"])
        .inc();
}

pub(crate) fn record_memory_lookup(suggestions: &[MemorySuggestion]) {
    ensure_registered();
    let outcome = memory_lookup_outcome(suggestions);
    metrics()
        .memory_lookups_total
        .with_label_values(&[outcome])
        .inc();
    tracing::debug!(
        component = "translation",
        operation = "memory_lookup",
        outcome,
        suggestion_count = suggestions.len(),
        "Observed Translation Memory lookup"
    );
}

pub(crate) fn record_qa_validation(validation: &TranslationPatchValidation) {
    ensure_registered();
    let outcome = if validation.accepted {
        if validation.issues.is_empty() {
            "accepted"
        } else {
            "accepted_with_warnings"
        }
    } else {
        "rejected"
    };
    let metrics = metrics();
    metrics
        .qa_validations_total
        .with_label_values(&[outcome])
        .inc();
    for issue in &validation.issues {
        metrics
            .qa_violations_total
            .with_label_values(&[
                qa_issue_family(&issue.code),
                qa_issue_severity(issue.severity),
            ])
            .inc();
    }
}

pub(crate) fn record_interchange_import_report(report: &TranslationInterchangeConflictReport) {
    ensure_registered();
    let metrics = metrics();
    metrics
        .interchange_import_items_total
        .with_label_values(&["accepted"])
        .inc_by(u64::from(report.accepted_items));
    metrics
        .interchange_import_items_total
        .with_label_values(&["conflict"])
        .inc_by(u64::from(report.conflict_items));
    metrics
        .interchange_import_items_total
        .with_label_values(&["rejected"])
        .inc_by(u64::from(report.rejected_items));
}

pub(crate) fn record_interchange_expiry_cleanup(storage_deleted: bool) {
    ensure_registered();
    metrics()
        .interchange_expiry_cleanup_total
        .with_label_values(&[if storage_deleted {
            "storage_deleted"
        } else {
            "storage_delete_failed"
        }])
        .inc();
}

async fn observe_operation<T>(
    domain: &'static str,
    operation: &'static str,
    work: impl Future<Output = TranslationResult<T>>,
) -> (TranslationResult<T>, Duration) {
    ensure_registered();
    let started_at = Instant::now();
    let span = tracing::info_span!(
        "translation.operation",
        component = "translation",
        domain,
        operation,
    );
    let result = work.instrument(span).await;
    (result, started_at.elapsed())
}

fn ensure_registered() {
    if METRICS_REGISTERED.load(Ordering::Acquire)
        || METRICS_REGISTERING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }

    if rustok_telemetry::register_runtime_collector(Box::new(metrics().clone())).is_ok() {
        METRICS_REGISTERED.store(true, Ordering::Release);
    }
    METRICS_REGISTERING.store(false, Ordering::Release);
}

fn metrics() -> &'static TranslationObservabilityMetrics {
    METRICS.get_or_init(TranslationObservabilityMetrics::new)
}

fn record_result_trace<T>(
    domain: &'static str,
    operation: &'static str,
    result: &TranslationResult<T>,
    duration: Duration,
) {
    match result {
        Ok(_) => tracing::debug!(
            component = "translation",
            domain,
            operation,
            duration_seconds = duration.as_secs_f64(),
            "Translation operation completed"
        ),
        Err(error) => tracing::warn!(
            component = "translation",
            domain,
            operation,
            error_category = error_category(error),
            duration_seconds = duration.as_secs_f64(),
            "Translation operation failed"
        ),
    }
}

fn outcome<T>(result: &TranslationResult<T>) -> &'static str {
    if result.is_ok() { "success" } else { "failure" }
}

fn freshness_label(freshness: ProviderProjectionFreshness) -> &'static str {
    match freshness {
        ProviderProjectionFreshness::Current => "current",
        ProviderProjectionFreshness::Behind => "behind",
        ProviderProjectionFreshness::Unknown => "unknown",
    }
}

fn checkpoint_age(timestamp: DateTime<FixedOffset>) -> Option<Duration> {
    Utc::now()
        .signed_duration_since(timestamp.with_timezone(&Utc))
        .to_std()
        .ok()
}

fn error_category(error: &TranslationError) -> &'static str {
    match error {
        TranslationError::Forbidden => "forbidden",
        TranslationError::Provider { .. }
        | TranslationError::ProviderNotFound { .. }
        | TranslationError::ChangeCursorUnavailable
        | TranslationError::ProviderIdentityMismatch
        | TranslationError::MissingCheckpointCursor
        | TranslationError::CursorDidNotAdvance
        | TranslationError::FullRescanUnavailable
        | TranslationError::AggregateProgressUnavailable
        | TranslationError::InvalidProviderProgress(_)
        | TranslationError::InvalidProviderCheckpoint(_)
        | TranslationError::FullRescanChangeDrainLimit
        | TranslationError::FullRescanCursorDidNotAdvance
        | TranslationError::FullRescanPageOverflow
        | TranslationError::FullRescanResourceLimit
        | TranslationError::InvalidProviderValidation(_)
        | TranslationError::InvalidProviderReceipt(_)
        | TranslationError::ProviderReceiptMismatch => "provider",
        TranslationError::Database(_) | TranslationError::Event(_) => "infrastructure",
        TranslationError::WorkflowRevisionConflict
        | TranslationError::IdempotencyConflict
        | TranslationError::IdempotencyActorMismatch
        | TranslationError::CheckpointConflict
        | TranslationError::ProgressRevisionConflict
        | TranslationError::MemoryRevisionConflict { .. }
        | TranslationError::GlossaryRevisionConflict { .. }
        | TranslationError::TranslationPolicyConflict { .. }
        | TranslationError::ApplyInProgress
        | TranslationError::InterchangeArtifactInProgress => "conflict",
        TranslationError::InvalidRequest(_)
        | TranslationError::InvalidTenantId
        | TranslationError::InvalidWorkflowActor
        | TranslationError::InvalidCancellationReason
        | TranslationError::InvalidRetryReason
        | TranslationError::InvalidRecoveryReason
        | TranslationError::InvalidMachineCancellationReason
        | TranslationError::InvalidMachineRecoveryReason => "validation",
        TranslationError::InterchangeArtifactNotFound
        | TranslationError::InterchangeArtifactExpired
        | TranslationError::InterchangeArtifactNotReady
        | TranslationError::InterchangeArtifactAlreadyProcessed => "artifact_state",
        _ => "workflow",
    }
}

fn owner_error_kind(kind: &'static str) -> &'static str {
    match kind {
        "validation"
        | "timeout"
        | "unavailable"
        | "not_found"
        | "conflict"
        | "forbidden"
        | "invariant_violation" => kind,
        _ => "other",
    }
}

fn memory_lookup_outcome(suggestions: &[MemorySuggestion]) -> &'static str {
    if suggestions
        .iter()
        .any(|suggestion| suggestion.evidence.kind == MemoryMatchKind::Exact)
    {
        "exact"
    } else if suggestions
        .iter()
        .any(|suggestion| suggestion.evidence.kind == MemoryMatchKind::ContextualFuzzy)
    {
        "contextual_fuzzy"
    } else if suggestions
        .iter()
        .any(|suggestion| suggestion.evidence.kind == MemoryMatchKind::Fuzzy)
    {
        "fuzzy"
    } else {
        "miss"
    }
}

fn qa_issue_family(code: &str) -> &'static str {
    if code.starts_with("translation.qa.") {
        "deterministic"
    } else if code.starts_with("translation.glossary.") {
        "glossary"
    } else {
        "owner"
    }
}

fn qa_issue_severity(severity: TranslationPatchIssueSeverity) -> &'static str {
    match severity {
        TranslationPatchIssueSeverity::Error => "error",
        TranslationPatchIssueSeverity::Warning => "warning",
    }
}

fn operation_duration_buckets() -> Vec<f64> {
    vec![
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
    ]
}

fn checkpoint_age_buckets() -> Vec<f64> {
    vec![
        1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 3600.0, 14_400.0, 86_400.0,
    ]
}

fn artifact_size_buckets() -> Vec<f64> {
    vec![
        256.0,
        1024.0,
        4_096.0,
        16_384.0,
        65_536.0,
        262_144.0,
        1_048_576.0,
        4_194_304.0,
        8_388_608.0,
    ]
}

#[cfg(test)]
mod tests {
    use prometheus::{Encoder, Registry, TextEncoder};

    use super::{
        InterchangeOperation, ProviderOperation, TranslationObservabilityMetrics,
        WorkflowOperation, error_category, memory_lookup_outcome, qa_issue_family,
    };
    use crate::TranslationError;

    #[test]
    fn metric_labels_are_fixed_and_never_reuse_provider_or_artifact_identity() {
        assert_eq!(ProviderOperation::ChangeSync.as_str(), "change_sync");
        assert_eq!(
            ProviderOperation::AggregateProgress.as_str(),
            "aggregate_progress"
        );
        assert_eq!(WorkflowOperation::ProgressRead.as_str(), "progress_read");
        assert_eq!(WorkflowOperation::ApplyProposal.as_str(), "apply_proposal");
        assert_eq!(
            InterchangeOperation::ImportProcess.as_str(),
            "import_process"
        );
        assert_eq!(
            error_category(&TranslationError::Provider {
                code: "provider.dynamic.code".to_string(),
                message: "private detail".to_string(),
                retryable: true,
            }),
            "provider"
        );
        assert_eq!(
            qa_issue_family("translation.qa.required_value_empty"),
            "deterministic"
        );
        assert_eq!(
            qa_issue_family("translation.glossary.preferred_term_missing"),
            "glossary"
        );
        assert_eq!(qa_issue_family("owner.private.dynamic_code"), "owner");
        assert_eq!(memory_lookup_outcome(&[]), "miss");
    }

    #[test]
    fn collector_renders_only_translation_metric_families() {
        let metrics = TranslationObservabilityMetrics::new();
        metrics
            .provider_operations_total
            .with_label_values(&["change_sync", "success"])
            .inc();
        metrics
            .workflow_apply_attempts_total
            .with_label_values(&["completed"])
            .inc();
        metrics
            .interchange_import_items_total
            .with_label_values(&["accepted"])
            .inc_by(2);

        let registry = Registry::new();
        registry
            .register(Box::new(metrics))
            .expect("translation observability collector registration");
        let mut encoded = Vec::new();
        TextEncoder::new()
            .encode(&registry.gather(), &mut encoded)
            .expect("translation observability encoding");
        let rendered = String::from_utf8(encoded).expect("translation observability utf8");

        assert!(rendered.contains("rustok_translation_provider_operations_total"));
        assert!(rendered.contains("rustok_translation_workflow_apply_attempts_total"));
        assert!(rendered.contains("rustok_translation_interchange_import_items_total"));
        assert!(!rendered.contains("tenant_id"));
        assert!(!rendered.contains("resource_id"));
    }
}
