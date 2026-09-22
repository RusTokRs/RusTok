use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_cart::{
    CartLineItemResponse, CartMarketplaceLineSnapshot, CartResponse,
    ListMarketplaceCartLineSnapshotsRequest, MarketplaceCartSnapshotReadPort,
    PreparedCartCheckoutSnapshot,
};
use rustok_commerce::{CheckoutError, CheckoutPlanBuilder, dto::CompleteCheckoutInput};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    ProductCatalogReadPort, ProductCatalogReadProfile, ProductCatalogReadRuntime,
    ProductProjectionRequest, PublishedProductsRequest, StorefrontProductList,
    VariantProductProjectionRequest, dto::ProductResponse,
};
use rustok_product_transport::{
    GrpcProductCatalogReadProvider, ProductCatalogGrpcOperation, ProductCatalogGrpcService,
    TrustedProductCatalogAuthority,
    proto::product_catalog_read_service_server::ProductCatalogReadServiceServer,
};
use sea_orm::Database;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Endpoint, Server};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum RemoteFailure {
    Unavailable,
    Timeout,
}

impl RemoteFailure {
    fn port_error(self) -> PortError {
        match self {
            Self::Unavailable => PortError::unavailable(
                "product.remote_unavailable",
                "remote Product catalog is unavailable",
            ),
            Self::Timeout => PortError::timeout(
                "product.remote_timeout",
                "remote Product catalog deadline was exceeded",
            ),
        }
    }

    const fn expected_kind(self) -> PortErrorKind {
        match self {
            Self::Unavailable => PortErrorKind::Unavailable,
            Self::Timeout => PortErrorKind::Timeout,
        }
    }

    const fn expected_code(self) -> &'static str {
        match self {
            Self::Unavailable => "product.remote_unavailable",
            Self::Timeout => "product.remote_timeout",
        }
    }
}

struct FailingProductCatalogReadPort {
    failure: RemoteFailure,
}

#[async_trait]
impl ProductCatalogReadPort for FailingProductCatalogReadPort {
    async fn read_product_projection(
        &self,
        _context: PortContext,
        _request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        Err(self.failure.port_error())
    }

    async fn read_variant_product_projection(
        &self,
        _context: PortContext,
        _request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        Err(self.failure.port_error())
    }

    async fn list_published_products(
        &self,
        _context: PortContext,
        _request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        Err(self.failure.port_error())
    }
}

struct EmptyMarketplaceSnapshots;

#[async_trait]
impl MarketplaceCartSnapshotReadPort for EmptyMarketplaceSnapshots {
    async fn list_marketplace_line_snapshots(
        &self,
        _context: PortContext,
        _request: ListMarketplaceCartLineSnapshotsRequest,
    ) -> Result<Vec<CartMarketplaceLineSnapshot>, PortError> {
        Ok(Vec::new())
    }
}

async fn remote_runtime(
    tenant_id: Uuid,
    failure: RemoteFailure,
) -> (
    ProductCatalogReadRuntime,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback Product listener should bind");
    let address = listener
        .local_addr()
        .expect("loopback Product listener address should exist");
    let incoming = TcpListenerStream::new(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let authority = TrustedProductCatalogAuthority::new(
        tenant_id.to_string(),
        PortActor::service("trusted-commerce-remote-profile"),
    )
    .allow_operations([
        ProductCatalogGrpcOperation::ReadProductProjection,
        ProductCatalogGrpcOperation::ReadVariantProductProjection,
    ]);
    let service = ProductCatalogReadServiceServer::with_interceptor(
        ProductCatalogGrpcService::new(Arc::new(FailingProductCatalogReadPort { failure })),
        move |mut request: tonic::Request<()>| {
            request.extensions_mut().insert(authority.clone());
            Ok(request)
        },
    );
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("loopback Product gRPC server should run");
    });
    let provider = GrpcProductCatalogReadProvider::connect(
        Endpoint::from_shared(format!("http://{address}"))
            .expect("loopback Product endpoint should parse")
            .connect_timeout(Duration::from_secs(5)),
    )
    .await
    .expect("loopback Product gRPC client should connect");
    let runtime = ProductCatalogReadRuntime::external(Arc::new(provider));
    assert_eq!(runtime.profile(), ProductCatalogReadProfile::External);
    (runtime, shutdown_tx, server)
}

