pub mod admin;
#[path = "admin/checkout_operations.rs"]
pub(crate) mod checkout_operations;
mod common;
#[cfg(feature = "marketplace-financial")]
pub(crate) mod marketplace_financial;
#[cfg(feature = "marketplace-financial")]
pub(crate) mod marketplace_reversal_financial;
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
    shipping_option_read_runtime: crate::graphql_runtime::CommerceShippingOptionReadRuntime,
    shipping_option_admin_command_runtime: rustok_fulfillment::ShippingOptionAdminCommandRuntime,
    fulfillment_lifecycle_read_runtime:
        crate::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime,
    fulfillment_admin_command_runtime: rustok_fulfillment::FulfillmentAdminCommandRuntime,
    fulfillment_admin_create_command_runtime:
        rustok_fulfillment::FulfillmentAdminCreateCommandRuntime,
    order_read_runtime: crate::graphql_runtime::CommerceOrderReadRuntime,
    order_admin_command_runtime: rustok_order::OrderAdminCommandRuntime,
    payment_order_read_runtime: rustok_payment::PaymentOrderReadRuntime,
    payment_admin_read_runtime: rustok_payment::PaymentAdminReadRuntime,
    payment_admin_collection_command_runtime: rustok_payment::PaymentAdminCollectionCommandRuntime,
    payment_admin_refund_command_runtime: rustok_payment::PaymentAdminRefundCommandRuntime,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
    product_catalog_command_runtime: rustok_product::ProductCatalogCommandRuntime,
    #[cfg(feature = "marketplace-financial")]
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

    fn shipping_option_read_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_fulfillment::ShippingOptionReadPort> {
        self.shipping_option_read_runtime
            .shipping_option_read_port()
    }

    fn shipping_option_admin_read_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_fulfillment::ShippingOptionAdminReadPort> {
        self.shipping_option_read_runtime
            .shipping_option_admin_read_port()
    }

    fn shipping_option_admin_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_fulfillment::ShippingOptionAdminCommandPort> {
        self.shipping_option_admin_command_runtime.command_port()
    }

    fn fulfillment_read_port(&self) -> std::sync::Arc<dyn rustok_fulfillment::FulfillmentReadPort> {
        self.fulfillment_lifecycle_read_runtime
            .fulfillment_read_port()
    }

    fn fulfillment_admin_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_fulfillment::FulfillmentAdminCommandPort> {
        self.fulfillment_admin_command_runtime.command_port()
    }

    fn fulfillment_admin_create_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_fulfillment::FulfillmentAdminCreateCommandPort> {
        self.fulfillment_admin_create_command_runtime.command_port()
    }

    fn order_read_port(&self) -> std::sync::Arc<dyn rustok_order::OrderReadPort> {
        self.order_read_runtime.order_read_port()
    }

    fn order_admin_command_port(&self) -> std::sync::Arc<dyn rustok_order::OrderAdminCommandPort> {
        self.order_admin_command_runtime.command_port()
    }

    fn payment_order_read_port(&self) -> std::sync::Arc<dyn rustok_payment::PaymentOrderReadPort> {
        self.payment_order_read_runtime.read_port()
    }

    fn payment_admin_read_port(&self) -> std::sync::Arc<dyn rustok_payment::PaymentAdminReadPort> {
        self.payment_admin_read_runtime.read_port()
    }

    fn payment_admin_collection_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_payment::PaymentAdminCollectionCommandPort> {
        self.payment_admin_collection_command_runtime.command_port()
    }

    fn payment_admin_refund_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_payment::PaymentAdminRefundCommandPort> {
        self.payment_admin_refund_command_runtime.command_port()
    }

    fn product_catalog_read_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_product::ProductCatalogReadPort> {
        self.product_catalog_read_runtime.read_port()
    }

    fn product_catalog_command_port(
        &self,
    ) -> std::sync::Arc<dyn rustok_product::ProductCatalogCommandPort> {
        self.product_catalog_command_runtime.command_port()
    }

    #[cfg(feature = "marketplace-financial")]
    fn marketplace_financial_operator_service(&self) -> crate::MarketplaceFinancialOperatorService {
        self.marketplace_financial_runtime
            .operator_service(self.db_clone(), self.event_bus())
    }

    #[cfg(feature = "marketplace-financial")]
    fn marketplace_reversal_operator_service(&self) -> crate::MarketplaceReversalOperatorService {
        self.marketplace_financial_runtime
            .reversal_operator_service(self.db_clone())
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
        let payment_provider_registry = runtime
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider);
        let fulfillment_provider_registry = runtime
            .shared_get::<FulfillmentProviderRegistry>()
            .unwrap_or_else(FulfillmentProviderRegistry::with_manual_provider);
        let shipping_option_read_runtime = runtime
            .shared_get::<crate::graphql_runtime::CommerceShippingOptionReadRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require CommerceShippingOptionReadRuntime in HostRuntimeContext"
                )
            })?;
        let shipping_option_admin_command_runtime = runtime
            .shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()
            .unwrap_or_else(|| {
                rustok_fulfillment::ShippingOptionAdminCommandRuntime::in_process(
                    runtime.db_clone(),
                )
            });
        let fulfillment_lifecycle_read_runtime = runtime
            .shared_get::<crate::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require CommerceFulfillmentLifecycleReadRuntime in HostRuntimeContext"
                )
            })?;
        let fulfillment_admin_command_runtime = runtime
            .shared_get::<rustok_fulfillment::FulfillmentAdminCommandRuntime>()
            .unwrap_or_else(|| {
                rustok_fulfillment::FulfillmentAdminCommandRuntime::in_process(
                    runtime.db_clone(),
                    fulfillment_provider_registry.clone(),
                )
            });
        let fulfillment_admin_create_command_runtime = runtime
            .shared_get::<rustok_fulfillment::FulfillmentAdminCreateCommandRuntime>()
            .unwrap_or_else(|| {
                rustok_fulfillment::FulfillmentAdminCreateCommandRuntime::in_process(
                    runtime.db_clone(),
                    fulfillment_provider_registry.clone(),
                )
            });
        let order_read_runtime = runtime
            .shared_get::<crate::graphql_runtime::CommerceOrderReadRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require CommerceOrderReadRuntime in HostRuntimeContext"
                )
            })?;
        let order_admin_command_runtime = runtime
            .shared_get::<rustok_order::OrderAdminCommandRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require OrderAdminCommandRuntime in HostRuntimeContext"
                )
            })?;
        let payment_order_read_runtime = runtime
            .shared_get::<rustok_payment::PaymentOrderReadRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require PaymentOrderReadRuntime in HostRuntimeContext"
                )
            })?;
        let payment_admin_read_runtime = runtime
            .shared_get::<rustok_payment::PaymentAdminReadRuntime>()
            .unwrap_or_else(|| rustok_payment::PaymentAdminReadRuntime::in_process(runtime.db_clone()));
        let payment_admin_collection_command_runtime = runtime
            .shared_get::<rustok_payment::PaymentAdminCollectionCommandRuntime>()
            .unwrap_or_else(|| {
                rustok_payment::PaymentAdminCollectionCommandRuntime::in_process(
                    runtime.db_clone(),
                    payment_provider_registry.clone(),
                )
            });
        let payment_admin_refund_command_runtime = runtime
            .shared_get::<rustok_payment::PaymentAdminRefundCommandRuntime>()
            .unwrap_or_else(|| {
                rustok_payment::PaymentAdminRefundCommandRuntime::in_process(
                    runtime.db_clone(),
                    payment_provider_registry.clone(),
                )
            });
        let product_catalog_read_runtime = runtime
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require ProductCatalogReadRuntime in HostRuntimeContext"
                )
            })?;
        let product_catalog_command_runtime = runtime
            .shared_get::<rustok_product::ProductCatalogCommandRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require ProductCatalogCommandRuntime in HostRuntimeContext"
                )
            })?;
        #[cfg(feature = "marketplace-financial")]
        let marketplace_financial_runtime = runtime
            .shared_get::<crate::MarketplaceFinancialRuntime>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce marketplace-financial HTTP routes require MarketplaceFinancialRuntime in HostRuntimeContext"
                )
            })?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            payment_provider_registry,
            fulfillment_provider_registry,
            shipping_option_read_runtime,
            shipping_option_admin_command_runtime,
            fulfillment_lifecycle_read_runtime,
            fulfillment_admin_command_runtime,
            fulfillment_admin_create_command_runtime,
            order_read_runtime,
            order_admin_command_runtime,
            payment_order_read_runtime,
            payment_admin_read_runtime,
            payment_admin_collection_command_runtime,
            payment_admin_refund_command_runtime,
            product_catalog_read_runtime,
            product_catalog_command_runtime,
            #[cfg(feature = "marketplace-financial")]
            marketplace_financial_runtime,
        })
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = CommerceHttpRuntime::from_host(runtime)?;
    let router = axum::Router::new()
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
        );
    #[cfg(feature = "marketplace-financial")]
    let router = router.nest(
        "/admin/marketplace-financial",
        marketplace_financial::axum_router().merge(marketplace_reversal_financial::axum_router()),
    );
    Ok(router.with_state(state))
}
