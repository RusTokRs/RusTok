use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use uuid::Uuid;

use super::*;
use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation,
    IndexMutationAcknowledgeFailure, IndexMutationEventAcknowledger, IndexMutationEventCatalog,
    IndexRecord, IndexReplayFailure, IndexReplayMutationOutcome, IndexReplayMutationSink,
    IndexSchema, IndexSchemaSourceCatalog, IndexSource, IndexSourceCatalog, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleKey, LocaleMode, ModuleName, SchemaRef, SchemaRegistry,
    SchemaVersion, SharedIndexMutationEventRegistry, SharedIndexSourceRegistry,
};

const EVENT_DOMAIN: &str = "product.index.product-locale-refresh-v1";
const SOURCE_NAME: &str = "product-postgres-primary";

#[derive(Clone)]
struct ExactSource {
    mutation: Option<IndexMutation>,
    calls: Arc<Mutex<usize>>,
}

#[async_trait]
impl IndexSource for ExactSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        Ok(IndexSourcePage::new(&request, Vec::new(), None).expect("valid empty page"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        *self.calls.lock().expect("source call lock") += 1;
        IndexSourceLoadBatch::new(&request, self.mutation.clone().into_iter().collect()).map_err(
            |_| {
                IndexSourceFailure::permanent("source_refresh_fixture_invalid")
                    .expect("static failure code")
            },
        )
    }
}

#[derive(Clone)]
struct RecordingSink {
    calls: Arc<Mutex<Vec<&'static str>>>,
    observed_event_id: Arc<Mutex<Option<Uuid>>>,
}

#[async_trait]
impl IndexReplayMutationSink for RecordingSink {
    async fn apply_replay_mutation(
        &self,
        _registry: &SchemaRegistry,
        _source_name: &str,
        mutation: &IndexMutation,
    ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
        self.calls.lock().expect("sink call lock").push("apply");
        *self.observed_event_id.lock().expect("observed event lock") = Some(mutation.event_id());
        Ok(IndexReplayMutationOutcome::Applied)
    }
}

#[derive(Clone)]
struct RecordingAcknowledger {
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl IndexMutationEventAcknowledger for RecordingAcknowledger {
    type Token = String;

    async fn acknowledge(
        &self,
        _token: &Self::Token,
    ) -> Result<(), IndexMutationAcknowledgeFailure> {
        self.calls.lock().expect("ack call lock").push("ack");
        Ok(())
    }
}

fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").expect("module"),
        entity: EntityName::new("product").expect("entity"),
        version: SchemaVersion::new(2),
    }
}

fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::Required,
        fields: vec![IndexField {
            name: FieldName::new("title").expect("field"),
            value_type: IndexValueType::String,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

fn key() -> EntityKey {
    EntityKey {
        tenant_id: Uuid::from_u128(1),
        schema: schema_ref(),
        entity_id: Uuid::from_u128(2),
        locale: Some(LocaleKey::new("en-US").expect("locale")),
    }
}

fn mutation(source_version: u64) -> IndexMutation {
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(99),
        record: IndexRecord {
            key: key(),
            source_version,
            fields: BTreeMap::from([(
                FieldName::new("title").expect("field"),
                IndexValue::String("Product".to_owned()),
            )]),
            links: Vec::new(),
        },
    }
}

fn registries(
    mutation: Option<IndexMutation>,
    source_calls: Arc<Mutex<usize>>,
) -> (SharedIndexSourceRegistry, SharedIndexMutationEventRegistry) {
    let mut schemas = IndexSchemaSourceCatalog::new();
    schemas.register("product", schema()).expect("schema owner");

    let mut sources = IndexSourceCatalog::new();
    sources
        .register(
            "product",
            SOURCE_NAME,
            [schema_ref()],
            ExactSource {
                mutation,
                calls: source_calls,
            },
        )
        .expect("source");
    let shared_sources = sources.materialize(&schemas).expect("source registry");

    let mut events = IndexMutationEventCatalog::new();
    events
        .register("product", EVENT_DOMAIN, SOURCE_NAME, schema_ref())
        .expect("event route");
    let shared_events = events.materialize(&sources).expect("event registry");
    (shared_sources, shared_events)
}

fn delivery(minimum_source_version: u64) -> IndexSourceRefreshEventDelivery<String> {
    IndexSourceRefreshEventDelivery::new(
        EVENT_DOMAIN,
        Uuid::from_u128(7),
        key(),
        minimum_source_version,
        "broker-position-7".to_owned(),
    )
    .expect("delivery")
}

#[tokio::test]
async fn canonical_source_mutation_is_rebound_committed_and_then_acknowledged() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(0));
    let observed_event_id = Arc::new(Mutex::new(None));
    let (sources, events) = registries(Some(mutation(9)), source_calls.clone());
    let worker = IndexSourceRefreshEventWorker::new(
        RecordingSink {
            calls: calls.clone(),
            observed_event_id: observed_event_id.clone(),
        },
        RecordingAcknowledger {
            calls: calls.clone(),
        },
    );

    let outcome = worker
        .process(&SchemaRegistry::default(), &sources, &events, delivery(7))
        .await
        .expect("source refresh should commit and acknowledge");

    assert_eq!(outcome.event_id(), Uuid::from_u128(7));
    assert_eq!(outcome.source_name(), SOURCE_NAME);
    assert_eq!(outcome.source_version(), 9);
    assert_eq!(
        outcome.mutation_outcome(),
        IndexReplayMutationOutcome::Applied
    );
    assert_eq!(*source_calls.lock().expect("source call lock"), 1);
    assert_eq!(*calls.lock().expect("call lock"), vec!["apply", "ack"]);
    assert_eq!(
        *observed_event_id.lock().expect("observed event lock"),
        Some(Uuid::from_u128(7))
    );
}

