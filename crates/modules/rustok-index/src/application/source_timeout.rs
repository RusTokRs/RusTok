use std::time::Duration;

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use tokio::time::timeout;

use super::{
    IndexSource, IndexSourceError, IndexSourceFailure, IndexSourceLoadBatch,
    IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
};
use crate::SchemaRef;

/// Default upper bound for one owner-source `scan` or targeted `load` call.
///
/// A timed-out source call is cancelled by dropping its future and is reported
/// through a bounded retryable failure code. Replay and reconciliation operators
/// must configure leases longer than this bound plus their persistence margin;
/// this source wrapper never extends or heartbeats a job lease.
const DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30);

const INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout";
const INDEX_SOURCE_LOAD_TIMEOUT_CODE: &str = "index_source_load_timeout";

#[derive(Debug)]
struct TimedIndexSource<S> {
    inner: S,
    call_timeout: Duration,
}

impl<S> TimedIndexSource<S> {
    fn new(inner: S, call_timeout: Duration) -> Self {
        debug_assert!(!call_timeout.is_zero());
        Self {
            inner,
            call_timeout,
        }
    }
}

#[async_trait]
impl<S> IndexSource for TimedIndexSource<S>
where
    S: IndexSource,
{
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        match timeout(self.call_timeout, self.inner.scan(request)).await {
            Ok(result) => result,
            Err(_) => Err(retryable_timeout(INDEX_SOURCE_SCAN_TIMEOUT_CODE)),
        }
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        match timeout(self.call_timeout, self.inner.load(request)).await {
            Ok(result) => result,
            Err(_) => Err(retryable_timeout(INDEX_SOURCE_LOAD_TIMEOUT_CODE)),
        }
    }
}

/// Register one production source behind the canonical bounded call-timeout wrapper.
///
/// Direct `IndexSourceCatalog::register` remains available for isolated fixtures and
/// low-level contract tests. Selected production bridges use this helper so a source
/// backend cannot block replay, reconciliation, or targeted repair indefinitely.
pub fn register_index_source<S>(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    source_name: impl Into<String>,
    schemas: impl IntoIterator<Item = SchemaRef>,
    source: S,
) -> Result<(), IndexSourceError>
where
    S: IndexSource + 'static,
{
    super::source_registry::register_index_source(
        extensions,
        owner_module,
        source_name,
        schemas,
        TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT),
    )
}

fn retryable_timeout(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Index source timeout code must be valid")
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use uuid::Uuid;

    use super::*;
    use crate::{EntityName, IndexSourceFailureKind, ModuleName, SchemaVersion};

    struct PendingSource;

    #[async_trait]
    impl IndexSource for PendingSource {
        async fn scan(
            &self,
            _request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            pending().await
        }

        async fn load(
            &self,
            _request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            pending().await
        }
    }

    fn schema_ref() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    #[tokio::test]
    async fn timed_source_classifies_scan_timeout_as_retryable() {
        let source = TimedIndexSource::new(PendingSource, Duration::from_millis(1));
        let request =
            IndexSourceScanRequest::new(Uuid::from_u128(1), schema_ref(), None, 1).unwrap();

        let failure = source.scan(request).await.unwrap_err();
        assert_eq!(failure.kind(), IndexSourceFailureKind::Retryable);
        assert_eq!(failure.code(), INDEX_SOURCE_SCAN_TIMEOUT_CODE);
    }

    #[tokio::test]
    async fn timed_source_classifies_targeted_load_timeout_as_retryable() {
        let source = TimedIndexSource::new(PendingSource, Duration::from_millis(1));
        let key = crate::EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: schema_ref(),
            entity_id: Uuid::from_u128(2),
            locale: None,
        };
        let request = IndexSourceLoadRequest::new(vec![key]).unwrap();

        let failure = source.load(request).await.unwrap_err();
        assert_eq!(failure.kind(), IndexSourceFailureKind::Retryable);
        assert_eq!(failure.code(), INDEX_SOURCE_LOAD_TIMEOUT_CODE);
    }
}
