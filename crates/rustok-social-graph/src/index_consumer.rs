use std::sync::Arc;

use rustok_events::{ContractEventEnvelope, ContractEventEnvelopeError, ContractEventPayload};
use rustok_iggy::{IggyTransport, PersistentContractConsumerGroup};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialGraphIndexProcessOutcome {
    Projected(MutationApplyOutcome),
    IgnoredUnrelated { event_type: String },
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

/// Result-first durable consumer for `social_graph.relation.state_changed`.
///
/// The consumer owns one persistent broker cursor, one validated source schema,
/// and Index-owned schema/mutation stores. It acknowledges the exact broker message
/// only after the tenant schema exists and `PostgresMutationStore` commits an
/// applied result or terminally recognizes duplicate/stale delivery. A failed
/// registration, apply, or acknowledgement leaves the message replayable.
pub struct SocialGraphIndexConsumer {
    _transport: Arc<IggyTransport>,
    group: PersistentContractConsumerGroup,
    schema: IndexSchema,
    schema_store: PostgresSchemaRegistrationStore,
    store: PostgresMutationStore,
    registry: SchemaRegistry,
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
        let schema = social_graph_relation_index_schema()?;
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone())?;
        Ok(Self {
            _transport: transport,
            group,
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

    /// Receives, durably registers/applies/recognizes, and then acknowledges one message.
    ///
    /// `&mut self` deliberately serializes receive/apply/ack on this cursor, preventing
    /// another delivery from overtaking an outstanding unacknowledged message.
    pub async fn process_next(
        &mut self,
    ) -> Result<Option<SocialGraphIndexProcessOutcome>, SocialGraphIndexConsumerError> {
        let Some(consumed) = self
            .group
            .receive()
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?
        else {
            return Ok(None);
        };
        let outcome = self.apply_envelope(&consumed.envelope).await?;
        self.group
            .acknowledge(&consumed)
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?;
        Ok(Some(outcome))
    }
}

#[cfg(test)]
mod tests {
    use rustok_events::{ContractEventEnvelope, DomainEvent, SocialGraphRelationEvent};
    use rustok_index::IndexMutation;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn relation_envelope_becomes_stable_index_delivery() {
        let tenant_id = Uuid::from_u128(1);
        let envelope = ContractEventEnvelope::new(
            tenant_id,
            None,
            SocialGraphRelationEvent::RelationStateChanged {
                relation_id: Uuid::from_u128(2),
                source_user_id: Uuid::from_u128(3),
                target_user_id: Uuid::from_u128(4),
                relation_kind: "follow".to_string(),
                active: true,
                revision: 9,
            },
        )
        .unwrap();

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
}
