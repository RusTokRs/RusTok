use std::{fmt, future::Future};

use chrono::Utc;
use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use super::{
    IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,
    IndexReconciliationOperatorContext,
    source_continuation_runtime::IndexSourceContinuationKeyringRuntime,
};
use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;

#[derive(Debug, Error)]
pub enum IndexDriftSourcePageDiagnosisError {
    #[error(
        "Index drift source-page diagnosis requires a request-bound effective permission snapshot"
    )]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index drift source-page diagnosis")]
    Forbidden,
    #[error("Index drift source-page diagnosis limit is invalid: actual={actual}, max={max}")]
    InvalidPageLimit { actual: usize, max: usize },
    #[error("Index drift source-page continuation keyring is not configured")]
    ContinuationUnavailable,
    #[error("Index drift source-page continuation keyring could not be resolved")]
    ContinuationKeyringUnavailable,
    #[error(transparent)]
    Continuation(#[from] rustok_index::IndexSourceContinuationError),
    #[error(transparent)]
    Source(#[from] rustok_index::IndexSourceError),
    #[error("Index drift source-page candidate at position {position} failed diagnosis")]
    Diagnosis {
        position: usize,
        #[source]
        source: IndexDriftDiagnosisOperatorError,
    },
}

/// Result for exactly one owner-source page.
///
/// The raw continuation cursor remains server-owned and is not attached to GraphQL. Finding
/// receipts are retained only for authoritative source `Upsert` plus materialized `Missing`; source
/// entity identifiers and payloads are not copied into the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSourcePageDiagnosisOutcome {
    next_cursor: Option<rustok_index::IndexSourceCursor>,
    scanned_mutation_count: usize,
    candidate_count: usize,
    skipped_delete_count: usize,
    non_missing_count: usize,
    missing_recorded_count: usize,
    receipts: Vec<rustok_index::IndexDriftMismatchReceipt>,
}

impl IndexDriftSourcePageDiagnosisOutcome {
    pub fn next_cursor(&self) -> Option<&rustok_index::IndexSourceCursor> {
        self.next_cursor.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.next_cursor.is_none()
    }

    pub fn scanned_mutation_count(&self) -> usize {
        self.scanned_mutation_count
    }

    pub fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    pub fn skipped_delete_count(&self) -> usize {
        self.skipped_delete_count
    }

    pub fn non_missing_count(&self) -> usize {
        self.non_missing_count
    }

    pub fn missing_recorded_count(&self) -> usize {
        self.missing_recorded_count
    }

    pub fn receipts(&self) -> &[rustok_index::IndexDriftMismatchReceipt] {
        &self.receipts
    }

    pub fn into_next_cursor(self) -> Option<rustok_index::IndexSourceCursor> {
        self.next_cursor
    }
}

/// Sealed result for exactly one owner-source page.
///
/// This outcome carries only an authenticated confidential continuation token. It has no raw
/// `IndexSourceCursor` field or accessor and is the only page result suitable for a future transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSourcePageDiagnosisSealedOutcome {
    next_token: Option<rustok_index::IndexSourceContinuationToken>,
    scanned_mutation_count: usize,
    candidate_count: usize,
    skipped_delete_count: usize,
    non_missing_count: usize,
    missing_recorded_count: usize,
    receipts: Vec<rustok_index::IndexDriftMismatchReceipt>,
}

impl IndexDriftSourcePageDiagnosisSealedOutcome {
    fn from_raw(
        outcome: IndexDriftSourcePageDiagnosisOutcome,
        next_token: Option<rustok_index::IndexSourceContinuationToken>,
    ) -> Self {
        Self {
            next_token,
            scanned_mutation_count: outcome.scanned_mutation_count,
            candidate_count: outcome.candidate_count,
            skipped_delete_count: outcome.skipped_delete_count,
            non_missing_count: outcome.non_missing_count,
            missing_recorded_count: outcome.missing_recorded_count,
            receipts: outcome.receipts,
        }
    }

    pub fn next_token(&self) -> Option<&rustok_index::IndexSourceContinuationToken> {
        self.next_token.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.next_token.is_none()
    }

    pub fn scanned_mutation_count(&self) -> usize {
        self.scanned_mutation_count
    }

    pub fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    pub fn skipped_delete_count(&self) -> usize {
        self.skipped_delete_count
    }

    pub fn non_missing_count(&self) -> usize {
        self.non_missing_count
    }

    pub fn missing_recorded_count(&self) -> usize {
        self.missing_recorded_count
    }

