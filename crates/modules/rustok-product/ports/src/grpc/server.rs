use std::{collections::HashSet, fmt, sync::Arc};

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use crate::{
    FilteredPublishedProductsRequest, ProductCatalogCommandPort, ProductCatalogReadPort,
    ProductProjectionRequest, ProductVariantIdentityReadPort, PublishedProductsRequest,
    StorefrontCatalogFacetsRequest, StorefrontProductProjectionRequest,
    StorefrontVariantProductProjectionRequest, VariantProductProjectionRequest,
    dto::{
        AddProductImageInput, CreateProductInput, CreateVariantInput, SetVariantAxesInput,
        UpdateProductInput, UpdateVariantInput,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::service::Interceptor;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use super::auth::{
    AUTHORIZATION_METADATA, ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken,
    TENANT_ID_METADATA,
};
use super::client::{
    ProductIdPayload, ProductImageIdPayload, ProductInputPayload, ReorderProductImagesPayload,
    UpdateProductImagePayload, VariantIdPayload, VariantInputPayload,
};
use super::proto::product_catalog_command_service_server::ProductCatalogCommandService;
use super::proto::product_catalog_read_service_server::ProductCatalogReadService;
use super::proto::{JsonRequest, JsonResponse};

/// Provider-side adapter. The wrapped Product owner port retains catalog policy and persistence.
pub struct ProductCatalogGrpcService<P> {
    provider: Arc<P>,
    variant_identity_read_port: Option<Arc<dyn ProductVariantIdentityReadPort>>,
}

/// Authority established by a server-side authentication/authorization interceptor.
///
/// Network payloads may carry correlation, locale, channel, deadline, and idempotency metadata,
/// but tenant and principal authority are replaced with this trusted value.
#[derive(Clone, Debug)]
pub struct TrustedProductCatalogAuthority {
    pub tenant_id: String,
    pub actor: PortActor,
    pub claims: Vec<String>,
    pub roles: Vec<String>,
    allowed_operations: HashSet<ProductCatalogGrpcOperation>,
}

/// Product catalog operations authorized by the server-side authentication boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProductCatalogGrpcOperation {
    ReadProductProjection,
    ReadVariantProductProjection,
    ListPublishedProducts,
    ReadVariantIdentities,
    ListProductVariantIdentities,
    ReadStorefrontVariantProductProjection,
    ListFilteredPublishedProducts,
    LoadStorefrontCatalogFacets,
    ReadStorefrontProductProjection,
    CreateProduct,
    UpdateProduct,
    PublishProduct,
    UnpublishProduct,
    ArchiveProduct,
    DeleteProduct,
    CreateVariant,
    UpdateVariant,
    DeleteVariant,
    SetVariantAxes,
    PreviewVariantAxes,
    AddProductImage,
    UpdateProductImage,
    DeleteProductImage,
    ReorderProductImages,
}

impl ProductCatalogGrpcOperation {
    pub const READ: [Self; 9] = [
        Self::ReadProductProjection,
        Self::ReadVariantProductProjection,
        Self::ListPublishedProducts,
        Self::ReadVariantIdentities,
        Self::ListProductVariantIdentities,
        Self::ReadStorefrontVariantProductProjection,
        Self::ListFilteredPublishedProducts,
        Self::LoadStorefrontCatalogFacets,
        Self::ReadStorefrontProductProjection,
    ];

    pub const COMMAND: [Self; 15] = [
        Self::CreateProduct,
        Self::UpdateProduct,
        Self::PublishProduct,
        Self::UnpublishProduct,
        Self::ArchiveProduct,
        Self::DeleteProduct,
        Self::CreateVariant,
        Self::UpdateVariant,
        Self::DeleteVariant,
        Self::SetVariantAxes,
        Self::PreviewVariantAxes,
        Self::AddProductImage,
        Self::UpdateProductImage,
        Self::DeleteProductImage,
        Self::ReorderProductImages,
    ];
}

impl TrustedProductCatalogAuthority {
    pub fn new(tenant_id: impl Into<String>, actor: PortActor) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            allowed_operations: HashSet::new(),
        }
    }

    pub fn with_claim(mut self, claim: impl Into<String>) -> Self {
        self.claims.push(claim.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn allow_operation(mut self, operation: ProductCatalogGrpcOperation) -> Self {
        self.allowed_operations.insert(operation);
        self
    }

    pub fn allow_operations(
        mut self,
        operations: impl IntoIterator<Item = ProductCatalogGrpcOperation>,
    ) -> Self {
        self.allowed_operations.extend(operations);
        self
    }
}

/// Static service-to-service bearer authenticator for the Product read boundary.
///
/// A valid token authorizes the configured service actor to access the tenant
/// carried in trusted gRPC metadata. The token itself is never included in
/// `Debug`, status messages, or tracing fields.
#[derive(Clone)]
pub struct ProductCatalogGrpcBearerAuthenticator {
    token: ProductCatalogGrpcBearerToken,
    actor: PortActor,
    claims: Vec<String>,
    roles: Vec<String>,
    allowed_operations: HashSet<ProductCatalogGrpcOperation>,
}

impl ProductCatalogGrpcBearerAuthenticator {
    pub fn new(
        secret: impl AsRef<str>,
        actor: PortActor,
    ) -> Result<Self, ProductCatalogGrpcAuthenticationError> {
        Ok(Self::from_token(
            ProductCatalogGrpcBearerToken::new(secret)?,
            actor,
        ))
    }

    pub fn from_token(token: ProductCatalogGrpcBearerToken, actor: PortActor) -> Self {
        Self {
            token,
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            allowed_operations: HashSet::from(ProductCatalogGrpcOperation::READ),
        }
    }

    pub fn with_claim(mut self, claim: impl Into<String>) -> Self {
        self.claims.push(claim.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn with_allowed_operations(
        mut self,
        operations: impl IntoIterator<Item = ProductCatalogGrpcOperation>,
    ) -> Self {
        self.allowed_operations = operations.into_iter().collect();
        self
    }

    fn authenticate(
        &self,
        request: &Request<()>,
    ) -> Result<TrustedProductCatalogAuthority, Status> {
        let authorization = request
            .metadata()
            .get(AUTHORIZATION_METADATA)
            .ok_or_else(authentication_failed)?;
        if !self.token.matches_authorization(authorization.as_bytes()) {
            return Err(authentication_failed());
        }

        let tenant_id = request
            .metadata()
            .get(TENANT_ID_METADATA)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(authentication_failed)?;
        if tenant_id != tenant_id.trim() || Uuid::parse_str(tenant_id).is_err() {
            return Err(authentication_failed());
        }

        Ok(TrustedProductCatalogAuthority {
            tenant_id: tenant_id.to_string(),
            actor: self.actor.clone(),
            claims: self.claims.clone(),
            roles: self.roles.clone(),
            allowed_operations: self.allowed_operations.clone(),
        })
    }
}

impl fmt::Debug for ProductCatalogGrpcBearerAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductCatalogGrpcBearerAuthenticator")
            .field("token", &"[REDACTED]")
            .field("actor", &self.actor)
            .field("claims", &self.claims)
            .field("roles", &self.roles)
            .field("allowed_operations", &self.allowed_operations)
            .finish()
    }
}

/// Tonic interceptor that converts authenticated metadata into trusted Product authority.
#[derive(Clone, Debug)]
pub struct ProductCatalogGrpcBearerInterceptor {
    authenticator: ProductCatalogGrpcBearerAuthenticator,
}

impl ProductCatalogGrpcBearerInterceptor {
    pub fn new(authenticator: ProductCatalogGrpcBearerAuthenticator) -> Self {
        Self { authenticator }
    }

    pub fn from_bearer_token(
        secret: impl AsRef<str>,
        actor: PortActor,
    ) -> Result<Self, ProductCatalogGrpcAuthenticationError> {
        Ok(Self::new(ProductCatalogGrpcBearerAuthenticator::new(
            secret, actor,
        )?))
    }
}

impl Interceptor for ProductCatalogGrpcBearerInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let authority = self.authenticator.authenticate(&request)?;
        request.extensions_mut().insert(authority);
        Ok(request)
    }
}

