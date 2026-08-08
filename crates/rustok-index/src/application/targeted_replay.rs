use std::{collections::BTreeSet, fmt, sync::Arc};

use thiserror::Error;
use uuid::Uuid;

use crate::{LocaleMode, RecordValidationError, SchemaRef, SchemaRegistry};

use super::{
    IndexReplayFailure, IndexReplayMode, IndexReplayModeSelection, IndexReplayMutationOutcome,
    IndexReplayMutationSink, IndexSourceError, SharedIndexSourceRegistry,
};

/// Aggregate result of one bounded Targeted replay invocation.
///
/// Targeted replay owns no job, checkpoint, lease, retry state, scheduler or cancellation model.
/// Missing requested keys are reported as a count and are not synthesized into delete mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayTargetedOutcome {
    source_name: String,
    requested_count: usize,
    mutation_count: usize,
    missing_count: usize,
    applied_count: usize,
    duplicate_count: usize,
    stale_count: usize,
}

impl IndexReplayTargetedOutcome {
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn requested_count(&self) -> usize {
        self.requested_count
    }

    pub fn mutation_count(&self) -> usize {
        self.mutation_count
    }

    pub fn missing_count(&self) -> usize {
        self.missing_count
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

/// Bounded exact-key mutation application for `IndexReplayMode::Targeted`.
///
/// The executor consumes the canonical `IndexSourceLoadRequest` already owned by
/// `IndexReplayModeSelection::Targeted`, validates requested key shape against the active schema,
/// validates the complete returned batch before the first mutation write, then applies source-owned
/// stable mutation identities sequentially through the existing replay mutation sink. It
/// deliberately has no scan cursor or durable replay ownership.
pub struct IndexReplayTargetedExecutor<M> {
    sources: SharedIndexSourceRegistry,
    schemas: Arc<SchemaRegistry>,
    mutation_sink: M,
}

impl<M> IndexReplayTargetedExecutor<M>
where
    M: IndexReplayMutationSink,
{
    pub fn new(
        sources: SharedIndexSourceRegistry,
        schemas: Arc<SchemaRegistry>,
        mutation_sink: M,
    ) -> Self {
        Self {
            sources,
            schemas,
            mutation_sink,
        }
    }

    pub async fn run(
        &self,
        selection: IndexReplayModeSelection,
    ) -> Result<IndexReplayTargetedOutcome, IndexReplayTargetedError> {
        let request = match selection {
            IndexReplayModeSelection::Targeted(request) => request,
            other => {
                return Err(IndexReplayTargetedError::WrongMode {
                    actual: other.mode(),
                });
            }
        };

        let registered = self
            .schemas
            .get(request.schema())
            .ok_or_else(|| IndexReplayTargetedError::SchemaNotRegistered(request.schema().clone()))?;
        for (position, key) in request.keys().iter().enumerate() {
            let source = if key.entity_id.is_nil() {
                Some(RecordValidationError::NilEntityId)
            } else {
                match (registered.schema.locale_mode, key.locale.is_some()) {
                    (LocaleMode::Required, false) => Some(RecordValidationError::LocaleRequired(
                        request.schema().clone(),
                    )),
                    (LocaleMode::None, true) => Some(RecordValidationError::LocaleForbidden(
                        request.schema().clone(),
                    )),
                    _ => None,
                }
            };
            if let Some(source) = source {
                return Err(IndexReplayTargetedError::InvalidTarget { position, source });
            }
        }

        let descriptor = self
            .sources
            .source_for_schema(request.schema())
            .ok_or_else(|| IndexReplayTargetedError::UnknownSchemaSource(request.schema().clone()))?;
        let source_name = descriptor.source_name().to_owned();
        let requested_count = request.keys().len();
        let batch = self
            .sources
            .load(request)
            .await
            .map_err(IndexReplayTargetedError::Source)?;
        let mutations = batch.into_mutations();

        // Fail the entire invocation before its first persistence call when the source batch cannot
        // provide stable, schema-valid replay deliveries. This mirrors the durable page preflight
        // without importing a checkpoint or job state machine into Targeted execution.
        let mut event_ids = BTreeSet::<Uuid>::new();
        for (position, mutation) in mutations.iter().enumerate() {
            let event_id = mutation.event_id();
            if event_id.is_nil() {
                return Err(IndexReplayTargetedError::NilEventId { position });
            }
            if !event_ids.insert(event_id) {
                return Err(IndexReplayTargetedError::DuplicateEventId { position, event_id });
            }
            self.schemas.validate_mutation(mutation).map_err(|source| {
                IndexReplayTargetedError::InvalidMutation { position, source }
            })?;
        }

        let mutation_count = mutations.len();
        let mut applied_count = 0usize;
        let mut duplicate_count = 0usize;
        let mut stale_count = 0usize;
        for (position, mutation) in mutations.iter().enumerate() {
            match self
                .mutation_sink
                .apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)
                .await
                .map_err(|failure| IndexReplayTargetedError::Mutation { position, failure })?
            {
                IndexReplayMutationOutcome::Applied => applied_count += 1,
                IndexReplayMutationOutcome::Duplicate => duplicate_count += 1,
                IndexReplayMutationOutcome::StaleIgnored => stale_count += 1,
            }
        }

        Ok(IndexReplayTargetedOutcome {
            source_name,
            requested_count,
            mutation_count,
            missing_count: requested_count - mutation_count,
            applied_count,
            duplicate_count,
            stale_count,
        })
    }
}

impl<M> fmt::Debug for IndexReplayTargetedExecutor<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexReplayTargetedExecutor")
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error)]
pub enum IndexReplayTargetedError {
    #[error("Index replay Targeted executor cannot run {actual:?} mode")]
    WrongMode { actual: IndexReplayMode },
    #[error("No Index replay source owns Targeted schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index replay Targeted schema is not registered in the active runtime: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("Index replay Targeted key at position {position} is invalid")]
    InvalidTarget {
        position: usize,
        #[source]
        source: RecordValidationError,
    },
    #[error("Index replay Targeted source load failed")]
    Source(#[source] IndexSourceError),
    #[error("Index replay Targeted mutation at position {position} has a nil event id")]
    NilEventId { position: usize },
    #[error(
        "Index replay Targeted mutation at position {position} duplicates event id {event_id}"
    )]
    DuplicateEventId { position: usize, event_id: Uuid },
    #[error("Index replay Targeted mutation at position {position} violates the registered schema")]
    InvalidMutation {
        position: usize,
        #[source]
        source: RecordValidationError,
    },
    #[error("Index replay Targeted mutation at position {position} failed")]
    Mutation {
        position: usize,
        #[source]
        failure: IndexReplayFailure,
    },
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Mutex,
    };

    use async_trait::async_trait;
    use rustok_core::ModuleRuntimeExtensions;

    use crate::{
        EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation, IndexRecord,
        IndexSchema, IndexSource, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
        IndexSourcePage, IndexSourceScanRequest, IndexValue, IndexValueType, LocaleKey, LocaleMode,
        ModuleName, SchemaVersion, materialize_index_schema_registry,
        materialize_index_source_registry, register_index_schema_source, register_index_source,
    };

    use super::*;

    #[derive(Clone, Copy)]
    enum SourceMode {
        Valid,
        MissingSecond,
        NilEvent,
        DuplicateEvent,
        InvalidRecord,
    }

    struct TargetedSource {
        mode: SourceMode,
    }

    #[async_trait]
    impl IndexSource for TargetedSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None).unwrap())
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            let mut mutations = Vec::new();
            for (position, key) in request.keys().iter().enumerate() {
                if matches!(self.mode, SourceMode::MissingSecond) && position == 1 {
                    continue;
                }
                let event_id = match self.mode {
                    SourceMode::NilEvent if position == 0 => Uuid::nil(),
                    SourceMode::DuplicateEvent => Uuid::from_u128(900),
                    _ => Uuid::from_u128(900 + position as u128),
                };
                let fields = if matches!(self.mode, SourceMode::InvalidRecord) && position == 0 {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(
                        FieldName::new("id").unwrap(),
                        IndexValue::Uuid(key.entity_id),
                    )])
                };
                mutations.push(IndexMutation::Upsert {
                    event_id,
                    record: IndexRecord {
                        key: key.clone(),
                        source_version: position as u64 + 1,
                        fields,
                        links: Vec::new(),
                    },
                });
            }
            Ok(IndexSourceLoadBatch::new(&request, mutations).unwrap())
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        event_ids: Mutex<Vec<Uuid>>,
    }

    #[async_trait]
    impl IndexReplayMutationSink for RecordingSink {
        async fn apply_replay_mutation(
            &self,
            _registry: &SchemaRegistry,
            _source_name: &str,
            mutation: &IndexMutation,
        ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
            self.event_ids.lock().unwrap().push(mutation.event_id());
            Ok(IndexReplayMutationOutcome::Applied)
        }
    }

    struct RetryOnceSink {
        applied: Mutex<BTreeSet<Uuid>>,
        fail_event: Uuid,
        fail_once: Mutex<bool>,
    }

    impl RetryOnceSink {
        fn new(fail_event: Uuid) -> Self {
            Self {
                applied: Mutex::new(BTreeSet::new()),
                fail_event,
                fail_once: Mutex::new(true),
            }
        }
    }

    #[async_trait]
    impl IndexReplayMutationSink for RetryOnceSink {
        async fn apply_replay_mutation(
            &self,
            _registry: &SchemaRegistry,
            _source_name: &str,
            mutation: &IndexMutation,
        ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
            let event_id = mutation.event_id();
            let mut fail_once = self.fail_once.lock().unwrap();
            if event_id == self.fail_event && *fail_once {
                *fail_once = false;
                return Err(IndexReplayFailure::retryable("targeted_retryable_failure").unwrap());
            }
            drop(fail_once);

            let inserted = self.applied.lock().unwrap().insert(event_id);
            Ok(if inserted {
                IndexReplayMutationOutcome::Applied
            } else {
                IndexReplayMutationOutcome::Duplicate
            })
        }
    }

    fn schema(locale_mode: LocaleMode) -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("targeted-owner").unwrap(),
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

    fn key(entity_id: u128) -> EntityKey {
        EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: schema(LocaleMode::Optional).reference,
            entity_id: Uuid::from_u128(entity_id),
            locale: None,
        }
    }

    fn localized_key(entity_id: u128, locale: &str) -> EntityKey {
        EntityKey {
            locale: Some(LocaleKey::new(locale).unwrap()),
            ..key(entity_id)
        }
    }

    fn executor(mode: SourceMode) -> IndexReplayTargetedExecutor<RecordingSink> {
        executor_with_locale_mode(mode, LocaleMode::Optional)
    }

    fn executor_with_locale_mode(
        mode: SourceMode,
        locale_mode: LocaleMode,
    ) -> IndexReplayTargetedExecutor<RecordingSink> {
        let mut extensions = ModuleRuntimeExtensions::default();
        let schema = schema(locale_mode);
        register_index_schema_source(&mut extensions, "targeted_owner", schema.clone()).unwrap();
        register_index_source(
            &mut extensions,
            "targeted_owner",
            "targeted-owner-primary",
            [schema.reference.clone()],
            TargetedSource { mode },
        )
        .unwrap();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("schema registry");
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .expect("source registry");
        IndexReplayTargetedExecutor::new(sources, schemas.shared(), RecordingSink::default())
    }

    #[tokio::test]
    async fn targeted_load_applies_only_returned_mutations_and_reports_missing_keys() {
        let executor = executor(SourceMode::MissingSecond);
        let outcome = executor
            .run(IndexReplayModeSelection::targeted(vec![key(10), key(11)]).unwrap())
            .await
            .unwrap();

        assert_eq!(outcome.source_name(), "targeted-owner-primary");
        assert_eq!(outcome.requested_count(), 2);
        assert_eq!(outcome.mutation_count(), 1);
        assert_eq!(outcome.missing_count(), 1);
        assert_eq!(outcome.applied_count(), 1);
        assert_eq!(outcome.duplicate_count(), 0);
        assert_eq!(outcome.stale_count(), 0);
    }

    #[tokio::test]
    async fn targeted_rejects_other_modes_without_source_or_mutation_execution() {
        let executor = executor(SourceMode::Valid);
        let error = executor
            .run(IndexReplayModeSelection::full())
            .await
            .expect_err("Full must not alias Targeted execution");
        assert!(matches!(
            error,
            IndexReplayTargetedError::WrongMode {
                actual: IndexReplayMode::Full
            }
        ));
    }

    #[tokio::test]
    async fn targeted_rejects_invalid_requested_entity_and_locale_scope_before_load() {
        let none_executor = executor_with_locale_mode(SourceMode::Valid, LocaleMode::None);
        let forbidden_locale = none_executor
            .run(
                IndexReplayModeSelection::targeted(vec![localized_key(10, "en-US")]).unwrap(),
            )
            .await
            .expect_err("non-localized schema must reject locale Targeted key");
        assert!(matches!(
            forbidden_locale,
            IndexReplayTargetedError::InvalidTarget {
                position: 0,
                source: RecordValidationError::LocaleForbidden(_),
            }
        ));
        assert!(none_executor.mutation_sink.event_ids.lock().unwrap().is_empty());

        let required_executor = executor_with_locale_mode(SourceMode::Valid, LocaleMode::Required);
        let missing_locale = required_executor
            .run(IndexReplayModeSelection::targeted(vec![key(10)]).unwrap())
            .await
            .expect_err("localized schema must require locale Targeted key");
        assert!(matches!(
            missing_locale,
            IndexReplayTargetedError::InvalidTarget {
                position: 0,
                source: RecordValidationError::LocaleRequired(_),
            }
        ));
        assert!(required_executor.mutation_sink.event_ids.lock().unwrap().is_empty());

        let optional_executor = executor(SourceMode::Valid);
        let mut nil_key = key(10);
        nil_key.entity_id = Uuid::nil();
        let nil_entity = optional_executor
            .run(IndexReplayModeSelection::targeted(vec![nil_key]).unwrap())
            .await
            .expect_err("Targeted entity id must be non-nil before load");
        assert!(matches!(
            nil_entity,
            IndexReplayTargetedError::InvalidTarget {
                position: 0,
                source: RecordValidationError::NilEntityId,
            }
        ));
        assert!(optional_executor.mutation_sink.event_ids.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn targeted_preflights_nil_duplicate_and_schema_invalid_batches_before_writes() {
        for (mode, expected) in [
            (SourceMode::NilEvent, "nil"),
            (SourceMode::DuplicateEvent, "duplicate"),
            (SourceMode::InvalidRecord, "schema"),
        ] {
            let executor = executor(mode);
            let error = executor
                .run(IndexReplayModeSelection::targeted(vec![key(10), key(11)]).unwrap())
                .await
                .expect_err("invalid Targeted batch must fail closed");
            match expected {
                "nil" => assert!(matches!(error, IndexReplayTargetedError::NilEventId { .. })),
                "duplicate" => assert!(matches!(
                    error,
                    IndexReplayTargetedError::DuplicateEventId { .. }
                )),
                "schema" => assert!(matches!(
                    error,
                    IndexReplayTargetedError::InvalidMutation { .. }
                )),
                _ => unreachable!(),
            }
            assert!(executor.mutation_sink.event_ids.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn targeted_exact_retry_replays_stable_event_ids_after_partial_failure() {
        let template = executor(SourceMode::Valid);
        let executor = IndexReplayTargetedExecutor::new(
            template.sources.clone(),
            template.schemas.clone(),
            RetryOnceSink::new(Uuid::from_u128(901)),
        );
        let selection = || IndexReplayModeSelection::targeted(vec![key(10), key(11)]).unwrap();

        let error = executor
            .run(selection())
            .await
            .expect_err("second mutation should fail after first mutation is applied");
        assert!(matches!(
            error,
            IndexReplayTargetedError::Mutation { position: 1, .. }
        ));
        assert_eq!(
            executor
                .mutation_sink
                .applied
                .lock()
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![Uuid::from_u128(900)]
        );

        let retry = executor
            .run(selection())
            .await
            .expect("exact retry should converge through stable mutation identities");
        assert_eq!(retry.mutation_count(), 2);
        assert_eq!(retry.applied_count(), 1);
        assert_eq!(retry.duplicate_count(), 1);
        assert_eq!(retry.stale_count(), 0);
        assert_eq!(executor.mutation_sink.applied.lock().unwrap().len(), 2);
    }
}