    pub fn receipts(&self) -> &[rustok_index::IndexDriftMismatchReceipt] {
        &self.receipts
    }

    pub fn into_next_token(self) -> Option<rustok_index::IndexSourceContinuationToken> {
        self.next_token
    }
}

/// Server-owned one-page missing-entity candidate diagnosis boundary.
///
/// The runtime scans exactly one already-frozen owner source page, skips retained source deletes,
/// and delegates each source-present candidate sequentially to the guarded missing-only exact
/// operator method. It owns no loop, checkpoint, scheduler, task, repair handle, or transport
/// registration.
#[derive(Clone)]
pub struct IndexDriftSourcePageDiagnosisRuntime {
    sources: rustok_index::SharedIndexSourceRegistry,
    exact: IndexDriftDiagnosisOperatorRuntime,
    continuation: Option<IndexSourceContinuationKeyringRuntime>,
}

impl IndexDriftSourcePageDiagnosisRuntime {
    fn new(
        sources: rustok_index::SharedIndexSourceRegistry,
        exact: IndexDriftDiagnosisOperatorRuntime,
        continuation: Option<IndexSourceContinuationKeyringRuntime>,
    ) -> Self {
        Self {
            sources,
            exact,
            continuation,
        }
    }

    pub async fn diagnose_source_page(
        &self,
        context: IndexReconciliationOperatorContext,
        schema: rustok_index::SchemaRef,
        cursor: Option<rustok_index::IndexSourceCursor>,
        limit: usize,
    ) -> std::result::Result<IndexDriftSourcePageDiagnosisOutcome, IndexDriftSourcePageDiagnosisError>
    {
        let request = authorize_and_build_scan_request(context, schema, cursor, limit)?;
        self.diagnose_request(context, request).await
    }

    /// Diagnoses one bounded source page without allowing raw source cursor JSON across the service
    /// boundary.
    ///
    /// Authorization and page-limit validation precede token parsing. The token is authenticated,
    /// decrypted, and scope-checked before `IndexSourceScanRequest` is constructed. An outgoing raw
    /// cursor is sealed before the result is returned.
    pub async fn diagnose_source_page_sealed(
        &self,
        context: IndexReconciliationOperatorContext,
        schema: rustok_index::SchemaRef,
        continuation: Option<&str>,
        limit: usize,
    ) -> std::result::Result<
        IndexDriftSourcePageDiagnosisSealedOutcome,
        IndexDriftSourcePageDiagnosisError,
    > {
        authorize_context(context)?;
        validate_page_limit(limit)?;
        let keyring = self
            .continuation
            .as_ref()
            .ok_or(IndexDriftSourcePageDiagnosisError::ContinuationUnavailable)?;
        let scope = rustok_index::IndexSourceContinuationScope::from_registry(
            context.tenant_id(),
            schema.clone(),
            &self.sources,
        )?;
        let codec = keyring
            .resolve_codec()
            .await
            .map_err(|_| IndexDriftSourcePageDiagnosisError::ContinuationKeyringUnavailable)?;
        let cursor = continuation
            .map(|encoded| codec.open_encoded(&scope, encoded, Utc::now()))
            .transpose()?;
        let request =
            rustok_index::IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)?;
        let outcome = self.diagnose_request(context, request).await?;
        let next_token = outcome
            .next_cursor
            .as_ref()
            .map(|cursor| codec.seal(&scope, cursor, Utc::now(), keyring.lifetime()))
            .transpose()?;
        Ok(IndexDriftSourcePageDiagnosisSealedOutcome::from_raw(
            outcome, next_token,
        ))
    }

    async fn diagnose_request(
        &self,
        context: IndexReconciliationOperatorContext,
        request: rustok_index::IndexSourceScanRequest,
    ) -> std::result::Result<IndexDriftSourcePageDiagnosisOutcome, IndexDriftSourcePageDiagnosisError>
    {
        let page = self.sources.scan(request).await?;
        diagnose_page_with(page, |position, key| {
            let exact = self.exact.clone();
            async move {
                exact
                    .diagnose_missing_entity_candidate(context, key)
                    .await
                    .map_err(|source| IndexDriftSourcePageDiagnosisError::Diagnosis {
                        position,
                        source,
                    })
            }
        })
        .await
    }
}

impl fmt::Debug for IndexDriftSourcePageDiagnosisRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftSourcePageDiagnosisRuntime")
            .field(
                "sealed_continuation_configured",
                &self.continuation.is_some(),
            )
            .finish_non_exhaustive()
    }
}

