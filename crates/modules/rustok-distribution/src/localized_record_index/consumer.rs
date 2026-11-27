use async_trait::async_trait;
use rustok_core::{
    Error, ModuleEventListenerContext,
    events::{EventHandler, HandlerResult},
};
use rustok_events::{ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope};
use rustok_index::{
    IndexSchema, MutationDelivery, PostgresMutationStore, PostgresSchemaRegistrationStore,
    SharedIndexSchemaRegistry,
};

use super::source::{Factory, Source};

pub(super) struct Consumer {
    module_slug: &'static str,
    entity_type: &'static str,
    name: String,
    source: Source,
    schema: Option<IndexSchema>,
    schemas: Option<SharedIndexSchemaRegistry>,
    registration: PostgresSchemaRegistrationStore,
    mutations: PostgresMutationStore,
}

impl Consumer {
    pub(super) fn new(factory: &Factory, context: &ModuleEventListenerContext<'_>) -> Self {
        let schemas = context
            .extensions
            .get::<SharedIndexSchemaRegistry>()
            .cloned();
        let schema = schemas
            .as_ref()
            .and_then(|schemas| schemas.registry().get(&factory.reference))
            .map(|registered| registered.schema.clone());
        Self {
            module_slug: factory.owner.module_slug,
            entity_type: factory.owner.entity_type,
            name: factory.name.clone(),
            source: factory.source(context.db.clone()),
            schema,
            schemas,
            registration: PostgresSchemaRegistrationStore::new(context.db.clone()),
            mutations: PostgresMutationStore::new(context.db.clone()),
        }
    }
}

#[async_trait]
impl EventHandler for Consumer {
    fn name(&self) -> &'static str {
        "localized_record_index"
    }

    fn handles(&self, _: &DomainEvent) -> bool {
        false
    }

    async fn handle(&self, _: &EventEnvelope) -> HandlerResult {
        Err(refused("expected a sealed refresh event"))
    }

    fn handles_contract(&self, payload: &ContractEventPayload) -> bool {
        matches!(payload, ContractEventPayload::LocalizedIndexRefresh(event)
            if event.module_slug == self.module_slug && event.entity_type == self.entity_type)
    }

    async fn handle_contract(&self, envelope: &ContractEventEnvelope) -> HandlerResult {
        let ContractEventPayload::LocalizedIndexRefresh(event) =
            envelope.payload().map_err(refused)?
        else {
            return Err(refused("expected a localized refresh event"));
        };
        if event.module_slug != self.module_slug || event.entity_type != self.entity_type {
            return Err(refused("event does not target this owner"));
        }
        let (Some(schemas), Some(schema)) = (&self.schemas, &self.schema) else {
            return Err(refused("owner schema is unavailable"));
        };
        let schemas = schemas.registry();
        let mutations = self
            .source
            .refresh(envelope.tenant_id(), event.record_id, event.source_version)
            .await
            .map_err(refused)?;
        let deliveries = mutations
            .into_iter()
            .map(|mutation| {
                schemas.validate_mutation(&mutation).map_err(refused)?;
                MutationDelivery::from_event(&self.name, mutation).map_err(refused)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.registration
            .register(envelope.tenant_id(), schema)
            .await
            .map_err(refused)?;
        for delivery in deliveries {
            self.mutations
                .apply(schemas, &delivery)
                .await
                .map_err(refused)?;
        }
        Ok(())
    }
}

fn refused(error: impl std::fmt::Display) -> Error {
    Error::External(format!("localized record Index refresh failed: {error}"))
}
