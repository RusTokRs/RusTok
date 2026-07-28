use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo,
};
use async_graphql::{Context, ServerResult, Value};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{
    ShippingOptionAdminReadPort, ShippingOptionReadPort,
    in_process_shipping_option_admin_read_port, in_process_shipping_option_read_port,
};
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;

/// Host-selected shipping-option read ports used by mounted commerce GraphQL resolvers.
///
/// The application host composes this runtime once and carries it through
/// `HostRuntimeContext`. Directly embedded schemas that do not install the mounted
/// extension retain an explicit in-process compatibility fallback outside the query facade.
#[derive(Clone)]
pub struct CommerceShippingOptionReadRuntime {
    shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
    shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
}

impl CommerceShippingOptionReadRuntime {
    pub fn new(
        shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
        shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>,
    ) -> Self {
        Self {
            shipping_option_reads,
            shipping_option_admin_reads,
        }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(
            in_process_shipping_option_read_port(db.clone()),
            in_process_shipping_option_admin_read_port(db),
        )
    }

    pub fn shipping_option_read_port(&self) -> Arc<dyn ShippingOptionReadPort> {
        self.shipping_option_reads.clone()
    }

    pub fn shipping_option_admin_read_port(&self) -> Arc<dyn ShippingOptionAdminReadPort> {
        self.shipping_option_admin_reads.clone()
    }
}

tokio::task_local! {
    static CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME: CommerceShippingOptionReadRuntime;
}

/// Resolver-scoped bridge from schema runtime data to the private compatibility facade.
#[derive(Default)]
pub struct CommerceShippingOptionReadScope;

impl ExtensionFactory for CommerceShippingOptionReadScope {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(CommerceShippingOptionReadScopeExtension)
    }
}

struct CommerceShippingOptionReadScopeExtension;

#[async_trait::async_trait]
impl Extension for CommerceShippingOptionReadScopeExtension {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        let Some(runtime_data) = ctx.data_opt::<CommerceGraphqlRuntimeData>() else {
            return next.run(ctx, info).await;
        };
        CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME
            .scope(
                runtime_data.shipping_option_read_runtime(),
                next.run(ctx, info),
            )
            .await
    }
}

pub(crate) fn shipping_option_read_runtime_for_current_graphql_scope(
    db: DatabaseConnection,
) -> CommerceShippingOptionReadRuntime {
    CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME
        .try_with(Clone::clone)
        .unwrap_or_else(|_| CommerceShippingOptionReadRuntime::in_process(db))
}

/// Provider registries and host-composed ports available to every commerce GraphQL resolver.
///
/// Hosts supply composed capabilities through `HostRuntimeContext`. The built-in manual
/// provider registries remain deterministic fallbacks for tests and deployments that have not
/// installed external providers. Mounted shipping-option reads are required host data.
#[derive(Clone)]
pub struct CommerceGraphqlRuntimeData {
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
    marketplace_financial_runtime: crate::MarketplaceFinancialRuntime,
    shipping_option_read_runtime: CommerceShippingOptionReadRuntime,
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

    pub fn shipping_option_read_runtime(&self) -> CommerceShippingOptionReadRuntime {
        self.shipping_option_read_runtime.clone()
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
        shipping_option_read_runtime: inputs
            .shared_get::<CommerceShippingOptionReadRuntime>()
            .ok_or_else(|| {
                "commerce GraphQL requires CommerceShippingOptionReadRuntime in host composition"
                    .to_string()
            })?,
    })
}

pub(crate) fn payment_provider_registry_from_context(ctx: &Context<'_>) -> PaymentProviderRegistry {
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
