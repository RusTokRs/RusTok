use std::sync::Arc;

use rustok_fulfillment::{
    FulfillmentAdminCommandPort, FulfillmentAdminCommandRuntime,
    FulfillmentAdminCreateCommandPort, FulfillmentAdminCreateCommandRuntime,
    providers::FulfillmentProviderRegistry,
};
use sea_orm::DatabaseConnection;

/// Host-selected Fulfillment owner commands consumed by mounted Commerce GraphQL mutations.
///
/// Commerce composes the existing lifecycle and manual-create command capabilities but does not
/// own Fulfillment persistence, provider journals, or provider execution. Hosts may inject either
/// owner runtime independently; missing capabilities use the Fulfillment-owned in-process adapters
/// with the deployment-selected provider registry.
#[derive(Clone)]
pub struct CommerceFulfillmentCommandRuntime {
    lifecycle_commands: FulfillmentAdminCommandRuntime,
    create_commands: FulfillmentAdminCreateCommandRuntime,
}

impl CommerceFulfillmentCommandRuntime {
    pub fn new(
        lifecycle_commands: FulfillmentAdminCommandRuntime,
        create_commands: FulfillmentAdminCreateCommandRuntime,
    ) -> Self {
        Self {
            lifecycle_commands,
            create_commands,
        }
    }

    pub fn in_process(
        db: DatabaseConnection,
        provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        Self::new(
            FulfillmentAdminCommandRuntime::in_process(
                db.clone(),
                provider_registry.clone(),
            ),
            FulfillmentAdminCreateCommandRuntime::in_process(db, provider_registry),
        )
    }

    pub(crate) fn from_graphql_inputs(
        inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
    ) -> Self {
        let provider_registry = inputs
            .shared_get::<FulfillmentProviderRegistry>()
            .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider);
        let lifecycle_commands = inputs
            .shared_get::<FulfillmentAdminCommandRuntime>()
            .unwrap_or_else(|| {
                FulfillmentAdminCommandRuntime::in_process(
                    inputs.db_clone(),
                    provider_registry.clone(),
                )
            });
        let create_commands = inputs
            .shared_get::<FulfillmentAdminCreateCommandRuntime>()
            .unwrap_or_else(|| {
                FulfillmentAdminCreateCommandRuntime::in_process(
                    inputs.db_clone(),
                    provider_registry.clone(),
                )
            });
        Self::new(lifecycle_commands, create_commands)
    }

    pub fn lifecycle_command_port(&self) -> Arc<dyn FulfillmentAdminCommandPort> {
        self.lifecycle_commands.command_port()
    }

    pub fn create_command_port(&self) -> Arc<dyn FulfillmentAdminCreateCommandPort> {
        self.create_commands.command_port()
    }
}
