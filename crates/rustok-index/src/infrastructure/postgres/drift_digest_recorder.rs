use async_trait::async_trait;

use crate::{
    IndexDriftDependencyFailure, IndexDriftDigestError, IndexDriftDigestMismatch,
    IndexDriftMismatchReceipt, IndexDriftMismatchRecordStatus, IndexDriftMismatchRecorder,
};

use super::drift_finding_inspector::{IndexDriftFindingScope, IndexDriftFindingSeverity};
use super::drift_finding_writer::{
    IndexDriftDigestFindingRequest, IndexDriftFindingWriteError, IndexDriftFindingWriteOutcome,
    PostgresIndexDriftFindingWriter,
};

const CHECK_NAME: &str = "source_index_digest_mismatch";
const LOCALE_SCOPE_UNSUPPORTED: &str = "index_drift_locale_free_scope_unsupported";
const REQUEST_REJECTED: &str = "index_drift_finding_request_rejected";
const STORAGE_FAILED: &str = "index_drift_finding_storage_failed";
const BACKEND_UNSUPPORTED: &str = "index_drift_finding_backend_unsupported";
const RECEIPT_INVALID: &str = "index_drift_finding_receipt_invalid";

#[async_trait]
impl IndexDriftMismatchRecorder for PostgresIndexDriftFindingWriter {
    async fn record_digest_mismatch(
        &self,
        mismatch: &IndexDriftDigestMismatch,
    ) -> Result<IndexDriftMismatchReceipt, IndexDriftDependencyFailure> {
        let key = mismatch.key();
        let Some(locale) = key.locale.clone() else {
            return Err(permanent_failure(LOCALE_SCOPE_UNSUPPORTED));
        };
        let request = IndexDriftDigestFindingRequest::new(
            key.tenant_id,
            CHECK_NAME,
            IndexDriftFindingSeverity::Error,
            IndexDriftFindingScope::Entity {
                schema: key.schema.clone(),
                entity_id: key.entity_id,
                locale,
            },
            mismatch.source_digest(),
            mismatch.materialized_digest(),
        )
        .map_err(map_request_error)?;

        let outcome = PostgresIndexDriftFindingWriter::record_digest_mismatch(self, &request)
            .await
            .map_err(map_write_error)?;
        let (status, finding_id, finding_key) = match outcome {
            IndexDriftFindingWriteOutcome::Created {
                finding_id,
                finding_key,
            } => (
                IndexDriftMismatchRecordStatus::Created,
                finding_id,
                finding_key,
            ),
            IndexDriftFindingWriteOutcome::Refreshed {
                finding_id,
                finding_key,
            } => (
                IndexDriftMismatchRecordStatus::Refreshed,
                finding_id,
                finding_key,
            ),
            IndexDriftFindingWriteOutcome::Reopened {
                finding_id,
                finding_key,
            } => (
                IndexDriftMismatchRecordStatus::Reopened,
                finding_id,
                finding_key,
            ),
            IndexDriftFindingWriteOutcome::Suppressed {
                finding_id,
                finding_key,
            } => (
                IndexDriftMismatchRecordStatus::Suppressed,
                finding_id,
                finding_key,
            ),
        };
        IndexDriftMismatchReceipt::new(status, finding_id, finding_key)
            .map_err(|_| permanent_failure(RECEIPT_INVALID))
    }
}

fn map_request_error(_error: IndexDriftFindingWriteError) -> IndexDriftDependencyFailure {
    permanent_failure(REQUEST_REJECTED)
}

fn map_write_error(error: IndexDriftFindingWriteError) -> IndexDriftDependencyFailure {
    match error {
        IndexDriftFindingWriteError::Storage => retryable_failure(STORAGE_FAILED),
        IndexDriftFindingWriteError::UnsupportedBackend => permanent_failure(BACKEND_UNSUPPORTED),
        _ => permanent_failure(REQUEST_REJECTED),
    }
}

fn retryable_failure(code: &'static str) -> IndexDriftDependencyFailure {
    IndexDriftDependencyFailure::retryable(code)
        .expect("static Index drift retryable failure code is valid")
}

fn permanent_failure(code: &'static str) -> IndexDriftDependencyFailure {
    IndexDriftDependencyFailure::permanent(code)
        .expect("static Index drift permanent failure code is valid")
}

#[allow(dead_code)]
fn _error_contract_is_public(_: IndexDriftDigestError) {}
