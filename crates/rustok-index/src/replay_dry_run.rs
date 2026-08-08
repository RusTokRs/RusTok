use std::{collections::BTreeSet, fmt, sync::Arc};

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    IndexMutation, IndexSourceCursor, IndexSourceError, IndexSourceScanRequest, LocaleKey, LocaleMode,
    RecordValidationError, SchemaRef, SchemaRegistry, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry,
};

const MAX_DRY_RUN_PAGES: usize = 1_024;

/// One bounded, side-effect-free replay validation request.
///
/// The request carries one explicit schema-wide or exact-locale source scope plus an optional
/// source-owned continuation cursor so a large source can be inspected over multiple operator
/// calls without creating a replay job or durable checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayDryRunRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
    cursor: Option<IndexSourceCursor>,
    page_limit: usize,
    max_pages: usize,
}

impl IndexReplayDryRunRequest {
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        cursor: Option<IndexSourceCursor>,
        page_limit: usize,
        max_pages: usize,
    ) -> Result<Self, IndexReplayDryRunError> {
        Self::new_scoped(tenant_id, schema, None, cursor, page_limit, max_pages)
    }

    pub fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        cursor: Option<IndexSourceCursor>,
        page_limit: usize,
        max_pages: usize,
    ) -> Result<Self, IndexReplayDryRunError> {
        Self::new_scoped(
            tenant_id,
            schema,
            Some(locale),
            cursor,
            page_limit,
            max_pages,
        )
    }

    fn new_scoped(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
        cursor: Option<IndexSourceCursor>,
        page_limit: usize,
        max_pages: usize,
    ) -> Result<Self, IndexReplayDryRunError> {
        match locale.as_ref() {
            Some(locale) => IndexSourceScanRequest::for_locale(
                tenant_id,
                schema.clone(),
                locale.clone(),
                cursor.clone(),
                page_limit,
            ),
            None => IndexSourceScanRequest::new(
                tenant_id,
                schema.clone(),
                cursor.clone(),
                page_limit,
            ),
        }
        .map_err(IndexReplayDryRunError::InvalidRequest)?;
        if !(1..=MAX_DRY_RUN_PAGES).contains(&max_pages) {
            return Err(IndexReplayDryRunError::InvalidMaxPages {
                actual: max_pages,
                max: MAX_DRY_RUN_PAGES,
            });
        }
        Ok(Self {
            tenant_id,
            schema,
            locale,
            cursor,
            page_limit,
            max_pages,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }

    pub fn cursor(&self) -> Option<&IndexSourceCursor> {
        self.cursor.as_ref()
    }

    pub fn page_limit(&self) -> usize {
        self.page_limit
    }

    pub fn max_pages(&self) -> usize {
        self.max_pages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayDryRunStatus {
    Complete,
    Yielded,
}

/// Aggregate result of one bounded dry-run invocation.
///
/// No mutation, inbox delivery, job, checkpoint, or reconciliation progress is persisted by this
/// capability. `next_cursor` is present only when the page budget was exhausted before completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayDryRunOutcome {
    status: IndexReplayDryRunStatus,
    source_name: String,
    next_cursor: Option<IndexSourceCursor>,
    pages_scanned: usize,
    mutation_count: usize,
    upsert_count: usize,
    delete_count: usize,
    max_source_version: Option<u64>,
}

impl IndexReplayDryRunOutcome {
    pub fn status(&self) -> IndexReplayDryRunStatus {
        self.status
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn next_cursor(&self) -> Option<&IndexSourceCursor> {
        self.next_cursor.as_ref()
    }

    pub fn pages_scanned(&self) -> usize {
        self.pages_scanned
    }

    pub fn mutation_count(&self) -> usize {
        self.mutation_count
    }

    pub fn upsert_count(&self) -> usize {
        self.upsert_count
    }

    pub fn delete_count(&self) -> usize {
        self.delete_count
    }

    pub fn max_source_version(&self) -> Option<u64> {
        self.max_source_version
    }
}

/// Immutable, cloneable dry-run capability materialized from the complete source/schema set.
#[derive(Clone)]
pub struct SharedIndexReplayDryRunRuntime {
    sources: SharedIndexSourceRegistry,
    schemas: Arc<SchemaRegistry>,
}

impl SharedIndexReplayDryRunRuntime {
    fn new(sources: SharedIndexSourceRegistry, schemas: Arc<SchemaRegistry>) -> Self {
        Self { sources, schemas }
    }

    pub async fn run(
        &self,
        request: IndexReplayDryRunRequest,
    ) -> Result<IndexReplayDryRunOutcome, IndexReplayDryRunError> {
        let descriptor = self
            .sources
            .source_for_schema(request.schema())
            .ok_or_else(|| IndexReplayDryRunError::UnknownSchemaSource(request.schema.clone()))?;
        let registered = self
            .schemas
            .get(request.schema())
            .ok_or_else(|| IndexReplayDryRunError::SchemaNotRegistered(request.schema.clone()))?;
        if request.locale().is_some() && registered.schema.locale_mode == LocaleMode::None {
            return Err(IndexReplayDryRunError::LocaleScopeUnsupported(
                request.schema.clone(),
            ));
        }

        let source_name = descriptor.source_name().to_owned();
        let mut cursor = request.cursor.clone();
        let mut pages_scanned = 0usize;
        let mut mutation_count = 0usize;
        let mut upsert_count = 0usize;
        let mut delete_count = 0usize;
        let mut max_source_version = None::<u64>;

        for page_index in 0..request.max_pages {
            let scan_request = match request.locale() {
                Some(locale) => IndexSourceScanRequest::for_locale(
                    request.tenant_id,
                    request.schema.clone(),
                    locale.clone(),
                    cursor.clone(),
                    request.page_limit,
                ),
                None => IndexSourceScanRequest::new(
                    request.tenant_id,
                    request.schema.clone(),
                    cursor.clone(),
                    request.page_limit,
                ),
            }
            .map_err(IndexReplayDryRunError::InvalidRequest)?;
            let page = self
                .sources
                .scan(scan_request)
                .await
                .map_err(IndexReplayDryRunError::Source)?;

            let mut event_ids = BTreeSet::new();
            for (position, mutation) in page.mutations().iter().enumerate() {
                let event_id = mutation.event_id();
                if event_id.is_nil() {
                    return Err(IndexReplayDryRunError::NilEventId {
                        page_index,
                        position,
                    });
                }
                if !event_ids.insert(event_id) {
                    return Err(IndexReplayDryRunError::DuplicateEventId {
                        page_index,
                        position,
                        event_id,
                    });
                }
                self.schemas.validate_mutation(mutation).map_err(|source| {
                    IndexReplayDryRunError::InvalidMutation {
                        page_index,
                        position,
                        source,
                    }
                })?;
                match mutation {
                    IndexMutation::Upsert { .. } => upsert_count += 1,
                    IndexMutation::Delete { .. } => delete_count += 1,
                }
                max_source_version = Some(
                    max_source_version
                        .map_or(mutation.source_version(), |current| {
                            current.max(mutation.source_version())
                        }),
                );
            }

            pages_scanned += 1;
            mutation_count += page.mutations().len();
            cursor = page.next_cursor().cloned();
            if cursor.is_none() {
                return Ok(IndexReplayDryRunOutcome {
                    status: IndexReplayDryRunStatus::Complete,
                    source_name,
                    next_cursor: None,
                    pages_scanned,
                    mutation_count,
                    upsert_count,
                    delete_count,
                    max_source_version,
                });
            }
        }

        Ok(IndexReplayDryRunOutcome {
            status: IndexReplayDryRunStatus::Yielded,
            source_name,
            next_cursor: cursor,
            pages_scanned,
            mutation_count,
            upsert_count,
            delete_count,
            max_source_version,
        })
    }
}

impl fmt::Debug for SharedIndexReplayDryRunRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexReplayDryRunRuntime")
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error)]
pub enum IndexReplayDryRunError {
    #[error("Index replay dry-run request is invalid")]
    InvalidRequest(#[source] IndexSourceError),
    #[error("Index replay dry-run max pages is invalid: actual={actual}, max={max}")]
    InvalidMaxPages { actual: usize, max: usize },
    #[error("No Index replay source owns dry-run schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index replay dry-run schema is not registered in the active runtime: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("Index replay dry-run schema does not support locale-scoped pages: {0}")]
    LocaleScopeUnsupported(SchemaRef),
    #[error("Index replay dry-run source failed")]
    Source(#[source] IndexSourceError),
    #[error(
        "Index replay dry-run mutation at page {page_index} position {position} has a nil event id"
    )]
    NilEventId {
        page_index: usize,
        position: usize,
    },
    #[error(
        "Index replay dry-run mutation at page {page_index} position {position} duplicates event id {event_id}"
    )]
    DuplicateEventId {
        page_index: usize,
        position: usize,
        event_id: Uuid,
    },
    #[error(
        "Index replay dry-run mutation at page {page_index} position {position} violates the registered schema"
    )]
    InvalidMutation {
        page_index: usize,
        position: usize,
        #[source]
        source: RecordValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexReplayDryRunRuntimeCompositionError {
    #[error("shared Index replay dry-run runtime is already materialized")]
    AlreadyMaterialized,
    #[error("shared Index source registry exists without the shared schema registry")]
    MissingSchemaRegistry,
}

/// Publishes one immutable dry-run runtime only after source factories and both shared registries
/// have been materialized. This function performs no source call and no database I/O.
pub fn materialize_index_replay_dry_run_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexReplayDryRunRuntime>, IndexReplayDryRunRuntimeCompositionError> {
    if extensions.contains::<SharedIndexReplayDryRunRuntime>() {
        return Err(IndexReplayDryRunRuntimeCompositionError::AlreadyMaterialized);
    }
    let Some(sources) = extensions.get::<SharedIndexSourceRegistry>().cloned() else {
        return Ok(None);
    };
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or(IndexReplayDryRunRuntimeCompositionError::MissingSchemaRegistry)?;
    let runtime = SharedIndexReplayDryRunRuntime::new(sources, schemas.shared());
    extensions.insert(runtime.clone());
    Ok(Some(runtime))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;

    use crate::{
        EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexRecord, IndexSchema,
        IndexSource, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
        IndexSourcePage, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaVersion,
        materialize_index_schema_registry, materialize_index_source_registry,
        register_index_schema_source, register_index_source,
    };

    use super::*;

    struct PagedSource {
        page_count: usize,
        invalid_record: bool,
    }

    #[async_trait]
    impl IndexSource for PagedSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            let page_index = request
                .cursor()
                .map(|cursor| cursor.value().as_u64().expect("integer cursor") as usize)
                .unwrap_or(0);
            let entity_id = Uuid::from_u128(100 + page_index as u128);
            let fields = if self.invalid_record {
                BTreeMap::new()
            } else {
                BTreeMap::from([(
                    FieldName::new("id").unwrap(),
                    IndexValue::Uuid(entity_id),
                )])
            };
            let mutation = IndexMutation::Upsert {
                event_id: Uuid::from_u128(1_000 + page_index as u128),
                record: IndexRecord {
                    key: EntityKey {
                        tenant_id: request.tenant_id(),
                        schema: request.schema().clone(),
                        entity_id,
                        locale: request.locale().cloned(),
                    },
                    source_version: page_index as u64 + 1,
                    fields,
                    links: Vec::new(),
                },
            };
            let next_cursor = if page_index + 1 < self.page_count {
                Some(IndexSourceCursor::new(serde_json::json!(page_index + 1)).unwrap())
            } else {
                None
            };
            Ok(IndexSourcePage::new(&request, vec![mutation], next_cursor).unwrap())
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new()).unwrap())
        }
    }

    fn schema(locale_mode: LocaleMode) -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("dry-run-owner").unwrap(),
                entity: EntityName::new("item").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode,
            fields: vec![IndexField {
                name: FieldName::new("id").unwrap(),
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        }
    }

    fn runtime(
        page_count: usize,
        invalid_record: bool,
        locale_mode: LocaleMode,
    ) -> SharedIndexReplayDryRunRuntime {
        let mut extensions = ModuleRuntimeExtensions::default();
        let schema = schema(locale_mode);
        register_index_schema_source(&mut extensions, "dry_run_owner", schema.clone()).unwrap();
        register_index_source(
            &mut extensions,
            "dry_run_owner",
            "dry-run-primary",
            [schema.reference.clone()],
            PagedSource {
                page_count,
                invalid_record,
            },
        )
        .unwrap();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .expect("source registry");
        extensions.insert(schemas);
        extensions.insert(sources);
        materialize_index_replay_dry_run_runtime(&mut extensions)
            .unwrap()
            .expect("dry-run runtime")
    }

    #[tokio::test]
    async fn bounded_dry_run_yields_a_resume_cursor_and_completes_without_state() {
        let runtime = runtime(3, false, LocaleMode::Optional);
        let first = runtime
            .run(
                IndexReplayDryRunRequest::new(
                    Uuid::from_u128(1),
                    schema(LocaleMode::Optional).reference,
                    None,
                    10,
                    1,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), IndexReplayDryRunStatus::Yielded);
        assert_eq!(first.pages_scanned(), 1);
        assert_eq!(first.mutation_count(), 1);
        assert_eq!(first.upsert_count(), 1);
        assert_eq!(first.delete_count(), 0);
        assert_eq!(first.max_source_version(), Some(1));

        let second = runtime
            .run(
                IndexReplayDryRunRequest::new(
                    Uuid::from_u128(1),
                    schema(LocaleMode::Optional).reference,
                    first.next_cursor().cloned(),
                    10,
                    2,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), IndexReplayDryRunStatus::Complete);
        assert!(second.next_cursor().is_none());
        assert_eq!(second.pages_scanned(), 2);
        assert_eq!(second.mutation_count(), 2);
        assert_eq!(second.max_source_version(), Some(3));
    }

    #[tokio::test]
    async fn exact_locale_dry_run_uses_the_same_canonical_scope_for_every_page() {
        let runtime = runtime(2, false, LocaleMode::Optional);
        let locale = LocaleKey::new("EN-us").unwrap();
        let first = runtime
            .run(
                IndexReplayDryRunRequest::for_locale(
                    Uuid::from_u128(1),
                    schema(LocaleMode::Optional).reference,
                    locale.clone(),
                    None,
                    10,
                    1,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(locale.as_str(), "en-US");
        assert_eq!(first.status(), IndexReplayDryRunStatus::Yielded);

        let second = runtime
            .run(
                IndexReplayDryRunRequest::for_locale(
                    Uuid::from_u128(1),
                    schema(LocaleMode::Optional).reference,
                    locale,
                    first.next_cursor().cloned(),
                    10,
                    1,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), IndexReplayDryRunStatus::Complete);
        assert!(second.next_cursor().is_none());
    }

    #[tokio::test]
    async fn exact_locale_dry_run_rejects_a_non_localized_schema_before_source_scan() {
        let error = runtime(1, false, LocaleMode::None)
            .run(
                IndexReplayDryRunRequest::for_locale(
                    Uuid::from_u128(1),
                    schema(LocaleMode::None).reference,
                    LocaleKey::new("en-US").unwrap(),
                    None,
                    10,
                    1,
                )
                .unwrap(),
            )
            .await
            .expect_err("non-localized schema must reject exact-locale dry-run");
        assert!(matches!(
            error,
            IndexReplayDryRunError::LocaleScopeUnsupported(_)
        ));
    }

    #[tokio::test]
    async fn dry_run_rejects_a_schema_invalid_mutation_before_any_persistence_boundary() {
        let error = runtime(1, true, LocaleMode::Optional)
            .run(
                IndexReplayDryRunRequest::new(
                    Uuid::from_u128(1),
                    schema(LocaleMode::Optional).reference,
                    None,
                    10,
                    1,
                )
                .unwrap(),
            )
            .await
            .expect_err("missing required field must fail dry-run validation");
        assert!(matches!(
            error,
            IndexReplayDryRunError::InvalidMutation { .. }
        ));
    }

    #[test]
    fn dry_run_request_bounds_page_budget() {
        let tenant_id = Uuid::from_u128(1);
        assert!(IndexReplayDryRunRequest::new(
            tenant_id,
            schema(LocaleMode::Optional).reference,
            None,
            10,
            0,
        )
        .is_err());
        assert!(IndexReplayDryRunRequest::new(
            tenant_id,
            schema(LocaleMode::Optional).reference,
            None,
            10,
            MAX_DRY_RUN_PAGES + 1,
        )
        .is_err());
    }
}
