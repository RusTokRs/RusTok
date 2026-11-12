/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_product::{
    FilteredPublishedProductsRequest, ProductCatalogCommandPort, ProductCatalogReadPort,
    ProductProjectionRequest, ProductVariantIdentitiesRequest, ProductVariantIdentityListRequest,
    ProductVariantIdentityReadPort, ProductVariantIdentitySnapshot, PublishedProductsRequest,
    StorefrontCatalogFacet, StorefrontCatalogFacetsRequest, StorefrontProductList,
    StorefrontProductProjectionRequest, StorefrontVariantProductProjectionRequest,
    VariantProductProjectionRequest,
    dto::{
        AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
        ProductResponse, SetVariantAxesInput, UpdateProductImageInput, UpdateProductInput,
        UpdateVariantInput, VariantAxesPreviewResponse, VariantAxisConfigResponse, VariantResponse,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};
use uuid::Uuid;

use crate::auth::{AUTHORIZATION_METADATA, TENANT_ID_METADATA};
use crate::proto::JsonRequest;
use crate::proto::product_catalog_command_service_client::ProductCatalogCommandServiceClient;
use crate::proto::product_catalog_read_service_client::ProductCatalogReadServiceClient;
use crate::{ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken};

/// Consumer-side gRPC adapter implementing the Product-owned catalog read port.
pub struct GrpcProductCatalogReadProvider {
    client: ProductCatalogReadServiceClient<Channel>,
    authentication: Option<ProductCatalogGrpcBearerToken>,
}

impl GrpcProductCatalogReadProvider {
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            client: ProductCatalogReadServiceClient::new(channel),
            authentication: None,
        }
    }

    pub async fn connect(endpoint: Endpoint) -> Result<Self, tonic::transport::Error> {
        Ok(Self::from_channel(endpoint.connect().await?))
    }

    pub async fn connect_with_tls(
        endpoint: Endpoint,
        tls_config: ClientTlsConfig,
    ) -> Result<Self, tonic::transport::Error> {
        Ok(Self::from_channel(
            endpoint.tls_config(tls_config)?.connect().await?,
        ))
    }

    /// Installs a prevalidated deployment credential without exposing its secret value.
    pub fn with_authentication(mut self, authentication: ProductCatalogGrpcBearerToken) -> Self {
        self.authentication = Some(authentication);
        self
    }

    /// Validates and installs the deployment bearer credential used for every RPC.
    pub fn with_bearer_token(
        self,
        secret: impl AsRef<str>,
    ) -> Result<Self, ProductCatalogGrpcAuthenticationError> {
        Ok(self.with_authentication(ProductCatalogGrpcBearerToken::new(secret)?))
    }

    fn read_request(
        &self,
        context: &PortContext,
        input: &impl Serialize,
    ) -> Result<Request<JsonRequest>, PortError> {
        let payload = JsonRequest {
            context_json: encode(context)?,
            input_json: encode(input)?,
        };
        grpc_request(payload, context, self.authentication.as_ref())
    }
}

