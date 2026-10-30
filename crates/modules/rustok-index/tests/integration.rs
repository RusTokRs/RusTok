use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_core::{EventBus, ModuleRuntimeExtensions};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation, IndexRecord,
    IndexReplayDryRunRequest, IndexReplayDryRunStatus, IndexSchema, IndexSource, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
    materialize_index_replay_dry_run_runtime, materialize_index_schema_registry,
    materialize_index_source_registry, register_index_schema_source, register_index_source,
};
use tokio::sync::broadcast;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct ContentIndexSource;

#[async_trait]
impl IndexSource for ContentIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let entity_id = Uuid::from_u128(42);
        let mutations = vec![IndexMutation::Upsert {
            event_id: Uuid::now_v7(),
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: request.tenant_id(),
                    schema: request.schema().clone(),
                    entity_id,
                    locale: None,
                },
                source_version: 1,
                fields: BTreeMap::from([(
                    FieldName::new("id").expect("id is a valid field name"),
                    IndexValue::Uuid(entity_id),
                )]),
                links: Vec::new(),
            },
        }];
        Ok(IndexSourcePage::new(&request, mutations, None)
            .expect("content fixture satisfies page scope"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty load is valid"))
    }
}

fn content_schema() -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("content").expect("valid module name"),
            entity: EntityName::new("article").expect("valid entity name"),
            version: SchemaVersion::INITIAL,
        },
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").expect("valid field name"),
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

#[tokio::test]
async fn test_index_rebuild_flow() -> TestResult<()> {
    let mut extensions = ModuleRuntimeExtensions::default();
    let schema = content_schema();
    let schema_ref = schema.reference.clone();

    register_index_schema_source(&mut extensions, "content", schema.clone())?;
    register_index_source(
        &mut extensions,
        "content",
        "content-primary",
        [schema_ref.clone()],
        ContentIndexSource,
    )?;

    let schemas = materialize_index_schema_registry(&extensions)?.expect("schema registry");
    let sources = materialize_index_source_registry(&extensions)?.expect("source registry");
    extensions.insert(schemas);
    extensions.insert(sources);

    let dry_run_runtime =
        materialize_index_replay_dry_run_runtime(&mut extensions)?.expect("dry run runtime");

    let tenant_id = Uuid::now_v7();
    let bus = EventBus::new();
    let mut events = bus.subscribe();

    let event = DomainEvent::ReindexRequested {
        target_type: "content".to_string(),
        target_id: None,
    };

    bus.publish(tenant_id, None, event)?;

    let envelope = next_event(&mut events).await?;
    assert!(matches!(
        envelope.event,
        DomainEvent::ReindexRequested { .. }
    ));

    // Execute dry run replay for the reindex request
    let request = IndexReplayDryRunRequest::new(
        tenant_id,
        SchemaRef {
            module: ModuleName::new("content")?,
            entity: EntityName::new("article")?,
            version: SchemaVersion::INITIAL,
        },
        None,
        100,
        10,
    )?;

    let outcome = dry_run_runtime.run(request).await?;
    assert_eq!(outcome.status(), IndexReplayDryRunStatus::Complete);
    assert_eq!(outcome.pages_scanned(), 1);
    assert_eq!(outcome.mutation_count(), 1);

    Ok(())
}

async fn next_event(
    receiver: &mut broadcast::Receiver<EventEnvelope>,
) -> TestResult<EventEnvelope> {
    let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .map_err(|_| "timed out waiting for event")??;
    Ok(envelope)
}
