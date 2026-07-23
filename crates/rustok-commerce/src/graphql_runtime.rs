use async_graphql::Context;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;

/// Provider registries available to every commerce GraphQL resolver.
///
/// Hosts may supply composed registries through `HostRuntimeContext`. The built-in
/// manual adapters remain a deterministic fallback for tests and deployments that
/// have not installed external providers.
#[derive(Clone)]
pub struct CommerceGraphqlRuntimeData {
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
    marketplace_financial_runtime: crate::MarketplaceFinancialRuntime,
}

impl CommerceGraphqlRuntimeData {
    pub fn payment_provider_registry(&self) -> PaymentProviderRegistry {
        self.payment_provider_registry.clone()
    }

    pub fn fulfillment_provider_registry(&self) -> FulfillmentProviderRegistry {
        self.fulfillment_provider_registry.clone()
    }

    pub fn marketplace_financial_runtime(&self) -> crate::MarketplaceFinancialRuntime {
        self.marketplace_financial_runtime.clone()
    }
}

/// Capability-owned factory consumed by manifest-generated schema composition.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<CommerceGraphqlRuntimeData, String> {
    Ok(CommerceGraphqlRuntimeData {
        payment_provider_registry: inputs
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
        fulfillment_provider_registry: inputs
            .shared_get::<FulfillmentProviderRegistry>()
            .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider),
        marketplace_financial_runtime: inputs
            .shared_get::<crate::MarketplaceFinancialRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires MarketplaceFinancialRuntime in host composition"
                    .to_string()
            })?,
    })
}

pub(crate) fn payment_provider_registry_from_context(
    ctx: &Context<'_>,
) -> PaymentProviderRegistry {
    ctx.data_opt::<CommerceGraphqlRuntimeData>()
        .map(CommerceGraphqlRuntimeData::payment_provider_registry)
        .unwrap_or_else(PaymentProviderRegistry::with_manual_provider)
}

pub(crate) fn payment_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> crate::PaymentOrchestrationService {
    crate::PaymentOrchestrationService::new(db)
        .with_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn refund_reconciliation_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> crate::RefundReconciliationService {
    crate::RefundReconciliationService::new(db)
        .with_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn fulfillment_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
) -> crate::FulfillmentOrchestrationService {
    let service = crate::FulfillmentOrchestrationService::new(db);
    match ctx.data_opt::<CommerceGraphqlRuntimeData>() {
        Some(runtime) => service.with_provider_registry(runtime.fulfillment_provider_registry()),
        None => service,
    }
}

pub(crate) fn post_order_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::PostOrderOrchestrationService {
    crate::PostOrderOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn order_change_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::OrderChangeOrchestrationService {
    crate::OrderChangeOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}

pub(crate) fn return_completion_orchestration_from_context(
    ctx: &Context<'_>,
    db: DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> crate::ReturnCompletionOrchestrationService {
    crate::ReturnCompletionOrchestrationService::new(db, event_bus)
        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))
}