#[tokio::test]
async fn missing_or_behind_source_state_suppresses_apply_and_ack() {
    for source_mutation in [None, Some(mutation(6))] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_calls = Arc::new(Mutex::new(0));
        let (sources, events) = registries(source_mutation, source_calls.clone());
        let worker = IndexSourceRefreshEventWorker::new(
            RecordingSink {
                calls: calls.clone(),
                observed_event_id: Arc::new(Mutex::new(None)),
            },
            RecordingAcknowledger {
                calls: calls.clone(),
            },
        );

        let result = worker
            .process(&SchemaRegistry::default(), &sources, &events, delivery(7))
            .await;

        assert!(result.is_err());
        assert_eq!(*source_calls.lock().expect("source call lock"), 1);
        assert!(calls.lock().expect("call lock").is_empty());
    }
}

#[tokio::test]
async fn schema_mismatch_fails_before_source_load_apply_or_ack() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(0));
    let (sources, events) = registries(Some(mutation(9)), source_calls.clone());
    let worker = IndexSourceRefreshEventWorker::new(
        RecordingSink {
            calls: calls.clone(),
            observed_event_id: Arc::new(Mutex::new(None)),
        },
        RecordingAcknowledger {
            calls: calls.clone(),
        },
    );
    let mut wrong_key = key();
    wrong_key.schema.version = SchemaVersion::INITIAL;
    let wrong_delivery = IndexSourceRefreshEventDelivery::new(
        EVENT_DOMAIN,
        Uuid::from_u128(7),
        wrong_key,
        7,
        "broker-position-7".to_owned(),
    )
    .expect("structurally valid delivery");

    assert!(
        worker
            .process(
                &SchemaRegistry::default(),
                &sources,
                &events,
                wrong_delivery,
            )
            .await
            .is_err()
    );
    assert_eq!(*source_calls.lock().expect("source call lock"), 0);
    assert!(calls.lock().expect("call lock").is_empty());
}

#[test]
fn delivery_rejects_invalid_identity_and_revision() {
    assert!(matches!(
        IndexSourceRefreshEventDelivery::new("BAD DOMAIN", Uuid::from_u128(7), key(), 1, (),),
        Err(IndexSourceRefreshEventError::InvalidEventDomain(_))
    ));
    assert!(matches!(
        IndexSourceRefreshEventDelivery::new(EVENT_DOMAIN, Uuid::nil(), key(), 1, ()),
        Err(IndexSourceRefreshEventError::NilEventId)
    ));
    assert!(matches!(
        IndexSourceRefreshEventDelivery::new(EVENT_DOMAIN, Uuid::from_u128(7), key(), 0, (),),
        Err(IndexSourceRefreshEventError::ZeroMinimumSourceVersion)
    ));
}