fn prepared_snapshot(
    tenant_id: Uuid,
    cart_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
) -> PreparedCartCheckoutSnapshot {
    let now = Utc::now();
    let amount = Decimal::new(1000, 2);
    let cart = CartResponse {
        id: cart_id,
        tenant_id,
        channel_id: None,
        channel_slug: Some("web".to_string()),
        customer_id: None,
        email: None,
        region_id: None,
        country_code: None,
        locale_code: Some("en".to_string()),
        selected_shipping_option_id: None,
        status: "checking_out".to_string(),
        currency_code: "USD".to_string(),
        subtotal_amount: amount,
        adjustment_total: Decimal::ZERO,
        shipping_total: Decimal::ZERO,
        total_amount: amount,
        tax_total: Decimal::ZERO,
        metadata: serde_json::json!({}),
        created_at: now,
        updated_at: now,
        completed_at: None,
        line_items: vec![CartLineItemResponse {
            id: Uuid::new_v4(),
            cart_id,
            product_id: Some(product_id),
            variant_id: Some(variant_id),
            shipping_profile_slug: "default".to_string(),
            seller_id: None,
            seller_scope: None,
            sku: Some("REMOTE-SKU".to_string()),
            title: "Remote product".to_string(),
            quantity: 1,
            unit_price: amount,
            total_price: amount,
            currency_code: "USD".to_string(),
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }],
        adjustments: Vec::new(),
        tax_lines: Vec::new(),
        delivery_groups: Vec::new(),
    };
    PreparedCartCheckoutSnapshot {
        cart,
        shipping_address: None,
        billing_address: None,
        subtotal: amount,
        discount_total: Decimal::ZERO,
        tax_total: Decimal::ZERO,
        total: amount,
        snapshot_hash: "remote-product-consumer-snapshot".to_string(),
        projection_hash: "remote-product-consumer-projection".to_string(),
        status: "checking_out".to_string(),
        locked: true,
        delivery_groups: Vec::new(),
        tax_context: None,
        updated_at: now.fixed_offset(),
    }
}

async fn assert_remote_failure_blocks_checkout(failure: RemoteFailure) {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let variant_id = Uuid::new_v4();
    let (product_runtime, shutdown_tx, server) = remote_runtime(tenant_id, failure).await;
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory Commerce database should connect");
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let builder = CheckoutPlanBuilder::new(
        db.clone(),
        Arc::new(rustok_region::RegionService::new(db.clone())),
        rustok_inventory::in_process_inventory_reservation_port(db, event_bus),
        product_runtime.read_port(),
    )
    .with_marketplace_snapshot_read_port(Arc::new(EmptyMarketplaceSnapshots));
    let snapshot = prepared_snapshot(tenant_id, cart_id, product_id, variant_id);
    let input = CompleteCheckoutInput {
        cart_id,
        shipping_option_id: None,
        shipping_selections: None,
        region_id: None,
        country_code: None,
        locale: Some("en".to_string()),
        create_fulfillment: false,
        metadata: serde_json::json!({}),
    };

    let result = builder
        .build(tenant_id, actor_id, Uuid::new_v4(), &input, &snapshot)
        .await;
    let _ = shutdown_tx.send(());
    server
        .await
        .expect("loopback Product gRPC server task should stop");

    match result.expect_err("remote Product failure must block checkout planning") {
        CheckoutError::BoundaryFailure {
            stage,
            kind,
            code,
            retryable,
            ..
        } => {
            assert_eq!(stage, "read_checkout_product_projection");
            assert_eq!(kind, failure.expected_kind());
            assert_eq!(code, failure.expected_code());
            assert!(retryable);
        }
        error => panic!("expected Product boundary failure, got {error:?}"),
    }
}

#[tokio::test]
async fn remote_product_unavailable_blocks_checkout_without_snapshot_fallback() {
    assert_remote_failure_blocks_checkout(RemoteFailure::Unavailable).await;
}

#[tokio::test]
async fn remote_product_timeout_blocks_checkout_without_snapshot_fallback() {
    assert_remote_failure_blocks_checkout(RemoteFailure::Timeout).await;
}