fn authorize_context(
    context: IndexReconciliationOperatorContext,
) -> std::result::Result<(), IndexDriftSourcePageDiagnosisError> {
    let permissions = permissions_for(&context.tenant_id(), &context.actor_id())
        .ok_or(IndexDriftSourcePageDiagnosisError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexDriftSourcePageDiagnosisError::Forbidden);
    }
    Ok(())
}

fn validate_page_limit(
    limit: usize,
) -> std::result::Result<(), IndexDriftSourcePageDiagnosisError> {
    if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit) {
        return Err(IndexDriftSourcePageDiagnosisError::InvalidPageLimit {
            actual: limit,
            max: MAX_SOURCE_PAGE_DIAGNOSIS_SIZE,
        });
    }
    Ok(())
}

fn authorize_and_build_scan_request(
    context: IndexReconciliationOperatorContext,
    schema: rustok_index::SchemaRef,
    cursor: Option<rustok_index::IndexSourceCursor>,
    limit: usize,
) -> std::result::Result<rustok_index::IndexSourceScanRequest, IndexDriftSourcePageDiagnosisError> {
    authorize_context(context)?;
    validate_page_limit(limit)?;
    rustok_index::IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)
        .map_err(Into::into)
}

async fn diagnose_page_with<Diagnose, DiagnoseFuture>(
    page: rustok_index::IndexSourcePage,
    mut diagnose: Diagnose,
) -> std::result::Result<IndexDriftSourcePageDiagnosisOutcome, IndexDriftSourcePageDiagnosisError>
where
    Diagnose: FnMut(usize, rustok_index::EntityKey) -> DiagnoseFuture,
    DiagnoseFuture: Future<
        Output = std::result::Result<
            rustok_index::IndexDriftMissingEntityCandidateOutcome,
            IndexDriftSourcePageDiagnosisError,
        >,
    >,
{
    let (mutations, next_cursor) = page.into_parts();
    let scanned_mutation_count = mutations.len();
    let mut candidate_count = 0;
    let mut skipped_delete_count = 0;
    let mut non_missing_count = 0;
    let mut missing_recorded_count = 0;
    let mut receipts = Vec::new();

    for (position, mutation) in mutations.into_iter().enumerate() {
        if matches!(&mutation, rustok_index::IndexMutation::Delete { .. }) {
            skipped_delete_count += 1;
            continue;
        }

        candidate_count += 1;
        match diagnose(position, mutation.key().clone()).await? {
            rustok_index::IndexDriftMissingEntityCandidateOutcome::NotCandidate => {
                non_missing_count += 1;
            }
            rustok_index::IndexDriftMissingEntityCandidateOutcome::MissingRecorded {
                receipt,
                ..
            } => {
                missing_recorded_count += 1;
                receipts.push(receipt);
            }
        }
    }

    Ok(IndexDriftSourcePageDiagnosisOutcome {
        next_cursor,
        scanned_mutation_count,
        candidate_count,
        skipped_delete_count,
        non_missing_count,
        missing_recorded_count,
        receipts,
    })
}

