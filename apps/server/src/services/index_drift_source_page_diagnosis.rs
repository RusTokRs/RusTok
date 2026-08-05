use std::{fmt, future::Future};

use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use super::{
    IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,
    IndexReconciliationOperatorContext,
};
use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;

#[derive(Debug, Error)]
pub enum IndexDriftSourcePageDiagnosisError {
    #[error("Index drift source-page diagnosis requires a request-bound effective permission snapshot")]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index drift source-page diagnosis")]
    Forbidden,
    #[error(
        "Index drift source-page diagnosis limit is invalid: actual={actual}, max={max}"
    )]
    InvalidPageLimit { actual: usize, max: usize },
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
/// The continuation cursor remains server-owned and is not attached to GraphQL. Finding receipts
/// are the only per-candidate values retained; source entity identifiers and payloads are not
/// copied into the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftSourcePageDiagnosisOutcome {
    next_cursor: Option<rustok_index::IndexSourceCursor>,
    scanned_mutation_count: usize,
    candidate_count: usize,
    skipped_delete_count: usize,
    consistent_count: usize,
    mismatch_recorded_count: usize,
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

    pub fn consistent_count(&self) -> usize {
        self.consistent_count
    }

    pub fn mismatch_recorded_count(&self) -> usize {
        self.mismatch_recorded_count
    }

    pub fn receipts(&self) -> &[rustok_index::IndexDriftMismatchReceipt] {
        &self.receipts
    }

    pub fn into_next_cursor(self) -> Option<rustok_index::IndexSourceCursor> {
        self.next_cursor
    }
}

/// Server-owned one-page candidate diagnosis boundary.
///
/// The runtime scans exactly one already-frozen owner source page, skips retained source deletes,
/// and delegates each source-present candidate sequentially to the existing exact-entity operator.
/// It owns no loop, checkpoint, scheduler, task, repair handle, or transport registration.
#[derive(Clone)]
pub struct IndexDriftSourcePageDiagnosisRuntime {
    sources: rustok_index::SharedIndexSourceRegistry,
    exact: IndexDriftDiagnosisOperatorRuntime,
}

impl IndexDriftSourcePageDiagnosisRuntime {
    fn new(
        sources: rustok_index::SharedIndexSourceRegistry,
        exact: IndexDriftDiagnosisOperatorRuntime,
    ) -> Self {
        Self { sources, exact }
    }

    pub async fn diagnose_source_page(
        &self,
        context: IndexReconciliationOperatorContext,
        schema: rustok_index::SchemaRef,
        cursor: Option<rustok_index::IndexSourceCursor>,
        limit: usize,
    ) -> std::result::Result<
        IndexDriftSourcePageDiagnosisOutcome,
        IndexDriftSourcePageDiagnosisError,
    > {
        let request = authorize_and_build_scan_request(context, schema, cursor, limit)?;
        let page = self.sources.scan(request).await?;
        diagnose_page_with(page, |position, key| {
            let exact = self.exact.clone();
            async move {
                exact
                    .diagnose_entity(context, key)
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
            .finish_non_exhaustive()
    }
}

fn authorize_and_build_scan_request(
    context: IndexReconciliationOperatorContext,
    schema: rustok_index::SchemaRef,
    cursor: Option<rustok_index::IndexSourceCursor>,
    limit: usize,
) -> std::result::Result<rustok_index::IndexSourceScanRequest, IndexDriftSourcePageDiagnosisError> {
    let permissions = permissions_for(&context.tenant_id(), &context.actor_id())
        .ok_or(IndexDriftSourcePageDiagnosisError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexDriftSourcePageDiagnosisError::Forbidden);
    }
    if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit) {
        return Err(IndexDriftSourcePageDiagnosisError::InvalidPageLimit {
            actual: limit,
            max: MAX_SOURCE_PAGE_DIAGNOSIS_SIZE,
        });
    }
    rustok_index::IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)
        .map_err(Into::into)
}

async fn diagnose_page_with<Diagnose, DiagnoseFuture>(
    page: rustok_index::IndexSourcePage,
    mut diagnose: Diagnose,
) -> std::result::Result<
    IndexDriftSourcePageDiagnosisOutcome,
    IndexDriftSourcePageDiagnosisError,
>
where
    Diagnose: FnMut(usize, rustok_index::EntityKey) -> DiagnoseFuture,
    DiagnoseFuture: Future<
        Output = std::result::Result<
            rustok_index::IndexDriftDigestOutcome,
            IndexDriftSourcePageDiagnosisError,
        >,
    >,
{
    let (mutations, next_cursor) = page.into_parts();
    let scanned_mutation_count = mutations.len();
    let mut candidate_count = 0;
    let mut skipped_delete_count = 0;
    let mut consistent_count = 0;
    let mut mismatch_recorded_count = 0;
    let mut receipts = Vec::new();

    for (position, mutation) in mutations.into_iter().enumerate() {
        if matches!(&mutation, rustok_index::IndexMutation::Delete { .. }) {
            skipped_delete_count += 1;
            continue;
        }

        candidate_count += 1;
        match diagnose(position, mutation.key().clone()).await? {
            rustok_index::IndexDriftDigestOutcome::Consistent { .. } => {
                consistent_count += 1;
            }
            rustok_index::IndexDriftDigestOutcome::MismatchRecorded { receipt, .. } => {
                mismatch_recorded_count += 1;
                receipts.push(receipt);
            }
        }
    }

    Ok(IndexDriftSourcePageDiagnosisOutcome {
        next_cursor,
        scanned_mutation_count,
        candidate_count,
        skipped_delete_count,
        consistent_count,
        mismatch_recorded_count,
        receipts,
    })
}

pub(super) fn materialize_index_drift_source_page_diagnosis(
    extensions: &mut ModuleRuntimeExtensions,
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

    extensions.insert(IndexDriftSourcePageDiagnosisRuntime::new(sources, exact));
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
        EntityKey, EntityName, IndexDriftDigestOutcome, IndexMutation, IndexRecord,
        IndexSourceCursor, IndexSourcePage, IndexSourceScanRequest, LocaleKey, ModuleName,
        SchemaRef, SchemaVersion,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        IndexDriftSourcePageDiagnosisError, authorize_and_build_scan_request, diagnose_page_with,
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
    async fn one_page_skips_deletes_and_diagnoses_each_upsert_once() {
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
                Ok(IndexDriftDigestOutcome::Consistent {
                    digest: "a".repeat(64),
                })
            }
        })
        .await
        .expect("bounded page should diagnose");

        assert_eq!(diagnosed.lock().unwrap().as_slice(), &[first]);
        assert_eq!(outcome.scanned_mutation_count(), 2);
        assert_eq!(outcome.candidate_count(), 1);
        assert_eq!(outcome.skipped_delete_count(), 1);
        assert_eq!(outcome.consistent_count(), 1);
        assert_eq!(outcome.mismatch_recorded_count(), 0);
        assert_eq!(outcome.next_cursor(), Some(&next_cursor));
        assert!(!outcome.is_complete());
        assert!(outcome.receipts().is_empty());
    }
}
