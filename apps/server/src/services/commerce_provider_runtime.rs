use rustok_api::HostRuntimeContext;
#[cfg(feature = "mod-marketplace_seller")]
use std::sync::Arc;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Attach the host-composed commerce provider registries and owner runtimes to a capability
/// runtime.
///
/// Values already installed in `ServerRuntimeContext` or `HostRuntimeContext` are always preserved
/// so external adapters registered by the host remain visible to every transport. When no provider
/// runtime exists, the process composes the deterministic built-in baseline once.
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

    #[cfg(all(feature = "mod-commerce", feature = "mod-payment"))]
    let host = {
        let runtime = host
            .shared_get::<rustok_payment::PaymentCollectionRuntime>()
            .or_else(|| server.shared_get::<rustok_payment::PaymentCollectionRuntime>())
            .unwrap_or_else(|| {
                rustok_payment::PaymentCollectionRuntime::in_process(server.db_clone())
            });
        server.shared_insert(runtime.clone());
        host.with_shared_value(runtime)
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

    #[cfg(all(feature = "mod-commerce", feature = "mod-fulfillment"))]
    let host = {
        let runtime = server
            .shared_get::<rustok_commerce::graphql_runtime::CommerceShippingOptionReadRuntime>()
            .unwrap_or_else(|| {
                let runtime =
                    rustok_commerce::graphql_runtime::CommerceShippingOptionReadRuntime::in_process(
                        server.db_clone(),
                    );
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-fulfillment"))]
    let host = {
        let runtime = host
            .shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()
            .or_else(|| server.shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>())
            .unwrap_or_else(|| {
                rustok_fulfillment::ShippingOptionAdminCommandRuntime::in_process(
                    server.db_clone(),
                )
            });
        server.shared_insert(runtime.clone());
        host.with_shared_value(runtime)
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-fulfillment"))]
    let host = {
        let runtime = server
            .shared_get::<rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()
            .unwrap_or_else(|| {
                let runtime = rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime::in_process(
                    server.db_clone(),
                );
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
    let host = {
        let runtime = server
            .shared_get::<rustok_commerce::graphql_runtime::CommerceOrderReadRuntime>()
            .unwrap_or_else(|| {
                let event_bus = server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .expect(
                        "TransactionalEventBus must be initialized before CommerceOrderReadRuntime",
                    );
                let runtime =
                    rustok_commerce::graphql_runtime::CommerceOrderReadRuntime::in_process(
                        server.db_clone(),
                        event_bus,
                    );
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
    let host = {
        let runtime = host
            .shared_get::<rustok_order::OrderAdminCommandRuntime>()
            .or_else(|| server.shared_get::<rustok_order::OrderAdminCommandRuntime>())
            .or_else(|| {
                server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .map(|event_bus| {
                        rustok_order::OrderAdminCommandRuntime::in_process(
                            server.db_clone(),
                            event_bus,
                        )
                    })
            });
        match runtime {
            Some(runtime) => {
                server.shared_insert(runtime.clone());
                host.with_shared_value(runtime)
            }
            None => host,
        }
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
    let host = {
        let runtime = host
            .shared_get::<rustok_order::OrderPostOrderCommandRuntime>()
            .or_else(|| server.shared_get::<rustok_order::OrderPostOrderCommandRuntime>())
            .or_else(|| {
                server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .map(|event_bus| {
                        rustok_order::OrderPostOrderCommandRuntime::in_process(
                            server.db_clone(),
                            event_bus,
                        )
                    })
            });
        match runtime {
            Some(runtime) => {
                server.shared_insert(runtime.clone());
                host.with_shared_value(runtime)
            }
            None => host,
        }
    };

    #[cfg(all(feature = "mod-commerce", feature = "mod-payment"))]
    let host = {
        let runtime = host
            .shared_get::<rustok_payment::PaymentOrderReadRuntime>()
            .or_else(|| server.shared_get::<rustok_payment::PaymentOrderReadRuntime>())
            .unwrap_or_else(|| rustok_payment::PaymentOrderReadRuntime::in_process(server.db_clone()));
        server.shared_insert(runtime.clone());
        host.with_shared_value(runtime)
    };

    #[cfg(feature = "commerce-marketplace-financial")]
    let host = {
        let runtime = server
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime =
                    rustok_commerce::MarketplaceFinancialRuntime::in_process(server.db_clone());
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(feature = "mod-marketplace_seller")]
    let host = {
        let runtime = server
            .shared_get::<rustok_marketplace_seller::MarketplaceSellerRuntime>()
            .unwrap_or_else(|| {
                let service = Arc::new(rustok_marketplace_seller::MarketplaceSellerService::new(
                    server.db_clone(),
                ));
                let read_port: Arc<dyn rustok_marketplace_seller::MarketplaceSellerReadPort> =
                    service.clone();
                let command_port: Arc<dyn rustok_marketplace_seller::MarketplaceSellerCommandPort> =
                    service;
                let runtime = rustok_marketplace_seller::MarketplaceSellerRuntime::new(
                    read_port,
                    command_port,
                );
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(feature = "mod-product")]
    let host = {
        let runtime = host
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .or_else(|| server.shared_get::<rustok_product::ProductCatalogReadRuntime>())
            .or_else(|| {
                server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .map(|event_bus| {
                        rustok_product::ProductCatalogReadRuntime::in_process(
                            server.db_clone(),
                            event_bus,
                        )
                    })
            });
        match runtime {
            Some(runtime) => {
                server.shared_insert(runtime.clone());
                host.with_shared_value(runtime)
            }
            None => host,
        }
    };

    #[cfg(feature = "mod-product")]
    let host = {
        let runtime = host
            .shared_get::<rustok_product::ProductCatalogCommandRuntime>()
            .or_else(|| server.shared_get::<rustok_product::ProductCatalogCommandRuntime>())
            .or_else(|| {
                server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .map(|event_bus| {
                        rustok_product::ProductCatalogCommandRuntime::in_process(
                            server.db_clone(),
                            event_bus,
                        )
                    })
            });
        match runtime {
            Some(runtime) => {
                server.shared_insert(runtime.clone());
                host.with_shared_value(runtime)
            }
            None => host,
        }
    };

    #[cfg(all(
        feature = "mod-marketplace_listing",
        feature = "mod-marketplace_seller",
        feature = "mod-product"
    ))]
    let host = {
        let runtime = server
            .shared_get::<rustok_marketplace_listing::MarketplaceListingRuntime>()
            .unwrap_or_else(|| {
                let event_bus = server
                    .shared_get::<rustok_outbox::TransactionalEventBus>()
                    .expect("TransactionalEventBus must be initialized before marketplace listing");
                let seller_reader = server
                    .shared_get::<rustok_marketplace_seller::MarketplaceSellerRuntime>()
                    .expect(
                        "MarketplaceSellerRuntime must be initialized before marketplace listing",
                    )
                    .shared_read_port();
                let product_reader = server
                    .shared_get::<rustok_product::ProductCatalogReadRuntime>()
                    .expect(
                        "ProductCatalogReadRuntime must be initialized before marketplace listing",
                    )
                    .read_port();
                let ports = Arc::new(rustok_marketplace_listing::MarketplaceListingService::new(
                    server.db_clone(),
                    event_bus,
                    seller_reader,
                    product_reader,
                ));
                let runtime = rustok_marketplace_listing::MarketplaceListingRuntime::new(ports);
                server.shared_insert(runtime.clone());
                runtime
            });
        host.with_shared_value(runtime)
    };

    #[cfg(all(
        feature = "commerce-marketplace-financial",
        feature = "mod-payment"
    ))]
    let host = {
        let observers = server
            .shared_get::<rustok_payment::PaymentProviderEventObservers>()
            .unwrap_or_else(|| {
                let runtime = server
                    .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
                    .expect(
                        "MarketplaceFinancialRuntime must be initialized before payment event observers",
                    );
                let observers = runtime.payment_provider_event_observers(server.db_clone());
                server.shared_insert(observers.clone());
                observers
            });
        host.with_shared_value(observers)
    };

    #[cfg(all(feature = "mod-ai", feature = "mod-order"))]
    let host = if let Some(event_bus) = server.shared_get::<rustok_outbox::TransactionalEventBus>()
    {
        let port = rustok_order::in_process_checkout_completion_port(server.db_clone(), event_bus);
        host.with_shared_value(rustok_ai::SharedAiOrderStatusPort(port))
    } else {
        host
    };

    #[cfg(all(feature = "mod-ai", feature = "mod-product"))]
    let host =
        if let Some(runtime) = server.shared_get::<rustok_product::ProductCatalogReadRuntime>() {
            host.with_shared_value(rustok_ai::SharedAiProductCatalogReadPort(
                runtime.read_port(),
            ))
        } else {
            host
        };

    host
}

#[cfg(all(test, feature = "mod-payment", feature = "mod-fulfillment"))]
mod tests {
    #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
    use std::sync::Arc;

    #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
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
        #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
        server.shared_insert(TransactionalEventBus::new(Arc::new(OutboxTransport::new(
            db.clone(),
        ))));

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

        #[cfg(all(feature = "mod-commerce", feature = "mod-order"))]
        {
            let first_order = first
                .shared_get::<rustok_commerce::graphql_runtime::CommerceOrderReadRuntime>()
                .expect("order read runtime should be attached");
            let second_order = second
                .shared_get::<rustok_commerce::graphql_runtime::CommerceOrderReadRuntime>()
                .expect("order read runtime should be reused");
            assert!(Arc::ptr_eq(
                &first_order.order_read_port(),
                &second_order.order_read_port()
            ));
        }
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

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use sea_orm::Database;

    use super::attach_commerce_provider_registries;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    struct ExternalProductCatalogReadPort;

    #[async_trait]
    impl rustok_product::ProductCatalogReadPort for ExternalProductCatalogReadPort {
        async fn read_product_projection(
            &self,
            _context: PortContext,
            _request: rustok_product::ProductProjectionRequest,
        ) -> Result<rustok_product::dto::ProductResponse, PortError> {
            Err(PortError::unavailable(
                "external.test",
                "external test provider",
            ))
        }

        async fn read_variant_product_projection(
            &self,
            _context: PortContext,
            _request: rustok_product::VariantProductProjectionRequest,
        ) -> Result<rustok_product::dto::ProductResponse, PortError> {
            Err(PortError::unavailable(
                "external.test",
                "external test provider",
            ))
        }

        async fn list_published_products(
            &self,
            _context: PortContext,
            _request: rustok_product::PublishedProductsRequest,
        ) -> Result<rustok_product::StorefrontProductList, PortError> {
            Err(PortError::unavailable(
                "external.test",
                "external test provider",
            ))
        }
    }

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
            host.shared_get::<rustok_product::ProductCatalogReadRuntime>()
                .is_some()
        );
        assert!(
            host.shared_get::<rustok_ai::SharedAiProductCatalogReadPort>()
                .is_some()
        );
    }

    #[tokio::test]
    async fn preserves_host_selected_external_product_catalog_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let external = rustok_product::ProductCatalogReadRuntime::external(Arc::new(
            ExternalProductCatalogReadPort,
        ));
        let host = rustok_api::HostRuntimeContext::new(db).with_shared_value(external);

        let attached = attach_commerce_provider_registries(host, &server);
        let runtime = attached
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .expect("external product runtime should remain attached");
        assert_eq!(
            runtime.profile(),
            rustok_product::ProductCatalogReadProfile::External
        );
        assert!(
            attached
                .shared_get::<rustok_ai::SharedAiProductCatalogReadPort>()
                .is_some()
        );
    }
}
