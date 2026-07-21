pub mod admin;
#[path = "admin/checkout_operations.rs"]
pub(crate) mod checkout_operations;
mod common;
mod marketplace_financial;
pub mod products;
mod reconciliation;
pub(crate) mod return_completion_operations;
pub mod store;

use rustok_api::HostRuntimeContext;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct CommerceHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
    marketplace_financial_runtime: crate::MarketplaceFinancialRuntime,
}

impl CommerceHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn payment_provider_registry(&self) -> PaymentProviderRegistry {
        self.payment_provider_registry.clone()
    }

    fn fulfillment_provider_registry(&self) -> FulfillmentProviderRegistry {
        self.fulfillment_provider_registry.clone()
    }

    fn marketplace_financial_operator_service(&self) -> crate::MarketplaceFinancialOperatorService {
        self.marketplace_financial_runtime
            .operator_service(self.db_clone(), self.event_bus())
    }

    fn marketplace_paid_event_inbox_service(&self) -> crate::MarketplacePaidEventInboxService {
        self.marketplace_financial_runtime
            .paid_event_inbox(self.db_clone(), self.event_bus())
    }
}

impl CommerceHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require TransactionalEventBus in HostRuntimeContext"
                )
            })?;
        let marketplace_financial_runtime = runtime
            .shared_get::<crate::MarketplaceFinancialRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require MarketplaceFinancialRuntime in HostRuntimeContext"
                )
            })?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            payment_provider_registry: runtime
                .shared_get::<PaymentProviderRegistry>()
                .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
            fulfillment_provider_registry: runtime
                .shared_get::<FulfillmentProviderRegistry>()
                .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider),
            marketplace_financial_runtime,
        })
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = CommerceHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .nest("/store", store::axum_router())
        .nest("/admin", admin::axum_router())
        .nest(
            "/admin/checkout-operations",
            checkout_operations::axum_router(),
        )
        .nest(
            "/admin/return-completion-operations",
            return_completion_operations::axum_router(),
        )
        .nest(
            "/admin/fulfillment-provider-operations",
            reconciliation::axum_router(),
        )
        .nest(
            "/admin/marketplace-financial",
            marketplace_financial::axum_router(),
        )
        .with_state(state))
}
