use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_product::{
    ProductCatalogCommandPort, ProductCatalogReadPort, ProductCatalogReadRuntime,
    ProductFulfillmentRequirement, ProductProjectionRequest, ProductVariantIdentitiesRequest,
    ProductVariantIdentityListRequest, ProductVariantIdentityReadPort,
    ProductVariantIdentitySnapshot, PublishedProductsRequest, StorefrontProductList,
    StorefrontProductListItem, StorefrontVariantProductProjectionRequest,
    VariantProductProjectionRequest,
    dto::{
        AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
        ProductResponse, SetVariantAxesInput, UpdateProductImageInput, UpdateProductInput,
        UpdateVariantInput, VariantAxesPreviewResponse, VariantAxisConfigResponse,
        VariantResponse,
    },
    entities::product::ProductStatus,
};
use rustok_product_transport::{
    GrpcProductCatalogCommandProvider, GrpcProductCatalogReadProvider,
    ProductCatalogCommandGrpcService, ProductCatalogGrpcBearerAuthenticator,
    ProductCatalogGrpcBearerInterceptor, ProductCatalogGrpcOperation, ProductCatalogGrpcService,
    proto::product_catalog_command_service_server::ProductCatalogCommandServiceServer,
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
            fulfillment_requirement: ProductFulfillmentRequirement::Physical,
            shipping_profile_slug: Some("default".to_string()),
            primary_category_id: None,
            tags: vec!["remote".to_string()],
            brands: Vec::new(),
            metadata: serde_json::json!({"transport": "grpc"}),
            created_at: now,
            updated_at: now,
            published_at: Some(now),
            revision: 1,
            translations: Vec::new(),
            variant_axes: Vec::new(),
            variants: Vec::new(),
            images: Vec::new(),
            storefront_attributes: Vec::new(),
        }
    }
}

#[async_trait]
impl ProductCatalogReadPort for MockProductCatalogReadPort {
    async fn read_storefront_variant_product_projection(
        &self,
        context: PortContext,
        request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        self.validate_context(&context)?;
        assert_eq!(request.locale.as_deref(), Some("en"));
        assert_eq!(request.fallback_locale.as_deref(), Some("en-GB"));
        assert_eq!(request.public_channel_slug.as_deref(), Some("web"));
        Ok((request.variant_id == self.variant_id).then(|| self.product()))
    }

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
                primary_image: None,
                price_from: None,
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

#[async_trait]
impl ProductVariantIdentityReadPort for MockProductCatalogReadPort {
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        self.validate_context(&context)?;
        if request.variant_ids != [self.variant_id] {
            return Err(PortError::not_found(
                "product.variant_identity_missing",
                "one or more SKUs were not found in the current tenant",
            ));
        }
        assert_eq!(request.product_id, Some(self.product_id));
        Ok(vec![self.identity()])
    }

    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        self.validate_context(&context)?;
        assert_eq!(request.product_ids, [self.product_id]);
        Ok(vec![self.identity()])
    }
}