fn authentication_failed() -> Status {
    Status::unauthenticated("Product catalog service authentication failed")
}

impl<P> ProductCatalogGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            variant_identity_read_port: None,
        }
    }

    pub fn with_variant_identity_read_port(
        mut self,
        provider: Arc<dyn ProductVariantIdentityReadPort>,
    ) -> Self {
        self.variant_identity_read_port = Some(provider);
        self
    }

    fn variant_identity_read_port(&self) -> Result<&dyn ProductVariantIdentityReadPort, Status> {
        self.variant_identity_read_port.as_deref().ok_or_else(|| {
            Status::unimplemented("Product SKU identity capability is not configured")
        })
    }
}

#[tonic::async_trait]
impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P>
where
    P: ProductCatalogReadPort + 'static,
{
    async fn read_storefront_variant_product_projection(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadStorefrontVariantProductProjection,
        )?;
        let input: StorefrontVariantProductProjectionRequest =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .read_storefront_variant_product_projection(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn read_product_projection(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadProductProjection,
        )?;
        let request = request.into_inner();
        let input: ProductProjectionRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .read_product_projection(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn read_variant_product_projection(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadVariantProductProjection,
        )?;
        let request = request.into_inner();
        let input: VariantProductProjectionRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .read_variant_product_projection(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn list_published_products(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ListPublishedProducts,
        )?;
        let request = request.into_inner();
        let input: PublishedProductsRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .list_published_products(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn read_variant_identities(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadVariantIdentities,
        )?;
        let input = decode_input(&request.into_inner().input_json)?;
        let identities = self
            .variant_identity_read_port()?
            .read_variant_identities(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&identities)
    }

    async fn list_product_variant_identities(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ListProductVariantIdentities,
        )?;
        let input = decode_input(&request.into_inner().input_json)?;
        let identities = self
            .variant_identity_read_port()?
            .list_product_variant_identities(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&identities)
    }

    async fn list_filtered_published_products(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ListFilteredPublishedProducts,
        )?;
        let request = request.into_inner();
        let input: FilteredPublishedProductsRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .list_filtered_published_products(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn load_storefront_catalog_facets(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::LoadStorefrontCatalogFacets,
        )?;
        let request = request.into_inner();
        let input: StorefrontCatalogFacetsRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .load_storefront_catalog_facets(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn read_storefront_product_projection(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadStorefrontProductProjection,
        )?;
        let request = request.into_inner();
        let input: StorefrontProductProjectionRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .read_storefront_product_projection(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }
}

/// Provider-side adapter for the Product catalog command boundary.
pub struct ProductCatalogCommandGrpcService<P> {
    provider: Arc<P>,
}

impl<P> ProductCatalogCommandGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[tonic::async_trait]
impl<P> ProductCatalogCommandService for ProductCatalogCommandGrpcService<P>
where
    P: ProductCatalogCommandPort + 'static,
{
    async fn create_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::CreateProduct,
        )?;
        let input: CreateProductInput = decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .create_product(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn preview_variant_axes(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::PreviewVariantAxes,
        )?;
        let payload: ProductInputPayload<SetVariantAxesInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .preview_variant_axes(context, payload.product_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn set_variant_axes(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::SetVariantAxes,
        )?;
        let payload: ProductInputPayload<SetVariantAxesInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .set_variant_axes(context, payload.product_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn update_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::UpdateProduct,
        )?;
        let payload: ProductInputPayload<UpdateProductInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .update_product(context, payload.product_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn delete_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::DeleteProduct,
        )?;
        let payload: ProductIdPayload = decode_input(&request.into_inner().input_json)?;
        self.provider
            .delete_product(context, payload.product_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&())
    }

    async fn publish_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::PublishProduct,
        )?;
        let payload: ProductIdPayload = decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .publish_product(context, payload.product_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn unpublish_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::UnpublishProduct,
        )?;
        let payload: ProductIdPayload = decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .unpublish_product(context, payload.product_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn archive_product(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ArchiveProduct,
        )?;
        let payload: ProductIdPayload = decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .archive_product(context, payload.product_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn create_variant(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::CreateVariant,
        )?;
        let payload: ProductInputPayload<CreateVariantInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .create_variant(context, payload.product_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn update_variant(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::UpdateVariant,
        )?;
        let payload: VariantInputPayload<UpdateVariantInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .update_variant(context, payload.variant_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn delete_variant(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::DeleteVariant,
        )?;
        let payload: VariantIdPayload = decode_input(&request.into_inner().input_json)?;
        self.provider
            .delete_variant(context, payload.variant_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&())
    }

    async fn add_product_image(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::AddProductImage,
        )?;
        let payload: ProductInputPayload<AddProductImageInput> =
            decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .add_product_image(context, payload.product_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn update_product_image(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::UpdateProductImage,
        )?;
        let payload: UpdateProductImagePayload = decode_input(&request.into_inner().input_json)?;
        let value = self
            .provider
            .update_product_image(context, payload.product_id, payload.image_id, payload.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn delete_product_image(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::DeleteProductImage,
        )?;
        let payload: ProductImageIdPayload = decode_input(&request.into_inner().input_json)?;
        self.provider
            .delete_product_image(context, payload.product_id, payload.image_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&())
    }

    async fn reorder_product_images(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReorderProductImages,
        )?;
        let payload: ReorderProductImagesPayload = decode_input(&request.into_inner().input_json)?;
        self.provider
            .reorder_product_images(context, payload.product_id, payload.image_ids)
            .await
            .map_err(port_error_to_status)?;
        json_response(&())
    }
}

fn trusted_context<T>(
    request: &Request<T>,
    mut claimed: PortContext,
    operation: ProductCatalogGrpcOperation,
) -> Result<PortContext, Status> {
    let authority = request
        .extensions()
        .get::<TrustedProductCatalogAuthority>()
        .ok_or_else(|| Status::unauthenticated("trusted Product catalog authority is missing"))?;
    if !authority.allowed_operations.contains(&operation) {
        return Err(Status::permission_denied(
            "trusted Product catalog authority does not allow this operation",
        ));
    }
    if claimed.tenant_id != authority.tenant_id {
        return Err(Status::permission_denied(
            "Product tenant does not match authenticated authority",
        ));
    }
    claimed.tenant_id.clone_from(&authority.tenant_id);
    claimed.actor = authority.actor.clone();
    claimed.claims.clone_from(&authority.claims);
    claimed.roles.clone_from(&authority.roles);
    Ok(claimed)
}

fn decode_context(value: &[u8]) -> Result<PortContext, Status> {
    decode_input(value)
}

fn decode_input<T: DeserializeOwned>(value: &[u8]) -> Result<T, Status> {
    serde_json::from_slice(value).map_err(|error| {
        port_error_to_status(PortError::validation(
            "product.transport_invalid_json",
            error.to_string(),
        ))
    })
}

fn json_response<T: Serialize>(value: &T) -> Result<Response<JsonResponse>, Status> {
    let output_json = serde_json::to_vec(value).map_err(|error| {
        port_error_to_status(PortError::invariant_violation(
            "product.transport_encode",
            error.to_string(),
        ))
    })?;
    Ok(Response::new(JsonResponse { output_json }))
}

fn port_error_to_status(error: PortError) -> Status {
    let code = match error.kind {
        PortErrorKind::Validation => Code::InvalidArgument,
        PortErrorKind::NotFound => Code::NotFound,
        PortErrorKind::Conflict => Code::FailedPrecondition,
        PortErrorKind::Forbidden => Code::PermissionDenied,
        PortErrorKind::Unavailable => Code::Unavailable,
        PortErrorKind::Timeout => Code::DeadlineExceeded,
        PortErrorKind::InvariantViolation => Code::Internal,
    };
    let details = serde_json::to_vec(&error).unwrap_or_default();
    Status::with_details(code, error.message, Bytes::from(details))
}

#[cfg(test)]
mod tests {
    use super::{
        ProductCatalogGrpcBearerInterceptor, ProductCatalogGrpcOperation,
        TrustedProductCatalogAuthority, port_error_to_status, trusted_context,
    };
    use rustok_api::{PortActor, PortContext, PortError};
    use tonic::metadata::MetadataValue;
    use tonic::service::Interceptor;
    use tonic::{Code, Request};
    use uuid::Uuid;

    use crate::auth::{AUTHORIZATION_METADATA, TENANT_ID_METADATA};

    #[test]
    fn owner_error_is_preserved_in_status_details() {
        let error = PortError::not_found("product.not_found", "missing");
        let status = port_error_to_status(error.clone());
        assert_eq!(status.code(), Code::NotFound);
        assert_eq!(
            serde_json::from_slice::<PortError>(status.details()).unwrap(),
            error
        );
    }

    #[test]
    fn remote_context_requires_server_side_authority() {
        let request = Request::new(());
        let context = PortContext::new(
            "tenant-a",
            PortActor::service("untrusted-client"),
            "en",
            "corr-a",
        );
        let status = trusted_context(
            &request,
            context,
            ProductCatalogGrpcOperation::ReadProductProjection,
        )
        .expect_err("missing authority must fail closed");
        assert_eq!(status.code(), Code::Unauthenticated);
    }

    #[test]
    fn trusted_authority_replaces_untrusted_actor() {
        let authority = TrustedProductCatalogAuthority::new(
            "tenant-a",
            PortActor::service("trusted-product-service"),
        )
        .allow_operation(ProductCatalogGrpcOperation::ReadProductProjection);
        let mut request = Request::new(());
        request.extensions_mut().insert(authority);
        let context = PortContext::new(
            "tenant-a",
            PortActor::service("untrusted-client"),
            "en",
            "corr-b",
        );
        let trusted = trusted_context(
            &request,
            context,
            ProductCatalogGrpcOperation::ReadProductProjection,
        )
        .expect("trusted authority should be accepted");
        assert_eq!(trusted.tenant_id, "tenant-a");
        assert_eq!(trusted.actor, PortActor::service("trusted-product-service"));
    }

    #[test]
    fn projection_grant_does_not_authorize_variant_identity() {
        let authority = TrustedProductCatalogAuthority::new(
            "tenant-a",
            PortActor::service("projection-only-client"),
        )
        .allow_operation(ProductCatalogGrpcOperation::ReadVariantProductProjection);
        let mut request = Request::new(());
        request.extensions_mut().insert(authority);
        let context = PortContext::new("tenant-a", PortActor::system(), "en", "corr-identity");

        for operation in [
            ProductCatalogGrpcOperation::ReadVariantIdentities,
            ProductCatalogGrpcOperation::ListProductVariantIdentities,
        ] {
            let status = trusted_context(&request, context.clone(), operation)
                .expect_err("SKU identity requires its own operation grant");
            assert_eq!(status.code(), Code::PermissionDenied);
        }
    }

    #[test]
    fn bearer_interceptor_authenticates_tenant_and_service_actor() {
        let tenant_id = Uuid::new_v4().to_string();
        let mut interceptor = ProductCatalogGrpcBearerInterceptor::from_bearer_token(
            "catalog-secret",
            PortActor::service("rustok-server"),
        )
        .expect("valid service credential should be accepted");
        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTHORIZATION_METADATA,
            MetadataValue::try_from("Bearer catalog-secret").unwrap(),
        );
        request.metadata_mut().insert(
            TENANT_ID_METADATA,
            MetadataValue::try_from(tenant_id.as_str()).unwrap(),
        );

        let request = interceptor
            .call(request)
            .expect("valid service metadata should authenticate");
        let authority = request
            .extensions()
            .get::<TrustedProductCatalogAuthority>()
            .expect("trusted authority should be installed");
        assert_eq!(authority.tenant_id, tenant_id);
        assert_eq!(authority.actor, PortActor::service("rustok-server"));
        for operation in ProductCatalogGrpcOperation::READ {
            assert!(authority.allowed_operations.contains(&operation));
        }
        for operation in ProductCatalogGrpcOperation::COMMAND {
            assert!(!authority.allowed_operations.contains(&operation));
        }
    }

    #[test]
    fn bearer_interceptor_rejects_missing_or_wrong_token() {
        let mut interceptor = ProductCatalogGrpcBearerInterceptor::from_bearer_token(
            "catalog-secret",
            PortActor::service("rustok-server"),
        )
        .expect("valid service credential should be accepted");
        let missing = interceptor
            .call(Request::new(()))
            .expect_err("missing credential must fail closed");
        assert_eq!(missing.code(), Code::Unauthenticated);

        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTHORIZATION_METADATA,
            MetadataValue::try_from("Bearer wrong-secret").unwrap(),
        );
        request.metadata_mut().insert(
            TENANT_ID_METADATA,
            MetadataValue::try_from(Uuid::new_v4().to_string()).unwrap(),
        );
        let wrong = interceptor
            .call(request)
            .expect_err("wrong credential must fail closed");
        assert_eq!(wrong.code(), Code::Unauthenticated);
    }

    #[test]
    fn bearer_interceptor_rejects_invalid_tenant_metadata() {
        let mut interceptor = ProductCatalogGrpcBearerInterceptor::from_bearer_token(
            "catalog-secret",
            PortActor::service("rustok-server"),
        )
        .expect("valid service credential should be accepted");
        let mut request = Request::new(());
        request.metadata_mut().insert(
            AUTHORIZATION_METADATA,
            MetadataValue::try_from("Bearer catalog-secret").unwrap(),
        );
        request.metadata_mut().insert(
            TENANT_ID_METADATA,
            MetadataValue::try_from("tenant-not-a-uuid").unwrap(),
        );
        let status = interceptor
            .call(request)
            .expect_err("invalid tenant metadata must fail closed");
        assert_eq!(status.code(), Code::Unauthenticated);
    }
}
