use std::sync::Arc;

use rustok_events::{ContractEventEnvelope, ContractEventEnvelopeError, ContractEventPayload};
use rustok_iggy::{ConsumedContractEvent, DlqEntry, IggyTransport, PersistentContractConsumerGroup};
use rustok_index::{
    IndexSchema, MutationApplyOutcome, MutationDelivery, MutationStorageError,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SchemaRegistrationError,
    SchemaRegistry, SchemaRegistryError,
};
use sea_orm::DatabaseConnection;
use thiserror::Error;

use crate::index::{
    SocialGraphIndexError, social_graph_relation_index_mutation,
    social_graph_relation_index_schema,
};

/// The existing shared domain topic currently carries sealed Social Graph relation facts.
pub const SOCIAL_GRAPH_INDEX_TOPIC: &str = "domain";
/// Dedicated durable cursor identity for the Social Graph relation projection.
pub const SOCIAL_GRAPH_INDEX_CONSUMER_GROUP: &str = "rustok-social-graph-index";
/// Stable Index inbox source identity for relation-event deliveries.
pub const SOCIAL_GRAPH_INDEX_SOURCE: &str = "social_graph.relation.state_changed.v1";

#[derive(Debug, Error)]
pub enum SocialGraphIndexConsumerError {
    #[error("Social Graph Index transport operation failed: {0}")]
    Transport(String),
    #[error(transparent)]
    Envelope(#[from] ContractEventEnvelopeError),
    #[error(transparent)]
    Projection(#[from] SocialGraphIndexError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error(transparent)]
    SchemaPersistence(#[from] SchemaRegistrationError),
    #[error(transparent)]
    Storage(#[from] MutationStorageError),
}

impl SocialGraphIndexConsumerError {
    /// Stable bounded code suitable for retry/DLQ telemetry without storage details.
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::Transport(_) => "social_graph.index.transport_unavailable",
            Self::Envelope(_) => "social_graph.index.envelope_invalid",
            Self::Projection(_) => "social_graph.index.projection_invalid",
            Self::Registry(_) => "social_graph.index.registry_invalid",
            Self::SchemaPersistence(_) => "social_graph.index.schema_persistence_failed",
            Self::Storage(_) => "social_graph.index.mutation_persistence_failed",
        }
    }

    /// Only transient transport/storage ownership failures are retried in-process.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(_) => true,
            Self::Envelope(_) | Self::Projection(_) | Self::Registry(_) => false,
            Self::SchemaPersistence(error) => {
                matches!(error, SchemaRegistrationError::Storage(_))
            }
            Self::Storage(error) => matches!(
                error,
                MutationStorageError::DeliveryInProgress { .. }
                    | MutationStorageError::Storage(_)
                    | MutationStorageError::ConcurrentMutationConflict
                    | MutationStorageError::InboxCompletionLost
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialGraphIndexProcessOutcome {
    Projected(MutationApplyOutcome),
    IgnoredUnrelated { event_type: String },
    DeadLettered { error_code: &'static str },
}

/// Converts a validated sealed envelope into one durable Index inbox delivery.
///
/// Events from other sealed families are intentionally ignored by this dedicated
/// consumer group. Relevant relation facts retain the envelope event ID as the
/// Index delivery identity and the relation revision as the mutation source version.
pub fn social_graph_index_delivery_from_envelope(
    envelope: &ContractEventEnvelope,
) -> Result<Option<MutationDelivery>, SocialGraphIndexConsumerError> {
    let payload = envelope.payload()?;
    let ContractEventPayload::SocialGraphRelation(event) = payload else {
        return Ok(None);
    };
    let mutation = social_graph_relation_index_mutation(
        envelope.tenant_id(),
        envelope.id(),
        event.clone(),
    )?;
    Ok(Some(MutationDelivery::from_event(
        SOCIAL_GRAPH_INDEX_SOURCE,
        mutation,
    )?))
}

/// Transport-neutral durable projector used by live and replay-oriented consumers.
///
/// It persists or exactly recognizes the tenant schema through the Index owner,
/// then applies or terminally recognizes one mutation through the Index inbox.
/// It never acknowledges transport messages and never reads Social Graph storage.
pub struct SocialGraphIndexProjector {
    schema: IndexSchema,
    schema_store: PostgresSchemaRegistrationStore,
    store: PostgresMutationStore,
    registry: SchemaRegistry,
}

impl SocialGraphIndexProjector {
    pub fn new(db: DatabaseConnection) -> Result<Self, SocialGraphIndexConsumerError> {
        let schema = social_graph_relation_index_schema()?;
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone())?;
        Ok(Self {
            schema,
            schema_store: PostgresSchemaRegistrationStore::new(db.clone()),
            store: PostgresMutationStore::new(db),
            registry,
        })
    }

    pub fn registry(&self) -> &SchemaRegistry {
        &self.registry
    }

    pub async fn apply_envelope(
        &self,
        envelope: &ContractEventEnvelope,
    ) -> Result<SocialGraphIndexProcessOutcome, SocialGraphIndexConsumerError> {
        let Some(delivery) = social_graph_index_delivery_from_envelope(envelope)? else {
            return Ok(SocialGraphIndexProcessOutcome::IgnoredUnrelated {
                event_type: envelope.event_type().to_string(),
            });
        };
        self.schema_store
            .register(envelope.tenant_id(), &self.schema)
            .await?;
        let outcome = self.store.apply(&self.registry, &delivery).await?;
        Ok(SocialGraphIndexProcessOutcome::Projected(outcome))
    }
}

/// Result-first durable consumer for `social_graph.relation.state_changed`.
///
/// The consumer owns one persistent broker cursor and one transport-neutral
/// projector. Host runtimes may retain a received delivery across bounded retries,
/// but must acknowledge only after projection or successful DLQ publication.
pub struct SocialGraphIndexConsumer {
    transport: Arc<IggyTransport>,
    group: PersistentContractConsumerGroup,
    projector: SocialGraphIndexProjector,
}

impl SocialGraphIndexConsumer {
    pub async fn open(
        transport: Arc<IggyTransport>,
        db: DatabaseConnection,
    ) -> Result<Self, SocialGraphIndexConsumerError> {
        let group = transport
            .open_persistent_contract_consumer_group(
                SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
                SOCIAL_GRAPH_INDEX_TOPIC,
            )
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?;
        Ok(Self {
            transport,
            group,
            projector: SocialGraphIndexProjector::new(db)?,
        })
    }

    pub fn projector(&self) -> &SocialGraphIndexProjector {
        &self.projector
    }

    /// Receives one validated broker delivery without committing its offset.
    pub async fn receive_next(
        &mut self,
    ) -> Result<Option<ConsumedContractEvent>, SocialGraphIndexConsumerError> {
        self.group
            .receive()
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))
    }

    /// Persists or terminally recognizes the owner result without acknowledging.
    pub async fn project_consumed(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<SocialGraphIndexProcessOutcome, SocialGraphIndexConsumerError> {
        self.projector.apply_envelope(&consumed.envelope).await
    }

    /// Commits the exact broker offset after a durable result exists.
    pub async fn acknowledge_consumed(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        self.group
            .acknowledge(consumed)
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))
    }

    /// Publishes the exact original broker bytes to DLQ without committing the source offset.
    ///
    /// Hosts use this staged operation to distinguish DLQ publication failures from source
    /// acknowledgement failures and to retry acknowledgement without republishing in-process.
    pub async fn publish_consumed_to_dlq(
        &self,
        consumed: &ConsumedContractEvent,
        stable_error_code: &'static str,
        retry_count: u32,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        let entry = DlqEntry::new(
            consumed.envelope.id(),
            consumed.topic.clone(),
            consumed.raw_payload().to_vec(),
            stable_error_code,
            retry_count,
        )
        .with_connector_metadata(consumed.connector_metadata.clone());
        self.transport
            .move_to_dlq(entry)
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))
    }

    /// Publishes the exact original broker bytes to DLQ, then commits the source offset.
    ///
    /// This convenience method is valid only for a delivery that has not produced a durable
    /// Index result. Hosts requiring retry metrics should use [`Self::publish_consumed_to_dlq`]
    /// followed by [`Self::acknowledge_consumed`] so acknowledgement can be retried without
    /// republishing the DLQ entry in the same process.
    pub async fn move_to_dlq_and_acknowledge(
        &self,
        consumed: &ConsumedContractEvent,
        stable_error_code: &'static str,
        retry_count: u32,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)
            .await?;
        self.acknowledge_consumed(consumed).await
    }

    /// Receives, durably registers/applies/recognizes, and then acknowledges one message.
    ///
    /// `&mut self` deliberately serializes receive/apply/ack on this cursor, preventing
    /// another delivery from overtaking an outstanding unacknowledged message.
    pub async fn process_next(
        &mut self,
    ) -> Result<Option<SocialGraphIndexProcessOutcome>, SocialGraphIndexConsumerError> {
        let Some(consumed) = self.receive_next().await? else {
            return Ok(None);
        };
        let outcome = self.project_consumed(&consumed).await?;
        self.acknowledge_consumed(&consumed).await?;
        Ok(Some(outcome))
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::MigrationSource;
    use rustok_events::{ContractEventEnvelope, DomainEvent, SocialGraphRelationEvent};
    use rustok_index::{IndexModule, IndexMutation};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use super::*;

    fn relation_event(
        tenant_id: Uuid,
        relation_id: Uuid,
        active: bool,
        revision: i64,
    ) -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            tenant_id,
            None,
            SocialGraphRelationEvent::RelationStateChanged {
                relation_id,
                source_user_id: Uuid::from_u128(3),
                target_user_id: Uuid::from_u128(4),
                relation_kind: "follow".to_string(),
                active,
                revision,
            },
        )
        .unwrap()
    }

    async fn projector_fixture() -> (DatabaseConnection, SocialGraphIndexProjector, Uuid) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
            .await
            .unwrap();
        let tenant_id = Uuid::from_u128(1);
        db.execute_unprepared(&format!(
            "INSERT INTO tenants (id) VALUES ('{tenant_id}')"
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await.unwrap();
        }
        let projector = SocialGraphIndexProjector::new(db.clone()).unwrap();
        (db, projector, tenant_id)
    }

    async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
        db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "value")
            .unwrap()
    }

    #[test]
    fn relation_envelope_becomes_stable_index_delivery() {
        let tenant_id = Uuid::from_u128(1);
        let envelope = relation_event(tenant_id, Uuid::from_u128(2), true, 9);
        let delivery = social_graph_index_delivery_from_envelope(&envelope)
            .unwrap()
            .expect("relation event must produce an Index delivery");
        assert_eq!(delivery.source_name(), SOCIAL_GRAPH_INDEX_SOURCE);
        assert_eq!(delivery.delivery_id(), envelope.id().to_string());
        let IndexMutation::Upsert { record, .. } = delivery.mutation() else {
            panic!("active relation must produce an upsert");
        };
        assert_eq!(record.key.tenant_id, tenant_id);
        assert_eq!(record.key.entity_id, Uuid::from_u128(2));
        assert_eq!(record.source_version, 9);
    }

    #[test]
    fn unrelated_domain_event_is_ignored_without_projection() {
        let envelope = ContractEventEnvelope::new(
            Uuid::from_u128(1),
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::from_u128(2),
                kind: "post".to_string(),
                author_id: None,
            },
        )
        .unwrap();
        assert!(
            social_graph_index_delivery_from_envelope(&envelope)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn retry_classification_is_fail_closed() {
        assert!(SocialGraphIndexConsumerError::Transport("down".to_string()).is_retryable());
        assert!(SocialGraphIndexConsumerError::SchemaPersistence(
            SchemaRegistrationError::Storage("down".to_string())
        )
        .is_retryable());
        assert!(!SocialGraphIndexConsumerError::SchemaPersistence(
            SchemaRegistrationError::NilTenantId
        )
        .is_retryable());
        assert!(!SocialGraphIndexConsumerError::Storage(
            MutationStorageError::DeliveryConflict
        )
        .is_retryable());
    }

    #[tokio::test]
    async fn projector_persists_schema_before_result_first_mutation_apply() {
        let (db, projector, tenant_id) = projector_fixture().await;
        let relation_id = Uuid::from_u128(2);
        let first = relation_event(tenant_id, relation_id, true, 1);
        assert_eq!(
            projector.apply_envelope(&first).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Applied {
                source_version: 1,
            })
        );
        assert_eq!(
            projector.apply_envelope(&first).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Duplicate {
                source_version: 1,
            })
        );
        let second = relation_event(tenant_id, relation_id, false, 2);
        assert_eq!(
            projector.apply_envelope(&second).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Applied {
                source_version: 2,
            })
        );
        assert_eq!(
            scalar_i64(&db, "SELECT COUNT(*) AS value FROM index_schemas").await,
            1
        );
        assert_eq!(
            scalar_i64(
                &db,
                "SELECT COUNT(*) AS value FROM index_entities WHERE is_deleted = TRUE AND source_version = 2"
            )
            .await,
            1
        );
    }
}
