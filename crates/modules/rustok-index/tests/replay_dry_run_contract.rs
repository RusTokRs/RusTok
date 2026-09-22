use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation, IndexRecord,
    IndexReplayDryRunError, IndexReplayDryRunRequest, IndexReplayDryRunRuntimeCompositionError,
    IndexReplayDryRunStatus, IndexSchema, IndexSource, IndexSourceFailure, IndexSourceLoadBatch,
    IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest, IndexValue, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion, SharedIndexReplayDryRunRuntime,
    materialize_index_replay_dry_run_runtime, materialize_index_schema_registry,
    materialize_index_source_registry, register_index_schema_source, register_index_source,
};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum EventIdentityMode {
    Valid,
    Nil,
    Duplicate,
}

struct EventIdentitySource {
    mode: EventIdentityMode,
}

impl EventIdentitySource {
    fn event_ids(&self) -> Vec<Uuid> {
        match self.mode {
            EventIdentityMode::Valid => vec![Uuid::from_u128(1_001)],
            EventIdentityMode::Nil => vec![Uuid::nil()],
            EventIdentityMode::Duplicate => {
                let event_id = Uuid::from_u128(1_001);
                vec![event_id, event_id]
            }
        }
    }
}

#[async_trait]
impl IndexSource for EventIdentitySource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let mutations = self
            .event_ids()
            .into_iter()
            .enumerate()
            .map(|(position, event_id)| {
                let entity_id = Uuid::from_u128(100 + position as u128);
                IndexMutation::Upsert {
                    event_id,
                    record: IndexRecord {
                        key: EntityKey {
                            tenant_id: request.tenant_id(),
                            schema: request.schema().clone(),
                            entity_id,
                            locale: None,
                        },
                        source_version: position as u64 + 1,
                        fields: BTreeMap::from([(
                            FieldName::new("id").unwrap(),
                            IndexValue::Uuid(entity_id),
                        )]),
                        links: Vec::new(),
                    },
                }
            })
            .collect();

        Ok(IndexSourcePage::new(&request, mutations, None)
            .expect("event-identity fixtures must satisfy source-page scope"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new())
            .expect("empty targeted load should be valid"))
    }
}

fn schema() -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("dry-run-contract-owner").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        },
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

fn registered_extensions(mode: EventIdentityMode) -> ModuleRuntimeExtensions {
    let mut extensions = ModuleRuntimeExtensions::default();
    let schema = schema();
    register_index_schema_source(&mut extensions, "dry_run_contract_owner", schema.clone())
        .unwrap();
    register_index_source(
        &mut extensions,
        "dry_run_contract_owner",
        "dry-run-contract-primary",
        [schema.reference],
        EventIdentitySource { mode },
    )
    .unwrap();
    extensions
}

fn complete_extensions(mode: EventIdentityMode) -> ModuleRuntimeExtensions {
    let mut extensions = registered_extensions(mode);
    let schemas = materialize_index_schema_registry(&extensions)
        .unwrap()
        .expect("schema registry");
    let sources = materialize_index_source_registry(&extensions)
        .unwrap()
        .expect("source registry");
    extensions.insert(schemas);
    extensions.insert(sources);
    extensions
}

fn runtime(mode: EventIdentityMode) -> SharedIndexReplayDryRunRuntime {
    let mut extensions = complete_extensions(mode);
    materialize_index_replay_dry_run_runtime(&mut extensions)
        .unwrap()
        .expect("dry-run runtime")
}

fn request() -> IndexReplayDryRunRequest {
    IndexReplayDryRunRequest::new(Uuid::from_u128(1), schema().reference, None, 10, 1).unwrap()
}

#[tokio::test]
async fn dry_run_rejects_nil_event_id_before_accepting_the_page() {
    let error = runtime(EventIdentityMode::Nil)
        .run(request())
        .await
        .expect_err("nil event identity must fail closed");

    assert!(matches!(
        error,
        IndexReplayDryRunError::NilEventId {
            page_index: 0,
            position: 0,
        }
    ));
}

#[tokio::test]
async fn dry_run_rejects_page_local_duplicate_event_id() {
    let duplicate = Uuid::from_u128(1_001);
    let error = runtime(EventIdentityMode::Duplicate)
        .run(request())
        .await
        .expect_err("duplicate page event identity must fail closed");

    assert!(matches!(
        error,
        IndexReplayDryRunError::DuplicateEventId {
            page_index: 0,
            position: 1,
            event_id,
        } if event_id == duplicate
    ));
}

#[tokio::test]
async fn valid_event_identity_page_completes_without_a_resume_cursor() {
    let outcome = runtime(EventIdentityMode::Valid)
        .run(request())
        .await
        .expect("valid page should complete");

    assert_eq!(outcome.status(), IndexReplayDryRunStatus::Complete);
    assert_eq!(outcome.pages_scanned(), 1);
    assert_eq!(outcome.mutation_count(), 1);
    assert!(outcome.next_cursor().is_none());
}

#[test]
fn dry_run_materialization_requires_the_complete_registry_pair() {
    let mut missing_sources = ModuleRuntimeExtensions::default();
    register_index_schema_source(&mut missing_sources, "dry_run_contract_owner", schema()).unwrap();
    let schemas = materialize_index_schema_registry(&missing_sources)
        .unwrap()
        .expect("schema registry");
    missing_sources.insert(schemas);
    assert!(
        materialize_index_replay_dry_run_runtime(&mut missing_sources)
            .unwrap()
            .is_none()
    );

    let mut missing_schemas = registered_extensions(EventIdentityMode::Valid);
    let sources = materialize_index_source_registry(&missing_schemas)
        .unwrap()
        .expect("source registry");
    missing_schemas.insert(sources);
    assert_eq!(
        materialize_index_replay_dry_run_runtime(&mut missing_schemas).unwrap_err(),
        IndexReplayDryRunRuntimeCompositionError::MissingSchemaRegistry
    );
}

#[test]
fn dry_run_runtime_is_single_assignment() {
    let mut extensions = complete_extensions(EventIdentityMode::Valid);
    let first = materialize_index_replay_dry_run_runtime(&mut extensions)
        .unwrap()
        .expect("complete registries should publish dry-run runtime");
    assert!(extensions.contains::<SharedIndexReplayDryRunRuntime>());
    assert!(format!("{first:?}").contains("SharedIndexReplayDryRunRuntime"));

    assert_eq!(
        materialize_index_replay_dry_run_runtime(&mut extensions).unwrap_err(),
        IndexReplayDryRunRuntimeCompositionError::AlreadyMaterialized
    );
}
