use std::{collections::BTreeMap, fmt, sync::Arc};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexMutation, SchemaRef, SchemaRegistry};

use super::{
    IndexReplayFailure, IndexReplayMutationOutcome, IndexReplayMutationSink, IndexSourceCatalog,
};

const MAX_OWNER_MODULE_BYTES: usize = 128;
const MAX_EVENT_DOMAIN_BYTES: usize = 128;
const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_FAILURE_CODE_BYTES: usize = 128;

/// One immutable route from a versioned owner event domain to the exact replay source/schema
/// contract used for durable mutation persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexMutationEventDescriptor {
    owner_module: String,
    event_domain: String,
    source_name: String,
    schema: SchemaRef,
}

impl IndexMutationEventDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn event_domain(&self) -> &str {
        &self.event_domain
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }
}

/// Mutable composition-time event route catalog.
///
/// Every exact schema may have at most one incremental event domain. Materialization also
/// requires the named replay source to exist, have the same owner, and declare the exact schema.
#[derive(Debug, Clone, Default)]
pub struct IndexMutationEventCatalog {
    routes: BTreeMap<String, IndexMutationEventDescriptor>,
    schema_domains: BTreeMap<SchemaRef, String>,
}

impl IndexMutationEventCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn get(&self, event_domain: &str) -> Option<&IndexMutationEventDescriptor> {
        self.routes.get(event_domain)
    }

    pub fn event_for_schema(&self, schema: &SchemaRef) -> Option<&IndexMutationEventDescriptor> {
        self.schema_domains
            .get(schema)
            .and_then(|event_domain| self.routes.get(event_domain))
    }

    pub fn iter(&self) -> impl Iterator<Item = &IndexMutationEventDescriptor> {
        self.routes.values()
    }

    pub fn register(
        &mut self,
        owner_module: impl Into<String>,
        event_domain: impl Into<String>,
        source_name: impl Into<String>,
        schema: SchemaRef,
    ) -> Result<(), IndexMutationEventError> {
        let owner_module = owner_module.into();
        let event_domain = event_domain.into();
        let source_name = source_name.into();

        if !valid_owner_module(&owner_module) {
            return Err(IndexMutationEventError::InvalidOwnerModule(owner_module));
        }
        if !valid_machine_name(&event_domain, MAX_EVENT_DOMAIN_BYTES) {
            return Err(IndexMutationEventError::InvalidEventDomain(event_domain));
        }
        if !valid_machine_name(&source_name, MAX_SOURCE_NAME_BYTES) {
            return Err(IndexMutationEventError::InvalidSourceName(source_name));
        }
        if self.routes.contains_key(&event_domain) {
            return Err(IndexMutationEventError::DuplicateEventDomain(event_domain));
        }
        if let Some(existing_domain) = self.schema_domains.get(&schema) {
            return Err(IndexMutationEventError::SchemaEventConflict {
                schema,
                existing_domain: existing_domain.clone(),
                incoming_domain: event_domain,
            });
        }

        self.schema_domains
            .insert(schema.clone(), event_domain.clone());
        self.routes.insert(
            event_domain.clone(),
            IndexMutationEventDescriptor {
                owner_module,
                event_domain,
                source_name,
                schema,
            },
        );
        Ok(())
    }

    pub fn materialize(
        &self,
        source_catalog: &IndexSourceCatalog,
    ) -> Result<SharedIndexMutationEventRegistry, IndexMutationEventError> {
        for descriptor in self.routes.values() {
            let source = source_catalog
                .get(descriptor.source_name())
                .ok_or_else(|| IndexMutationEventError::UnknownReplaySource {
                    event_domain: descriptor.event_domain.clone(),
                    source_name: descriptor.source_name.clone(),
                })?;
            if source.owner_module() != descriptor.owner_module() {
                return Err(IndexMutationEventError::ReplaySourceOwnerMismatch {
                    event_domain: descriptor.event_domain.clone(),
                    source_name: descriptor.source_name.clone(),
                    event_owner: descriptor.owner_module.clone(),
                    source_owner: source.owner_module().to_owned(),
                });
            }
            if !source.schemas().contains(descriptor.schema()) {
                return Err(IndexMutationEventError::ReplaySourceSchemaMismatch {
                    event_domain: descriptor.event_domain.clone(),
                    source_name: descriptor.source_name.clone(),
                    schema: descriptor.schema.clone(),
                });
            }
        }

        Ok(SharedIndexMutationEventRegistry(Arc::new(
            IndexMutationEventRegistry {
                routes: self.routes.clone(),
                schema_domains: self.schema_domains.clone(),
            },
        )))
    }
}

