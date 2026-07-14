pub mod admin;
#[path = "admin/checkout_operations.rs"]
mod checkout_operations;
mod common;
pub mod products;
mod reconciliation;
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
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            payment_provider_registry: runtime
                .shared_get::<PaymentProviderRegistry>()
                .unwrap_or_else(PaymentProviderRegistry::with_manual_provider),
            fulfillment_provider_registry: runtime
                .shared_get::<FulfillmentProviderRegistry>()
                .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider),
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
            "/admin/fulfillment-provider-operations",
            reconciliation::axum_router(),
        )
        .with_state(state))
}
