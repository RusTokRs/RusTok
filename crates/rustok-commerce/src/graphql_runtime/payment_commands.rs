use std::sync::Arc;

use rustok_payment::{
    PaymentAdminCollectionCommandPort, PaymentAdminCollectionCommandRuntime,
    PaymentAdminRefundCommandPort, PaymentAdminRefundCommandRuntime,
    providers::PaymentProviderRegistry,
};
use sea_orm::DatabaseConnection;

/// Host-selected Payment owner commands consumed by mounted Commerce GraphQL mutations.
///
/// Commerce composes the existing collection and refund command capabilities but does not own
/// Payment persistence, provider journals, or provider execution. Hosts may inject either owner
/// runtime independently; missing capabilities use the Payment-owned in-process adapters with the
/// deployment-selected provider registry.
#[derive(Clone)]
pub struct CommercePaymentCommandRuntime {
    collection_commands: PaymentAdminCollectionCommandRuntime,
    refund_commands: PaymentAdminRefundCommandRuntime,
}

impl CommercePaymentCommandRuntime {
    pub fn new(
        collection_commands: PaymentAdminCollectionCommandRuntime,
        refund_commands: PaymentAdminRefundCommandRuntime,
    ) -> Self {
        Self {
            collection_commands,
            refund_commands,
        }
    }

    pub fn in_process(
        db: DatabaseConnection,
        provider_registry: PaymentProviderRegistry,
    ) -> Self {
        Self::new(
            PaymentAdminCollectionCommandRuntime::in_process(
                db.clone(),
                provider_registry.clone(),
            ),
            PaymentAdminRefundCommandRuntime::in_process(db, provider_registry),
        )
    }

    pub(crate) fn from_graphql_inputs(
        inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
    ) -> Self {
        let provider_registry = inputs
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider);
        let collection_commands = inputs
            .shared_get::<PaymentAdminCollectionCommandRuntime>()
            .unwrap_or_else(|| {
                PaymentAdminCollectionCommandRuntime::in_process(
                    inputs.db_clone(),
                    provider_registry.clone(),
                )
            });
        let refund_commands = inputs
            .shared_get::<PaymentAdminRefundCommandRuntime>()
            .unwrap_or_else(|| {
                PaymentAdminRefundCommandRuntime::in_process(
                    inputs.db_clone(),
                    provider_registry.clone(),
                )
            });
        Self::new(collection_commands, refund_commands)
    }

    pub fn collection_command_port(&self) -> Arc<dyn PaymentAdminCollectionCommandPort> {
        self.collection_commands.command_port()
    }

    pub fn refund_command_port(&self) -> Arc<dyn PaymentAdminRefundCommandPort> {
        self.refund_commands.command_port()
    }
}