#[derive(Debug)]
struct IndexMutationEventRegistry {
    routes: BTreeMap<String, IndexMutationEventDescriptor>,
    schema_domains: BTreeMap<SchemaRef, String>,
}

#[derive(Clone)]
pub struct SharedIndexMutationEventRegistry(Arc<IndexMutationEventRegistry>);

impl fmt::Debug for SharedIndexMutationEventRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexMutationEventRegistry")
            .field("route_count", &self.len())
            .finish()
    }
}

impl SharedIndexMutationEventRegistry {
    pub fn len(&self) -> usize {
        self.0.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.routes.is_empty()
    }

    pub fn get(&self, event_domain: &str) -> Option<&IndexMutationEventDescriptor> {
        self.0.routes.get(event_domain)
    }

    pub fn event_for_schema(&self, schema: &SchemaRef) -> Option<&IndexMutationEventDescriptor> {
        self.0
            .schema_domains
            .get(schema)
            .and_then(|event_domain| self.0.routes.get(event_domain))
    }
}

/// One broker delivery carrying an already-decoded generic Index mutation and the exact
/// broker-owned acknowledgement token for this delivery attempt.
pub struct IndexMutationEventDelivery<T> {
    event_domain: String,
    mutation: IndexMutation,
    acknowledgement_token: T,
}

impl<T> IndexMutationEventDelivery<T> {
    pub fn new(
        event_domain: impl Into<String>,
        mutation: IndexMutation,
        acknowledgement_token: T,
    ) -> Result<Self, IndexMutationEventError> {
        let event_domain = event_domain.into();
        if !valid_machine_name(&event_domain, MAX_EVENT_DOMAIN_BYTES) {
            return Err(IndexMutationEventError::InvalidEventDomain(event_domain));
        }
        if mutation.event_id().is_nil() {
            return Err(IndexMutationEventError::NilEventId);
        }
        if mutation.source_version() == 0 {
            return Err(IndexMutationEventError::ZeroSourceVersion);
        }
        if mutation.key().tenant_id.is_nil() {
            return Err(IndexMutationEventError::NilTenantId);
        }
        if mutation.key().entity_id.is_nil() {
            return Err(IndexMutationEventError::NilEntityId);
        }
        Ok(Self {
            event_domain,
            mutation,
            acknowledgement_token,
        })
    }

    pub fn event_domain(&self) -> &str {
        &self.event_domain
    }

    pub fn mutation(&self) -> &IndexMutation {
        &self.mutation
    }

    pub fn acknowledgement_token(&self) -> &T {
        &self.acknowledgement_token
    }