impl MockProductCatalogReadPort {
    fn identity(&self) -> ProductVariantIdentitySnapshot {
        ProductVariantIdentitySnapshot {
            tenant_id: self.tenant_id,
            product_id: self.product_id,
            variant_id: self.variant_id,
        }
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
    let authentication = ProductCatalogGrpcBearerAuthenticator::new(
        "loopback-catalog-token",
        PortActor::service("trusted-product-catalog-conformance"),
    )
    .expect("test credential must be valid")
    .with_allowed_operations([
        ProductCatalogGrpcOperation::ReadProductProjection,
        ProductCatalogGrpcOperation::ReadVariantProductProjection,
        ProductCatalogGrpcOperation::ListPublishedProducts,
        ProductCatalogGrpcOperation::ReadVariantIdentities,
        ProductCatalogGrpcOperation::ListProductVariantIdentities,
        ProductCatalogGrpcOperation::ReadStorefrontVariantProductProjection,
    ]);
    let expected_identity = provider.identity();
    let server_service = ProductCatalogReadServiceServer::with_interceptor(
        ProductCatalogGrpcService::new(provider.clone()).with_variant_identity_read_port(provider),
        ProductCatalogGrpcBearerInterceptor::new(authentication),
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

    let identity_request = ProductVariantIdentitiesRequest {
        product_id: Some(product_id),
        variant_ids: vec![variant_id],
    };
    let unauthenticated = remote
        .read_variant_identities(read_context(tenant_id), identity_request.clone())
        .await
        .expect_err("SKU identity calls require a bearer credential");
    assert_eq!(unauthenticated.kind, PortErrorKind::Forbidden);
    assert!(!unauthenticated.retryable);

    let remote = Arc::new(
        remote
            .with_bearer_token("loopback-catalog-token")
            .expect("test credential must be valid"),
    );
    let runtime = ProductCatalogReadRuntime::external(remote.clone())
        .with_variant_identity_read_port(remote.clone());
    let identity_port = runtime
        .variant_identity_read_port()
        .expect("the selected remote runtime must provide SKU identities");
    let identities = identity_port
        .read_variant_identities(read_context(tenant_id), identity_request)
        .await
        .expect("SKU identity lookup should cross gRPC");
    assert_eq!(identities, [expected_identity.clone()]);

    let listed = identity_port
        .list_product_variant_identities(
            read_context(tenant_id),
            ProductVariantIdentityListRequest {
                product_ids: vec![product_id],
            },
        )
        .await
        .expect("SKU identities for the selected Product page should cross gRPC");
    assert_eq!(listed, [expected_identity]);

    let missing_identity = identity_port
        .read_variant_identities(
            read_context(tenant_id),
            ProductVariantIdentitiesRequest {
                product_id: Some(product_id),
                variant_ids: vec![Uuid::new_v4()],
            },
        )
        .await
        .expect_err("missing SKU must retain its typed owner error");
    assert_eq!(missing_identity.kind, PortErrorKind::NotFound);
    assert_eq!(missing_identity.code, "product.variant_identity_missing");
    assert!(!missing_identity.retryable);

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

    let storefront_request = StorefrontVariantProductProjectionRequest {
        variant_id,
        locale: Some("en".to_string()),
        fallback_locale: Some("en-GB".to_string()),
        public_channel_slug: Some("web".to_string()),
    };
    let storefront_product = runtime
        .read_port()
        .read_storefront_variant_product_projection(
            read_context(tenant_id),
            storefront_request.clone(),
        )
        .await
        .expect("storefront SKU projection should cross gRPC")
        .expect("published SKU should resolve");
    assert_eq!(storefront_product.id, product_id);
    assert_eq!(storefront_product.tenant_id, tenant_id);
    let hidden = runtime
        .read_port()
        .read_storefront_variant_product_projection(
            read_context(tenant_id),
            StorefrontVariantProductProjectionRequest {
                variant_id: Uuid::new_v4(),
                ..storefront_request
            },
        )
        .await
        .expect("owner absence should cross gRPC");
    assert!(hidden.is_none());

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

struct MockProductCatalogCommandPort {
    tenant_id: Uuid,
    product_id: Uuid,
}

#[async_trait]
impl ProductCatalogCommandPort for MockProductCatalogCommandPort {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        assert_eq!(context.tenant_id, self.tenant_id.to_string());
        assert_eq!(
            context.actor,
            PortActor::service("trusted-product-catalog-conformance")
        );
        let now = Utc::now();
        Ok(ProductResponse {
            id: self.product_id,
            tenant_id: self.tenant_id,
            status: if input.publish {
                ProductStatus::Active
            } else {
                ProductStatus::Draft
            },
            seller_id: None,
            vendor: Some("RusToK".to_string()),
            product_type: Some("demo".to_string()),
            fulfillment_requirement: ProductFulfillmentRequirement::Physical,
            shipping_profile_slug: Some("default".to_string()),
            primary_category_id: None,
            tags: vec!["created-via-grpc".to_string()],
            brands: Vec::new(),
            metadata: serde_json::json!({"created_via": "grpc"}),
            created_at: now,
            updated_at: now,
            published_at: if input.publish { Some(now) } else { None },
            revision: 1,
            translations: Vec::new(),
            variant_axes: Vec::new(),
            variants: Vec::new(),
            images: Vec::new(),
            storefront_attributes: Vec::new(),
        })
    }

    async fn preview_variant_axes(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _input: SetVariantAxesInput,
    ) -> Result<VariantAxesPreviewResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn set_variant_axes(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _input: SetVariantAxesInput,
    ) -> Result<Vec<VariantAxisConfigResponse>, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn update_product(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn delete_product(&self, context: PortContext, product_id: Uuid) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        assert_eq!(product_id, self.product_id);
        Ok(())
    }

    async fn publish_product(
        &self,
        _context: PortContext,
        _product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn unpublish_product(
        &self,
        _context: PortContext,
        _product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn archive_product(
        &self,
        _context: PortContext,
        _product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn create_variant(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn update_variant(
        &self,
        _context: PortContext,
        _variant_id: Uuid,
        _input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn delete_variant(&self, _context: PortContext, _variant_id: Uuid) -> Result<(), PortError> {
        Ok(())
    }

    async fn add_product_image(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn update_product_image(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _image_id: Uuid,
        _input: UpdateProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        Err(PortError::unavailable("mock.unimplemented", "not used in test"))
    }

    async fn delete_product_image(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _image_id: Uuid,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn reorder_product_images(
        &self,
        _context: PortContext,
        _product_id: Uuid,
        _image_ids: Vec<Uuid>,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[tokio::test]
async fn loopback_grpc_provider_executes_the_product_catalog_command_port_contract() {
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let provider = Arc::new(MockProductCatalogCommandPort {
        tenant_id,
        product_id,
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener should bind");
    let address = listener
        .local_addr()
        .expect("loopback listener address should exist");
    let incoming = TcpListenerStream::new(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let authentication = ProductCatalogGrpcBearerAuthenticator::new(
        "loopback-command-token",
        PortActor::service("trusted-product-catalog-conformance"),
    )
    .expect("test credential must be valid")
    .with_allowed_operations([
        ProductCatalogGrpcOperation::CreateProduct,
        ProductCatalogGrpcOperation::DeleteProduct,
    ]);

    let server_service = ProductCatalogCommandServiceServer::with_interceptor(
        ProductCatalogCommandGrpcService::new(provider.clone()),
        ProductCatalogGrpcBearerInterceptor::new(authentication),
    );
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(server_service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("loopback Product command gRPC server should run");
    });

    let remote = GrpcProductCatalogCommandProvider::connect(
        Endpoint::from_shared(format!("http://{address}"))
            .expect("loopback endpoint should parse")
            .connect_timeout(Duration::from_secs(5)),
    )
    .await
    .expect("loopback Product command gRPC client should connect");

    let write_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("untrusted-caller"),
        "en",
        "corr-command",
    )
    .with_idempotency_key("idemp-create-product")
    .with_deadline(Duration::from_millis(5_000));

    let create_input = CreateProductInput {
        attribute_values: Vec::new(),
        translations: Vec::new(),
        variant_axes: Vec::new(),
        variants: Vec::new(),
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        brands: Vec::new(),
        tags: Vec::new(),
        metadata: serde_json::json!({}),
        publish: true,
    };

    let unauthenticated = remote
        .create_product(write_context.clone(), create_input.clone())
        .await
        .expect_err("command calls without bearer token must fail");
    assert_eq!(unauthenticated.kind, PortErrorKind::Forbidden);

    let remote = remote
        .with_bearer_token("loopback-command-token")
        .expect("bearer token should be accepted");

    let created = remote
        .create_product(write_context.clone(), create_input)
        .await
        .expect("create_product should succeed over gRPC loopback");
    assert_eq!(created.id, product_id);
    assert_eq!(created.tenant_id, tenant_id);
    assert_eq!(created.status, ProductStatus::Active);

    remote
        .delete_product(write_context, product_id)
        .await
        .expect("delete_product should succeed over gRPC loopback");

    let _ = shutdown_tx.send(());
    server
        .await
        .expect("loopback Product command gRPC server task should stop");
}
