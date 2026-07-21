use std::sync::Arc;

use rustok_api::HostRuntimeContext;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Attach the host-composed commerce provider registries to a capability runtime.
///
/// A registry already installed in `ServerRuntimeContext` is always preserved so
/// external adapters registered by the host remain visible to every transport.
/// When no payment registry exists, the process-owned provider runtime composes
/// the manual baseline and any deployment-configured external adapters once.
pub fn attach_commerce_provider_registries(
    host: HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> HostRuntimeContext {
    #[cfg(feature = "mod-payment")]
    let host = {
        let registry = server
            .shared_get::<rustok_payment::providers::PaymentProviderRegistry>()
            .unwrap_or_else(|| {
                let registry =
                    crate::services::payment_provider_runtime::build_payment_provider_registry(
                        server,
                    )
                    .unwrap_or_else(|error| {
                        panic!("payment provider runtime initialization failed: {error}")
                    });
                server.shared_insert(registry.clone());
                registry
            });
        host.with_shared_value(registry)
    };

    #[cfg(feature = "mod-fulfillment")]
    let host = {
        let registry = server
            .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            .unwrap_or_else(|| {
                let registry = rustok_fulfillment::providers::FulfillmentProviderRegistry::with_manual_provider();
                server.shared_insert(registry.clone());
                registry
            });
        host.with_shared_value(registry)
    };

    #[cfg(feature = "mod-commerce")]
    let host = {
        let runtime = server
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime = rustok_commerce::MarketplaceFinancialRuntime::in_process(
                    server.db_clone(),
                );
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(all(feature = "mod-ai", feature = "mod-order"))]
    let host = if let Some(event_bus) = server.shared_get::<rustok_outbox::TransactionalEventBus>()
    {
        let port: Arc<dyn rustok_order::CheckoutCompletionPort> = Arc::new(
            rustok_order::OrderService::new(server.db_clone(), event_bus),
        );
        host.with_shared_value(rustok_ai::SharedAiOrderStatusPort(port))
    } else {
        host
    };

    #[cfg(all(feature = "mod-ai", feature = "mod-product"))]
    let host = if let Some(event_bus) = server.shared_get::<rustok_outbox::TransactionalEventBus>()
    {
        let port: Arc<dyn rustok_product::ProductCatalogReadPort> = Arc::new(
            rustok_product::CatalogService::new(server.db_clone(), event_bus),
        );
        host.with_shared_value(rustok_ai::SharedAiProductCatalogReadPort(port))
    } else {
        host
    };

    host
}

#[cfg(all(test, feature = "mod-payment", feature = "mod-fulfillment"))]
mod tests {
    use sea_orm::Database;

    use super::attach_commerce_provider_registries;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    #[tokio::test]
    async fn installs_shared_manual_registries_once() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());

        let first = attach_commerce_provider_registries(
            rustok_api::HostRuntimeContext::new(db.clone()),
            &server,
        );
        let second =
            attach_commerce_provider_registries(rustok_api::HostRuntimeContext::new(db), &server);

        let first_payment = first
            .shared_get::<rustok_payment::providers::PaymentProviderRegistry>()
            .expect("payment registry should be attached");
        let second_payment = second
            .shared_get::<rustok_payment::providers::PaymentProviderRegistry>()
            .expect("payment registry should be reused");
        assert_eq!(first_payment.descriptors(), second_payment.descriptors());

        let first_fulfillment = first
            .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            .expect("fulfillment registry should be attached");
        let second_fulfillment = second
            .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            .expect("fulfillment registry should be reused");
        assert_eq!(
            first_fulfillment.descriptors(),
            second_fulfillment.descriptors()
        );
    }
}

#[cfg(all(test, feature = "mod-ai", feature = "mod-order"))]
mod order_status_port_tests {
    use std::sync::Arc;

    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use sea_orm::Database;

    use super::attach_commerce_provider_registries;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    #[tokio::test]
    async fn attaches_order_status_port_for_ai_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        server.shared_insert(TransactionalEventBus::new(Arc::new(OutboxTransport::new(
            db.clone(),
        ))));

        let host =
            attach_commerce_provider_registries(rustok_api::HostRuntimeContext::new(db), &server);
        assert!(
            host.shared_get::<rustok_ai::SharedAiOrderStatusPort>()
                .is_some()
        );
    }
}

#[cfg(all(test, feature = "mod-ai", feature = "mod-product"))]
mod product_catalog_read_port_tests {
    use std::sync::Arc;

    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use sea_orm::Database;

    use super::attach_commerce_provider_registries;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    #[tokio::test]
    async fn attaches_product_catalog_read_port_for_ai_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        server.shared_insert(TransactionalEventBus::new(Arc::new(OutboxTransport::new(
            db.clone(),
        ))));

        let host =
            attach_commerce_provider_registries(rustok_api::HostRuntimeContext::new(db), &server);
        assert!(
            host.shared_get::<rustok_ai::SharedAiProductCatalogReadPort>()
                .is_some()
        );
    }
}