#[async_trait]
impl ProductCatalogReadPort for GrpcProductCatalogReadProvider {
    async fn read_storefront_variant_product_projection(
        &self,
        context: PortContext,
        request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .read_storefront_variant_product_projection(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .read_product_projection(grpc_request(
                payload,
                &context,
                self.authentication.as_ref(),
            )?)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .read_variant_product_projection(grpc_request(
                payload,
                &context,
                self.authentication.as_ref(),
            )?)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .list_published_products(grpc_request(
                payload,
                &context,
                self.authentication.as_ref(),
            )?)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn list_filtered_published_products(
        &self,
        context: PortContext,
        request: FilteredPublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .list_filtered_published_products(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn load_storefront_catalog_facets(
        &self,
        context: PortContext,
        request: StorefrontCatalogFacetsRequest,
    ) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .load_storefront_catalog_facets(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn read_storefront_product_projection(
        &self,
        context: PortContext,
        request: StorefrontProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .read_storefront_product_projection(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }
}

#[async_trait]
impl ProductVariantIdentityReadPort for GrpcProductCatalogReadProvider {
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .read_variant_identities(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        let request = self.read_request(&context, &request)?;
        let response = self
            .client
            .clone()
            .list_product_variant_identities(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductIdPayload {
    pub product_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VariantIdPayload {
    pub variant_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductInputPayload<T> {
    pub product_id: Uuid,
    pub input: T,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VariantInputPayload<T> {
    pub variant_id: Uuid,
    pub input: T,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductImageIdPayload {
    pub product_id: Uuid,
    pub image_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UpdateProductImagePayload {
    pub product_id: Uuid,
    pub image_id: Uuid,
    pub input: UpdateProductImageInput,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ReorderProductImagesPayload {
    pub product_id: Uuid,
    pub image_ids: Vec<Uuid>,
}

/// Consumer-side gRPC adapter implementing the Product-owned catalog command port.
pub struct GrpcProductCatalogCommandProvider {
    client: ProductCatalogCommandServiceClient<Channel>,
    authentication: Option<ProductCatalogGrpcBearerToken>,
}

impl GrpcProductCatalogCommandProvider {
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            client: ProductCatalogCommandServiceClient::new(channel),
            authentication: None,
        }
    }

    pub async fn connect(endpoint: Endpoint) -> Result<Self, tonic::transport::Error> {
        Ok(Self::from_channel(endpoint.connect().await?))
    }

    pub async fn connect_with_tls(
        endpoint: Endpoint,
        tls_config: ClientTlsConfig,
    ) -> Result<Self, tonic::transport::Error> {
        Ok(Self::from_channel(
            endpoint.tls_config(tls_config)?.connect().await?,
        ))
    }

    pub fn with_authentication(mut self, authentication: ProductCatalogGrpcBearerToken) -> Self {
        self.authentication = Some(authentication);
        self
    }

    pub fn with_bearer_token(
        self,
        secret: impl AsRef<str>,
    ) -> Result<Self, ProductCatalogGrpcAuthenticationError> {
        Ok(self.with_authentication(ProductCatalogGrpcBearerToken::new(secret)?))
    }

    fn command_request(
        &self,
        context: &PortContext,
        input: &impl Serialize,
    ) -> Result<Request<JsonRequest>, PortError> {
        let payload = JsonRequest {
            context_json: encode(context)?,
            input_json: encode(input)?,
        };
        grpc_request(payload, context, self.authentication.as_ref())
    }
}

#[async_trait]
impl ProductCatalogCommandPort for GrpcProductCatalogCommandProvider {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError> {
        let request = self.command_request(&context, &input)?;
        let response = self
            .client
            .clone()
            .create_product(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn preview_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<VariantAxesPreviewResponse, PortError> {
        let payload = ProductInputPayload { product_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .preview_variant_axes(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn set_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<Vec<VariantAxisConfigResponse>, PortError> {
        let payload = ProductInputPayload { product_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .set_variant_axes(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn update_product(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError> {
        let payload = ProductInputPayload { product_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .update_product(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn delete_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError> {
        let payload = ProductIdPayload { product_id };
        let request = self.command_request(&context, &payload)?;
        let _ = self
            .client
            .clone()
            .delete_product(request)
            .await
            .map_err(status_to_port_error)?;
        Ok(())
    }

    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let payload = ProductIdPayload { product_id };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .publish_product(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn unpublish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let payload = ProductIdPayload { product_id };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .unpublish_product(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let payload = ProductIdPayload { product_id };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .archive_product(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn create_variant(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let payload = ProductInputPayload { product_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .create_variant(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn update_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let payload = VariantInputPayload { variant_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .update_variant(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn delete_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
    ) -> Result<(), PortError> {
        let payload = VariantIdPayload { variant_id };
        let request = self.command_request(&context, &payload)?;
        let _ = self
            .client
            .clone()
            .delete_variant(request)
            .await
            .map_err(status_to_port_error)?;
        Ok(())
    }

    async fn add_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        let payload = ProductInputPayload { product_id, input };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .add_product_image(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn update_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
        input: UpdateProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        let payload = UpdateProductImagePayload {
            product_id,
            image_id,
            input,
        };
        let request = self.command_request(&context, &payload)?;
        let response = self
            .client
            .clone()
            .update_product_image(request)
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn delete_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let payload = ProductImageIdPayload {
            product_id,
            image_id,
        };
        let request = self.command_request(&context, &payload)?;
        let _ = self
            .client
            .clone()
            .delete_product_image(request)
            .await
            .map_err(status_to_port_error)?;
        Ok(())
    }

    async fn reorder_product_images(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> Result<(), PortError> {
        let payload = ReorderProductImagesPayload {
            product_id,
            image_ids,
        };
        let request = self.command_request(&context, &payload)?;
        let _ = self
            .client
            .clone()
            .reorder_product_images(request)
            .await
            .map_err(status_to_port_error)?;
        Ok(())
    }
}

fn grpc_request<T>(
    payload: T,
    context: &PortContext,
    authentication: Option<&ProductCatalogGrpcBearerToken>,
) -> Result<Request<T>, PortError> {
    let mut request = Request::new(payload);
    if let Some(deadline_ms) = context.deadline_ms.filter(|deadline| *deadline > 0) {
        request.set_timeout(Duration::from_millis(deadline_ms));
    }

    let Some(authentication) = authentication else {
        return Ok(request);
    };
    if context.tenant_id != context.tenant_id.trim()
        || Uuid::parse_str(context.tenant_id.as_str()).is_err()
    {
        return Err(PortError::validation(
            "product.grpc_tenant_id_invalid",
            "Product gRPC tenant context is invalid",
        ));
    }
    let tenant_id = MetadataValue::<Ascii>::try_from(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.grpc_tenant_id_invalid",
            "Product gRPC tenant context is invalid",
        )
    })?;
    request
        .metadata_mut()
        .insert(AUTHORIZATION_METADATA, authentication.authorization_value());
    request.metadata_mut().insert(TENANT_ID_METADATA, tenant_id);
    Ok(request)
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, PortError> {
    serde_json::to_vec(value).map_err(|error| {
        PortError::invariant_violation("product.transport_encode", error.to_string())
    })
}

fn decode<T: DeserializeOwned>(value: &[u8]) -> Result<T, PortError> {
    serde_json::from_slice(value).map_err(|error| {
        PortError::invariant_violation("product.transport_decode", error.to_string())
    })
}

fn status_to_port_error(status: Status) -> PortError {
    if !status.details().is_empty()
        && let Ok(error) = serde_json::from_slice::<PortError>(status.details())
    {
        return error;
    }

    if status.code() == Code::Unimplemented {
        return PortError::new(
            PortErrorKind::Unavailable,
            "product.grpc_operation_unsupported",
            "The configured Product provider does not support this operation",
            false,
        );
    }

    let kind = match status.code() {
        Code::InvalidArgument => PortErrorKind::Validation,
        Code::NotFound => PortErrorKind::NotFound,
        Code::AlreadyExists | Code::Aborted | Code::FailedPrecondition => PortErrorKind::Conflict,
        Code::PermissionDenied | Code::Unauthenticated => PortErrorKind::Forbidden,
        Code::DeadlineExceeded => PortErrorKind::Timeout,
        Code::Unavailable | Code::ResourceExhausted => PortErrorKind::Unavailable,
        _ => PortErrorKind::InvariantViolation,
    };
    let retryable = matches!(kind, PortErrorKind::Timeout | PortErrorKind::Unavailable);
    PortError::new(
        kind,
        format!(
            "product.grpc.{}",
            status.code().description().replace(' ', "_")
        ),
        status.message(),
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use rustok_api::{PortActor, PortErrorKind};

    use super::*;

    #[test]
    fn structured_port_error_is_preserved_from_grpc_status_details() {
        let expected = PortError::validation("product.sku_invalid", "SKU format is invalid");
        let encoded = serde_json::to_vec(&expected).expect("port error should serialize");
        let status =
            Status::with_details(Code::InvalidArgument, "validation failed", encoded.into());

        assert_eq!(status_to_port_error(status), expected);
    }

    #[test]
    fn unsupported_rpc_is_unavailable_without_retry() {
        let error = status_to_port_error(Status::unimplemented("method not available"));
        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert_eq!(error.code, "product.grpc_operation_unsupported");
        assert!(!error.retryable);
    }

    #[test]
    fn unstructured_transport_status_retains_retryability() {
        let error = status_to_port_error(Status::unavailable("down"));
        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert!(error.retryable);
    }

    #[test]
    fn authenticated_request_carries_bearer_and_tenant_metadata() {
        let tenant_id = Uuid::new_v4().to_string();
        let context = PortContext::new(
            tenant_id.clone(),
            PortActor::service("rustok-server"),
            "en",
            "corr-auth",
        );
        let authentication = ProductCatalogGrpcBearerToken::new("catalog-secret")
            .expect("valid bearer token should be accepted");
        let request = grpc_request((), &context, Some(&authentication))
            .expect("authenticated request should be created");

        assert_eq!(
            request
                .metadata()
                .get(AUTHORIZATION_METADATA)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer catalog-secret")
        );
        assert_eq!(
            request
                .metadata()
                .get(TENANT_ID_METADATA)
                .and_then(|value| value.to_str().ok()),
            Some(tenant_id.as_str())
        );
    }

    #[test]
    fn authenticated_request_rejects_invalid_tenant_metadata() {
        let context = PortContext::new(
            "tenant-not-a-uuid",
            PortActor::service("rustok-server"),
            "en",
            "corr-invalid-tenant",
        );
        let authentication = ProductCatalogGrpcBearerToken::new("catalog-secret")
            .expect("valid bearer token should be accepted");
        let error = grpc_request((), &context, Some(&authentication))
            .expect_err("invalid Product tenant must fail before transport");
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.grpc_tenant_id_invalid");
    }
}
