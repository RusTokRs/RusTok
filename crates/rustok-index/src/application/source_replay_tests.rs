use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexRecord, IndexSchema,
    IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

use super::*;

struct ReplaySource {
    calls: Arc<AtomicUsize>,
    mutation: IndexMutation,
}

#[async_trait]
impl IndexSource for ReplaySource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(IndexSourcePage::new(&request, vec![self.mutation.clone()], None)
            .expect("bounded replay page"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

#[derive(Clone)]
struct RecordingMutationSink {
    calls: Arc<AtomicUsize>,
    order: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl IndexReplayMutationSink for RecordingMutationSink {
    async fn apply_replay_mutation(
        &self,
        _registry: &SchemaRegistry,
        _source_name: &str,
        _mutation: &IndexMutation,
    ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
        self.order.lock().unwrap().push("mutation");
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(if call == 0 {
            IndexReplayMutationOutcome::Applied
        } else {
            IndexReplayMutationOutcome::Duplicate
        })
    }
}

#[derive(Clone)]
struct RecordingCheckpointStore {
    checkpoint: Arc<Mutex<Option<IndexReplayCheckpoint>>>,
    fail_next_commit: Arc<AtomicBool>,
    order: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl IndexReplayCheckpointStore for RecordingCheckpointStore {
    async fn load_replay_checkpoint(
        &self,
        _key: &IndexReplayCheckpointKey,
    ) -> Result<Option<IndexReplayCheckpoint>, IndexReplayFailure> {
        Ok(self.checkpoint.lock().unwrap().clone())
    }

    async fn commit_replay_checkpoint(
        &self,
        checkpoint: &IndexReplayCheckpoint,
    ) -> Result<(), IndexReplayFailure> {
        self.order.lock().unwrap().push("checkpoint");
        if self.fail_next_commit.swap(false, Ordering::SeqCst) {
            return Err(IndexReplayFailure::retryable("checkpoint_unavailable").unwrap());
        }
        *self.checkpoint.lock().unwrap() = Some(checkpoint.clone());
        Ok(())
    }
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new("product").unwrap(),
        version: SchemaVersion::new(1),
    }
}

fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::None,
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

fn mutation(tenant_id: Uuid, entity_id: Uuid) -> IndexMutation {
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(10),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version: 7,
            fields: BTreeMap::from([(
                FieldName::new("id").unwrap(),
                IndexValue::Uuid(entity_id),
            )]),
            links: Vec::new(),
        },
    }
}

fn worker(
    tenant_id: Uuid,
    source_calls: Arc<AtomicUsize>,
    mutation_calls: Arc<AtomicUsize>,
    checkpoint: Arc<Mutex<Option<IndexReplayCheckpoint>>>,
    fail_next_commit: Arc<AtomicBool>,
    order: Arc<Mutex<Vec<&'static str>>>,
) -> IndexReplayWorker<RecordingMutationSink, RecordingCheckpointStore> {
    let mut schema_catalog = IndexSchemaSourceCatalog::new();
    schema_catalog.register("product", schema()).unwrap();

    let mut source_catalog = IndexSourceCatalog::new();
    source_catalog
        .register(
            "product",
            "product-primary",
            [schema_ref()],
            ReplaySource {
                calls: source_calls,
                mutation: mutation(tenant_id, Uuid::from_u128(20)),
            },
        )
        .unwrap();
    let sources = source_catalog.materialize(&schema_catalog).unwrap();

    let mut registry = SchemaRegistry::new();
    registry.register(schema()).unwrap();

    IndexReplayWorker::new(
        sources,
        Arc::new(registry),
        RecordingMutationSink {
            calls: mutation_calls,
            order: order.clone(),
        },
        RecordingCheckpointStore {
            checkpoint,
            fail_next_commit,
            order,
        },
    )
}

#[tokio::test]
async fn replay_page_commits_checkpoint_after_mutations() {
    let tenant_id = Uuid::from_u128(1);
    let source_calls = Arc::new(AtomicUsize::new(0));
    let mutation_calls = Arc::new(AtomicUsize::new(0));
    let checkpoint = Arc::new(Mutex::new(None));
    let order = Arc::new(Mutex::new(Vec::new()));
    let worker = worker(
        tenant_id,
        source_calls,
        mutation_calls,
        checkpoint.clone(),
        Arc::new(AtomicBool::new(false)),
        order.clone(),
    );

    let outcome = worker
        .run_next_page(IndexReplayPageRequest::new(tenant_id, schema_ref(), "", 10).unwrap())
        .await
        .unwrap();

    assert_eq!(outcome.status(), IndexReplayPageStatus::Complete);
    assert_eq!(outcome.applied_count(), 1);
    assert_eq!(*order.lock().unwrap(), vec!["mutation", "checkpoint"]);
    assert!(checkpoint.lock().unwrap().as_ref().unwrap().is_complete());
}

#[tokio::test]
async fn checkpoint_failure_replays_the_page_through_inbox_identity() {
    let tenant_id = Uuid::from_u128(2);
    let source_calls = Arc::new(AtomicUsize::new(0));
    let mutation_calls = Arc::new(AtomicUsize::new(0));
    let checkpoint = Arc::new(Mutex::new(None));
    let fail_next_commit = Arc::new(AtomicBool::new(true));
    let worker = worker(
        tenant_id,
        source_calls.clone(),
        mutation_calls.clone(),
        checkpoint.clone(),
        fail_next_commit,
        Arc::new(Mutex::new(Vec::new())),
    );
    let request = IndexReplayPageRequest::new(tenant_id, schema_ref(), "", 10).unwrap();

    assert!(matches!(
        worker.run_next_page(request.clone()).await,
        Err(IndexReplayError::CheckpointCommitFailed(_))
    ));
    assert!(checkpoint.lock().unwrap().is_none());

    let outcome = worker.run_next_page(request).await.unwrap();
    assert_eq!(outcome.duplicate_count(), 1);
    assert_eq!(source_calls.load(Ordering::SeqCst), 2);
    assert_eq!(mutation_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn completed_checkpoint_skips_the_source() {
    let tenant_id = Uuid::from_u128(3);
    let source_calls = Arc::new(AtomicUsize::new(0));
    let mutation_calls = Arc::new(AtomicUsize::new(0));
    let key = IndexReplayCheckpointKey::new(
        tenant_id,
        "product-primary",
        schema_ref(),
        "",
    )
    .unwrap();
    let checkpoint = Arc::new(Mutex::new(Some(
        IndexReplayCheckpoint::new(key, None, Some(7), Some(Uuid::from_u128(10).to_string()))
            .unwrap(),
    )));
    let worker = worker(
        tenant_id,
        source_calls.clone(),
        mutation_calls.clone(),
        checkpoint,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(Vec::new())),
    );

    let outcome = worker
        .run_next_page(IndexReplayPageRequest::new(tenant_id, schema_ref(), "", 10).unwrap())
        .await
        .unwrap();

    assert_eq!(outcome.status(), IndexReplayPageStatus::AlreadyComplete);
    assert_eq!(source_calls.load(Ordering::SeqCst), 0);
    assert_eq!(mutation_calls.load(Ordering::SeqCst), 0);
}