pub(super) fn materialize_index_drift_source_page_diagnosis(
    extensions: &mut ModuleRuntimeExtensions,
    continuation: Option<IndexSourceContinuationKeyringRuntime>,
) -> Result<()> {
    if extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>() {
        return Err(ServerError::Message(
            "guarded Index drift source-page diagnosis runtime is already materialized".to_string(),
        ));
    }

    let Some(sources) = extensions
        .get::<rustok_index::SharedIndexSourceRegistry>()
        .cloned()
    else {
        return Ok(());
    };
    let exact = extensions
        .get::<IndexDriftDiagnosisOperatorRuntime>()
        .cloned()
        .ok_or_else(|| {
            ServerError::Message(
                "Index source registry exists without guarded exact-entity drift diagnosis"
                    .to_string(),
            )
        })?;

    extensions.insert(IndexDriftSourcePageDiagnosisRuntime::new(
        sources,
        exact,
        continuation,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use rustok_api::Permission;
    use rustok_core::UserRole;
    use rustok_index::{
        EntityKey, EntityName, IndexDriftMissingEntityCandidateOutcome, IndexMutation, IndexRecord,
        IndexSourceContinuationToken, IndexSourceCursor, IndexSourcePage, IndexSourceScanRequest,
        LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        IndexDriftSourcePageDiagnosisError, IndexDriftSourcePageDiagnosisOutcome,
        IndexDriftSourcePageDiagnosisSealedOutcome, authorize_and_build_scan_request,
        diagnose_page_with,
    };
    use crate::services::index_replay_runtime_composition::IndexReconciliationOperatorContext;
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    fn schema() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("source-page-diagnosis").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn key(tenant_id: Uuid, entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id,
            schema: schema(),
            entity_id,
            locale: Some(LocaleKey::new("en").unwrap()),
        }
    }

    #[tokio::test]
    async fn authorization_precedes_page_limit_validation() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();

        let missing = with_rbac_request_scope(None, async {
            authorize_and_build_scan_request(context, schema(), None, 0)
        })
        .await
        .expect_err("missing authority must win over invalid limit");
        assert!(matches!(
            missing,
            IndexDriftSourcePageDiagnosisError::MissingRequestAuthority
        ));

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async { authorize_and_build_scan_request(context, schema(), None, 0) },
        )
        .await
        .expect_err("read-only authority must win over invalid limit");
        assert!(matches!(
            forbidden,
            IndexDriftSourcePageDiagnosisError::Forbidden
        ));

        let invalid = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async { authorize_and_build_scan_request(context, schema(), None, 0) },
        )
        .await
        .expect_err("authorized request must reach bounded limit validation");
        assert!(matches!(
            invalid,
            IndexDriftSourcePageDiagnosisError::InvalidPageLimit { actual: 0, max: 32 }
        ));
    }

    #[tokio::test]
    async fn one_page_skips_deletes_and_classifies_each_upsert_once() {
        let tenant_id = Uuid::new_v4();
        let first = key(tenant_id, Uuid::new_v4());
        let deleted = key(tenant_id, Uuid::new_v4());
        let request = IndexSourceScanRequest::new(tenant_id, schema(), None, 2).unwrap();
        let next_cursor = IndexSourceCursor::new(json!({"after": first.entity_id})).unwrap();
        let page = IndexSourcePage::new(
            &request,
            vec![
                IndexMutation::Upsert {
                    event_id: Uuid::new_v4(),
                    record: IndexRecord {
                        key: first.clone(),
                        source_version: 7,
                        fields: BTreeMap::new(),
                        links: Vec::new(),
                    },
                },
                IndexMutation::Delete {
                    event_id: Uuid::new_v4(),
                    key: deleted,
                    source_version: 8,
                },
            ],
            Some(next_cursor.clone()),
        )
        .unwrap();
        let diagnosed = Arc::new(Mutex::new(Vec::new()));
        let captured = diagnosed.clone();

        let outcome = diagnose_page_with(page, move |_, candidate| {
            let captured = captured.clone();
            async move {
                captured.lock().unwrap().push(candidate);
                Ok(IndexDriftMissingEntityCandidateOutcome::NotCandidate)
            }
        })
        .await
        .expect("bounded page should diagnose");

        assert_eq!(diagnosed.lock().unwrap().as_slice(), &[first]);
        assert_eq!(outcome.scanned_mutation_count(), 2);
        assert_eq!(outcome.candidate_count(), 1);
        assert_eq!(outcome.skipped_delete_count(), 1);
        assert_eq!(outcome.non_missing_count(), 1);
        assert_eq!(outcome.missing_recorded_count(), 0);
        assert_eq!(outcome.next_cursor(), Some(&next_cursor));
        assert!(!outcome.is_complete());
        assert!(outcome.receipts().is_empty());
    }

    #[test]
    fn sealed_outcome_replaces_raw_cursor_with_opaque_token() {
        let raw = IndexDriftSourcePageDiagnosisOutcome {
            next_cursor: Some(IndexSourceCursor::new(json!({"after": 7})).unwrap()),
            scanned_mutation_count: 3,
            candidate_count: 2,
            skipped_delete_count: 1,
            non_missing_count: 1,
            missing_recorded_count: 1,
            receipts: Vec::new(),
        };
        let token = IndexSourceContinuationToken::parse("opaque-token").unwrap();
        let sealed = IndexDriftSourcePageDiagnosisSealedOutcome::from_raw(raw, Some(token.clone()));

        assert_eq!(sealed.next_token(), Some(&token));
        assert_eq!(sealed.scanned_mutation_count(), 3);
        assert_eq!(sealed.candidate_count(), 2);
        assert_eq!(sealed.skipped_delete_count(), 1);
        assert_eq!(sealed.non_missing_count(), 1);
        assert_eq!(sealed.missing_recorded_count(), 1);
        assert!(!sealed.is_complete());
    }
}
