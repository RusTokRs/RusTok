use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, PublishedProductsRequest,
    StorefrontProductList, StorefrontProductListItem, VariantProductProjectionRequest,
    dto::ProductResponse, entities::product::ProductStatus,
};
use rustok_product_transport::{
    GrpcProductCatalogReadProvider, ProductCatalogGrpcOperation, ProductCatalogGrpcService,
    TrustedProductCatalogAuthority,
    proto::product_catalog_read_service_server::ProductCatalogReadServiceServer,
};
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Endpoint, Server};
use uuid::Uuid;

struct MockProductCatalogReadPort {
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
}

impl MockProductCatalogReadPort {
    fn validate_context(&self, context: &PortContext) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        assert_eq!(context.tenant_id, self.tenant_id.to_string());
        assert_eq!(
            context.actor,
            PortActor::service("trusted-product-catalog-conformance")
        );
        assert_eq!(context.channel.as_deref(), Some("web"));
        Ok(())
    }

    fn product(&self) -> ProductResponse {
        let now = Utc::now();
        ProductResponse {
            id: self.product_id,
            tenant_id: self.tenant_id,
            status: ProductStatus::Active,
            seller_id: None,
            vendor: Some("RusToK".to_string()),
            product_type: Some("demo".to_string()),
            shipping_profile_slug: Some("default".to_string()),
            primary_category_id: None,
            tags: vec!["remote".to_string()],
            metadata: serde_json::json!({"transport": "grpc"}),
            created_at: now,
            updated_at: now,
            published_at: Some(now),
            translations: Vec::new(),
            options: Vec::new(),
            variants: Vec::new(),
            images: Vec::new(),
        }
    }
}

#[async_trait]
impl ProductCatalogReadPort for MockProductCatalogReadPort {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        self.validate_context(&context)?;
        if request.product_id != self.product_id {
            return Err(PortError::not_found(
                "product.product_not_found",
                "product was not found",
            ));
        }
        Ok(self.product())
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        self.validate_context(&context)?;
        if request.variant_id != self.variant_id {
            return Err(PortError::not_found(
                "product.variant_not_found",
                "product variant was not found",
            ));
        }
        Ok(self.product())
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        self.validate_context(&context)?;
        assert_eq!(request.public_channel_slug.as_deref(), Some("web"));
        let product = self.product();
        Ok(StorefrontProductList {
            items: vec![StorefrontProductListItem {
                id: product.id,
                status: product.status,
                title: "Remote product".to_string(),
                handle: "remote-product".to_string(),
                seller_id: product.seller_id,
                vendor: product.vendor,
                product_type: product.product_type,
                tags: product.tags,
                created_at: product.created_at,
                published_at: product.published_at,
            }],
            total: 1,
            page: request.page,
            per_page: request.per_page,
            has_next: false,
        })
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("untrusted-product-client"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_channel("web")
    .with_deadline(Duration::from_secs(5))
}

#[tokio::test]
async fn loopback_grpc_provider_executes_the_product_catalog_port_contract() {
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let variant_id = Uuid::new_v4();
    let provider = Arc::new(MockProductCatalogReadPort {
        tenant_id,
        product_id,
        variant_id,
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener should bind");
    let address = listener
        .local_addr()
        .expect("loopback listener address should exist");
    let incoming = TcpListenerStream::new(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let authority = TrustedProductCatalogAuthority::new(
        tenant_id.to_string(),
        PortActor::service("trusted-product-catalog-conformance"),
    )
    .allow_operations([
        ProductCatalogGrpcOperation::ReadProductProjection,
        ProductCatalogGrpcOperation::ReadVariantProductProjection,
        ProductCatalogGrpcOperation::ListPublishedProducts,
    ]);
    let server_service = ProductCatalogReadServiceServer::with_interceptor(
        ProductCatalogGrpcService::new(provider),
        move |mut request: tonic::Request<()>| {
            request.extensions_mut().insert(authority.clone());
            Ok(request)
        },
    );
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(server_service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("loopback Product gRPC server should run");
    });
    let remote = GrpcProductCatalogReadProvider::connect(
        Endpoint::from_shared(format!("http://{address}"))
            .expect("loopback endpoint should parse")
            .connect_timeout(Duration::from_secs(5)),
    )
    .await
    .expect("loopback Product gRPC client should connect");

    let product = remote
        .read_product_projection(
            read_context(tenant_id),
            ProductProjectionRequest {
                product_id,
                locale: Some("en".to_string()),
                fallback_locale: None,
            },
        )
        .await
        .expect("product projection should cross gRPC");
    assert_eq!(product.id, product_id);
    assert_eq!(product.tenant_id, tenant_id);

    let variant_product = remote
        .read_variant_product_projection(
            read_context(tenant_id),
            VariantProductProjectionRequest {
                variant_id,
                locale: Some("en".to_string()),
                fallback_locale: None,
            },
        )
        .await
        .expect("variant-first projection should cross gRPC");
    assert_eq!(variant_product.id, product_id);

    let page = remote
        .list_published_products(
            read_context(tenant_id),
            PublishedProductsRequest {
                locale: Some("en".to_string()),
                fallback_locale: None,
                public_channel_slug: Some("web".to_string()),
                page: 1,
                per_page: 24,
            },
        )
        .await
        .expect("published products should cross gRPC");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, product_id);

    let missing = remote
        .read_product_projection(
            read_context(tenant_id),
            ProductProjectionRequest {
                product_id: Uuid::new_v4(),
                locale: None,
                fallback_locale: None,
            },
        )
        .await
        .expect_err("typed owner errors should cross gRPC details");
    assert_eq!(missing.kind, PortErrorKind::NotFound);
    assert_eq!(missing.code, "product.product_not_found");

    let missing_deadline = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("untrusted-product-client"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_channel("web");
    let deadline_error = remote
        .list_published_products(
            missing_deadline,
            PublishedProductsRequest {
                locale: None,
                fallback_locale: None,
                public_channel_slug: Some("web".to_string()),
                page: 1,
                per_page: 24,
            },
        )
        .await
        .expect_err("deadline policy should cross gRPC details");
    assert_eq!(deadline_error.kind, PortErrorKind::Timeout);
    assert_eq!(deadline_error.code, "port.deadline_required");

    let _ = shutdown_tx.send(());
    server
        .await
        .expect("loopback Product gRPC server task should stop");
}