    pub fn into_parts(self) -> (String, IndexMutation, T) {
        (self.event_domain, self.mutation, self.acknowledgement_token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMutationAcknowledgeFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index mutation acknowledgement reported a {kind:?} failure ({code})")]
pub struct IndexMutationAcknowledgeFailure {
    kind: IndexMutationAcknowledgeFailureKind,
    code: String,
}

impl IndexMutationAcknowledgeFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexMutationEventError> {
        Self::new(IndexMutationAcknowledgeFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexMutationEventError> {
        Self::new(IndexMutationAcknowledgeFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexMutationAcknowledgeFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexMutationEventError> {
        let code = code.into();
        if !valid_machine_name(&code, MAX_FAILURE_CODE_BYTES) {
            return Err(IndexMutationEventError::InvalidAcknowledgeFailureCode(code));
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexMutationAcknowledgeFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

/// Broker-specific acknowledgement adapter. The token remains opaque to Index and is never
/// logged, persisted, or derived from the logical event UUID.
#[async_trait]
pub trait IndexMutationEventAcknowledger: Send + Sync {
    type Token: Send + Sync;

    async fn acknowledge(&self, token: &Self::Token)
    -> Result<(), IndexMutationAcknowledgeFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexMutationEventProcessOutcome {
    event_id: Uuid,
    source_name: String,
    mutation_outcome: IndexReplayMutationOutcome,
}

impl IndexMutationEventProcessOutcome {
    pub fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn mutation_outcome(&self) -> IndexReplayMutationOutcome {
        self.mutation_outcome
    }
}

/// Database-neutral commit-before-ack orchestration.
///
/// The mutation sink must return only after its inbox/entity transaction is durable. Applied,
/// duplicate, and stale outcomes are all terminal deliveries and are acknowledged. Any mutation
/// failure suppresses acknowledgement. An acknowledgement failure is returned after durable commit,
/// so broker redelivery is expected and remains safe through inbox deduplication and source-version
/// monotonicity.
pub struct IndexMutationEventWorker<M, A> {
    mutation_sink: M,
    acknowledger: A,
}

impl<M, A> IndexMutationEventWorker<M, A>
where
    M: IndexReplayMutationSink,
    A: IndexMutationEventAcknowledger,
{
    pub fn new(mutation_sink: M, acknowledger: A) -> Self {
        Self {
            mutation_sink,
            acknowledger,
        }
    }

    pub async fn process(
        &self,
        schema_registry: &SchemaRegistry,
        event_registry: &SharedIndexMutationEventRegistry,
        delivery: IndexMutationEventDelivery<A::Token>,
    ) -> Result<IndexMutationEventProcessOutcome, IndexMutationEventProcessError> {
        let (event_domain, mutation, acknowledgement_token) = delivery.into_parts();
        let descriptor = event_registry.get(&event_domain).ok_or_else(|| {
            IndexMutationEventProcessError::UnknownEventDomain(event_domain.clone())
        })?;
        if mutation.key().schema != *descriptor.schema() {
            return Err(IndexMutationEventProcessError::MutationSchemaMismatch {
                event_domain,
                expected: descriptor.schema().clone(),
                actual: mutation.key().schema.clone(),
            });
        }

        let event_id = mutation.event_id();
        let mutation_outcome = self
            .mutation_sink
            .apply_replay_mutation(schema_registry, descriptor.source_name(), &mutation)
            .await
            .map_err(IndexMutationEventProcessError::Mutation)?;

        self.acknowledger
            .acknowledge(&acknowledgement_token)
            .await
            .map_err(IndexMutationEventProcessError::Acknowledge)?;

        Ok(IndexMutationEventProcessOutcome {
            event_id,
            source_name: descriptor.source_name().to_owned(),
            mutation_outcome,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexMutationEventError {
    #[error("Index mutation event owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error("Index mutation event domain is invalid: {0}")]
    InvalidEventDomain(String),
    #[error("Index mutation event replay source name is invalid: {0}")]
    InvalidSourceName(String),
    #[error("Index mutation acknowledgement failure code is invalid: {0}")]
    InvalidAcknowledgeFailureCode(String),
    #[error("Index mutation event domain is already registered: {0}")]
    DuplicateEventDomain(String),
    #[error(
        "Index schema {schema} has multiple mutation event domains: existing={existing_domain}, incoming={incoming_domain}"
    )]
    SchemaEventConflict {
        schema: SchemaRef,
        existing_domain: String,
        incoming_domain: String,
    },
    #[error("Index mutation event catalog exists without an Index replay source catalog")]
    MissingSourceCatalog,
    #[error("Index mutation event {event_domain} references unknown replay source {source_name}")]
    UnknownReplaySource {
        event_domain: String,
        source_name: String,
    },
    #[error(
        "Index mutation event {event_domain} owner does not match replay source {source_name}: event={event_owner}, source={source_owner}"
    )]
    ReplaySourceOwnerMismatch {
        event_domain: String,
        source_name: String,
        event_owner: String,
        source_owner: String,
    },
    #[error(
        "Index mutation event {event_domain} replay source {source_name} does not own schema {schema}"
    )]
    ReplaySourceSchemaMismatch {
        event_domain: String,
        source_name: String,
        schema: SchemaRef,
    },
    #[error("Index mutation event UUID cannot be nil")]
    NilEventId,
    #[error("Index mutation event source version must be positive")]
    ZeroSourceVersion,
    #[error("Index mutation event tenant UUID cannot be nil")]
    NilTenantId,
    #[error("Index mutation event entity UUID cannot be nil")]
    NilEntityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexMutationEventProcessError {
    #[error("Unknown Index mutation event domain: {0}")]
    UnknownEventDomain(String),
    #[error("Index mutation event {event_domain} carries schema {actual}, expected {expected}")]
    MutationSchemaMismatch {
        event_domain: String,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("Index mutation event persistence failed")]
    Mutation(#[source] IndexReplayFailure),
    #[error("Index mutation event acknowledgement failed after durable persistence")]
    Acknowledge(#[source] IndexMutationAcknowledgeFailure),
}

pub fn register_index_mutation_event(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    event_domain: impl Into<String>,
    source_name: impl Into<String>,
    schema: SchemaRef,
) -> Result<(), IndexMutationEventError> {
    extensions
        .get_or_insert_with::<IndexMutationEventCatalog, _>(IndexMutationEventCatalog::new)
        .register(owner_module, event_domain, source_name, schema)
}

pub fn materialize_index_mutation_event_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexMutationEventRegistry>, IndexMutationEventError> {
    let Some(catalog) = extensions.get::<IndexMutationEventCatalog>() else {
        return Ok(None);
    };
    if catalog.is_empty() {
        return Ok(None);
    }
    let source_catalog = extensions
        .get::<IndexSourceCatalog>()
        .ok_or(IndexMutationEventError::MissingSourceCatalog)?;
    catalog.materialize(source_catalog).map(Some)
}

fn valid_owner_module(value: &str) -> bool {
    valid_bounded_ascii(value, MAX_OWNER_MODULE_BYTES, false)
}

fn valid_machine_name(value: &str, max_bytes: usize) -> bool {
    valid_bounded_ascii(value, max_bytes, true)
}

fn valid_bounded_ascii(value: &str, max_bytes: usize, allow_dot: bool) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_')
                || (allow_dot && byte == b'.')
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{
        EntityKey, EntityName, IndexSource, IndexSourceFailure, IndexSourceLoadBatch,
        IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest, ModuleName, SchemaVersion,
    };

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
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
            Ok(IndexSourceLoadBatch::new(&request, Vec::new()).unwrap())
        }
    }

    #[derive(Clone)]
    struct RecordingSink {
        calls: Arc<Mutex<Vec<&'static str>>>,
        result: Result<IndexReplayMutationOutcome, IndexReplayFailure>,
    }

    #[async_trait]
    impl IndexReplayMutationSink for RecordingSink {
        async fn apply_replay_mutation(
            &self,
            _registry: &SchemaRegistry,
            _source_name: &str,
            _mutation: &IndexMutation,
        ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
            self.calls.lock().unwrap().push("apply");
            self.result.clone()
        }
    }

    #[derive(Clone)]
    struct RecordingAcknowledger {
        calls: Arc<Mutex<Vec<&'static str>>>,
        result: Result<(), IndexMutationAcknowledgeFailure>,
    }

    #[async_trait]
    impl IndexMutationEventAcknowledger for RecordingAcknowledger {
        type Token = String;

        async fn acknowledge(
            &self,
            _token: &Self::Token,
        ) -> Result<(), IndexMutationAcknowledgeFailure> {
            self.calls.lock().unwrap().push("ack");
            self.result.clone()
        }
    }

    fn schema_ref(version: u32) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::new(version),
        }
    }

    fn mutation(schema: SchemaRef) -> IndexMutation {
        IndexMutation::Delete {
            event_id: Uuid::from_u128(4),
            key: EntityKey {
                tenant_id: Uuid::from_u128(1),
                schema,
                entity_id: Uuid::from_u128(2),
                locale: None,
            },
            source_version: 7,
        }
    }

    fn event_registry() -> SharedIndexMutationEventRegistry {
        let schema = schema_ref(1);
        let mut sources = IndexSourceCatalog::new();
        sources
            .register(
                "product",
                "product-postgres-primary",
                [schema.clone()],
                NoopSource,
            )
            .unwrap();
        let mut events = IndexMutationEventCatalog::new();
        events
            .register(
                "product",
                "rustok-product.product-v1",
                "product-postgres-primary",
                schema,
            )
            .unwrap();
        events.materialize(&sources).unwrap()
    }

    #[test]
    fn event_catalog_is_exact_and_source_owned() {
        let schema = schema_ref(1);
        let mut catalog = IndexMutationEventCatalog::new();
        catalog
            .register(
                "product",
                "rustok-product.product-v1",
                "product-postgres-primary",
                schema.clone(),
            )
            .unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(
            catalog.event_for_schema(&schema).unwrap().event_domain(),
            "rustok-product.product-v1"
        );
        assert_eq!(
            catalog.register(
                "product",
                "rustok-product.product-v1",
                "product-postgres-primary",
                schema_ref(2),
            ),
            Err(IndexMutationEventError::DuplicateEventDomain(
                "rustok-product.product-v1".to_owned()
            ))
        );
        assert_eq!(
            catalog.register(
                "product",
                "rustok-product.product-v1-other",
                "product-postgres-primary",
                schema.clone(),
            ),
            Err(IndexMutationEventError::SchemaEventConflict {
                schema,
                existing_domain: "rustok-product.product-v1".to_owned(),
                incoming_domain: "rustok-product.product-v1-other".to_owned(),
            })
        );
    }

    #[test]
    fn event_materialization_requires_the_exact_replay_source_owner_and_schema() {
        let schema = schema_ref(1);
        let mut events = IndexMutationEventCatalog::new();
        events
            .register(
                "product",
                "rustok-product.product-v1",
                "product-postgres-primary",
                schema.clone(),
            )
            .unwrap();

        let missing = IndexSourceCatalog::new();
        assert!(matches!(
            events.materialize(&missing),
            Err(IndexMutationEventError::UnknownReplaySource { .. })
        ));

        let mut wrong_owner = IndexSourceCatalog::new();
        wrong_owner
            .register(
                "distribution",
                "product-postgres-primary",
                [schema.clone()],
                NoopSource,
            )
            .unwrap();
        assert!(matches!(
            events.materialize(&wrong_owner),
            Err(IndexMutationEventError::ReplaySourceOwnerMismatch { .. })
        ));

        let mut wrong_schema = IndexSourceCatalog::new();
        wrong_schema
            .register(
                "product",
                "product-postgres-primary",
                [schema_ref(2)],
                NoopSource,
            )
            .unwrap();
        assert!(matches!(
            events.materialize(&wrong_schema),
            Err(IndexMutationEventError::ReplaySourceSchemaMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn durable_terminal_mutation_is_acknowledged_after_apply() {
        for mutation_outcome in [
            IndexReplayMutationOutcome::Applied,
            IndexReplayMutationOutcome::Duplicate,
            IndexReplayMutationOutcome::StaleIgnored,
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let worker = IndexMutationEventWorker::new(
                RecordingSink {
                    calls: calls.clone(),
                    result: Ok(mutation_outcome),
                },
                RecordingAcknowledger {
                    calls: calls.clone(),
                    result: Ok(()),
                },
            );
            let outcome = worker
                .process(
                    &SchemaRegistry::default(),
                    &event_registry(),
                    IndexMutationEventDelivery::new(
                        "rustok-product.product-v1",
                        mutation(schema_ref(1)),
                        "broker-position-7".to_owned(),
                    )
                    .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(outcome.mutation_outcome(), mutation_outcome);
            assert_eq!(*calls.lock().unwrap(), vec!["apply", "ack"]);
        }
    }

    #[tokio::test]
    async fn mutation_failure_suppresses_acknowledgement() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let worker = IndexMutationEventWorker::new(
            RecordingSink {
                calls: calls.clone(),
                result: Err(IndexReplayFailure::retryable("mutation_storage_retryable").unwrap()),
            },
            RecordingAcknowledger {
                calls: calls.clone(),
                result: Ok(()),
            },
        );
        let result = worker
            .process(
                &SchemaRegistry::default(),
                &event_registry(),
                IndexMutationEventDelivery::new(
                    "rustok-product.product-v1",
                    mutation(schema_ref(1)),
                    "broker-position-7".to_owned(),
                )
                .unwrap(),
            )
            .await;

        assert!(matches!(
            result,
            Err(IndexMutationEventProcessError::Mutation(_))
        ));
        assert_eq!(*calls.lock().unwrap(), vec!["apply"]);
    }

    #[tokio::test]
    async fn acknowledgement_failure_is_reported_after_durable_apply() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let worker = IndexMutationEventWorker::new(
            RecordingSink {
                calls: calls.clone(),
                result: Ok(IndexReplayMutationOutcome::Applied),
            },
            RecordingAcknowledger {
                calls: calls.clone(),
                result: Err(
                    IndexMutationAcknowledgeFailure::retryable("broker_ack_retryable").unwrap(),
                ),
            },
        );
        let result = worker
            .process(
                &SchemaRegistry::default(),
                &event_registry(),
                IndexMutationEventDelivery::new(
                    "rustok-product.product-v1",
                    mutation(schema_ref(1)),
                    "broker-position-7".to_owned(),
                )
                .unwrap(),
            )
            .await;

        assert!(matches!(
            result,
            Err(IndexMutationEventProcessError::Acknowledge(_))
        ));
        assert_eq!(*calls.lock().unwrap(), vec!["apply", "ack"]);
    }

    #[tokio::test]
    async fn unknown_domain_and_schema_mismatch_fail_before_apply_or_ack() {
        for delivery in [
            IndexMutationEventDelivery::new(
                "rustok-product.unknown-v1",
                mutation(schema_ref(1)),
                "broker-position-7".to_owned(),
            )
            .unwrap(),
            IndexMutationEventDelivery::new(
                "rustok-product.product-v1",
                mutation(schema_ref(2)),
                "broker-position-8".to_owned(),
            )
            .unwrap(),
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let worker = IndexMutationEventWorker::new(
                RecordingSink {
                    calls: calls.clone(),
                    result: Ok(IndexReplayMutationOutcome::Applied),
                },
                RecordingAcknowledger {
                    calls: calls.clone(),
                    result: Ok(()),
                },
            );
            assert!(
                worker
                    .process(&SchemaRegistry::default(), &event_registry(), delivery)
                    .await
                    .is_err()
            );
            assert!(calls.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn delivery_rejects_nil_identity_and_zero_version() {
        let schema = schema_ref(1);
        let make = |event_id, tenant_id, entity_id, source_version| IndexMutation::Delete {
            event_id,
            key: EntityKey {
                tenant_id,
                schema: schema.clone(),
                entity_id,
                locale: None,
            },
            source_version,
        };

        assert!(matches!(
            IndexMutationEventDelivery::new(
                "rustok-product.product-v1",
                make(Uuid::nil(), Uuid::from_u128(1), Uuid::from_u128(2), 1),
                (),
            ),
            Err(IndexMutationEventError::NilEventId)
        ));
        assert!(matches!(
            IndexMutationEventDelivery::new(
                "rustok-product.product-v1",
                make(Uuid::from_u128(4), Uuid::nil(), Uuid::from_u128(2), 1),
                (),
            ),
            Err(IndexMutationEventError::NilTenantId)
        ));
        assert!(matches!(
            IndexMutationEventDelivery::new(
                "rustok-product.product-v1",
                make(Uuid::from_u128(4), Uuid::from_u128(1), Uuid::nil(), 1),
                (),
            ),
            Err(IndexMutationEventError::NilEntityId)
        ));
        assert!(matches!(
            IndexMutationEventDelivery::new(
                "rustok-product.product-v1",
                make(
                    Uuid::from_u128(4),
                    Uuid::from_u128(1),
                    Uuid::from_u128(2),
                    0
                ),
                (),
            ),
            Err(IndexMutationEventError::ZeroSourceVersion)
        ));
    }
}
