use std::{collections::HashSet, fmt, io, sync::Arc};

use crate::ProductAttachedValuesWritePort;
use crate::{
    FilteredPublishedProductsRequest, ProductAttachedValuesReadPort, ProductCatalogCommandPort,
    ProductCatalogReadPort, ProductProjectionRequest, ProductStorefrontHttpReadPort,
    ProductVariantIdentityReadPort, PublishedProductsRequest, StorefrontCatalogFacetsRequest,
    StorefrontProductProjectionRequest, StorefrontVariantProductProjectionRequest,
    VariantProductProjectionRequest,
    dto::{
        AddProductImageInput, CreateProductInput, CreateVariantInput, SetVariantAxesInput,
        UpdateProductInput, UpdateVariantInput,
    },
};
use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use serde::{Serialize, de::DeserializeOwned};
use tonic::service::Interceptor;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use super::auth::{
    AUTHORIZATION_METADATA, ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken,
    TENANT_ID_METADATA,
};
use super::payloads::{
    DeleteVariantPayload, ProductIdPayload, ProductImageIdPayload, ProductInputPayload,
    ReadAttachedValuesPayload, ReorderProductImagesPayload, UpdateProductImagePayload,
    VariantInputPayload,
};
use super::proto::product_catalog_command_service_server::ProductCatalogCommandService;
use super::proto::product_catalog_read_service_server::ProductCatalogReadService;
use super::proto::{JsonRequest, JsonResponse};
use crate::{
    ProductLocaleReplayEntry, ProductLocaleReplayKey, ProductLocaleReplayPage,
    ProductLocaleReplayPort, ProductLocaleReplayScan, ProductVariantReplayPort,
    ProductVariantReplayScan,
};

/// Provider-side adapter. The wrapped Product owner port retains catalog policy and persistence.
pub struct ProductCatalogGrpcService<P> {
    provider: Arc<P>,
    storefront_http_read_port: Option<Arc<dyn ProductStorefrontHttpReadPort>>,
    variant_identity_read_port: Option<Arc<dyn ProductVariantIdentityReadPort>>,
    attached_values_read_port: Option<Arc<dyn ProductAttachedValuesReadPort>>,
    locale_replay_port: Option<Arc<dyn ProductLocaleReplayPort>>,
    variant_replay_port: Option<Arc<dyn ProductVariantReplayPort>>,
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
    ListLegacyStorefrontProducts,
    ListLegacyStorefrontHttpProducts,
    ReadAttachedValues,
    ScanVariantReplay,
    LoadVariantReplay,
    ScanLocaleReplay,
    LoadLocaleReplay,
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
    UpdateAttachedValues,
    DeleteAttachedValues,
}

impl ProductCatalogGrpcOperation {
    pub const LOCALE_REPLAY: [Self; 2] = [Self::ScanLocaleReplay, Self::LoadLocaleReplay];

    pub const VARIANT_REPLAY: [Self; 2] = [Self::ScanVariantReplay, Self::LoadVariantReplay];

    pub const READ: [Self; 11] = [
        Self::ReadProductProjection,
        Self::ReadVariantProductProjection,
        Self::ListPublishedProducts,
        Self::ReadVariantIdentities,
        Self::ListProductVariantIdentities,
        Self::ReadStorefrontVariantProductProjection,
        Self::ListFilteredPublishedProducts,
        Self::LoadStorefrontCatalogFacets,
        Self::ReadStorefrontProductProjection,
        Self::ListLegacyStorefrontProducts,
        Self::ListLegacyStorefrontHttpProducts,
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
        if tenant_id != tenant_id.trim()
            || Uuid::parse_str(tenant_id)
                .ok()
                .is_none_or(|tenant_id| tenant_id.is_nil())
        {
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
    additional_authenticators: Vec<ProductCatalogGrpcBearerAuthenticator>,
}

impl ProductCatalogGrpcBearerInterceptor {
    pub fn new(authenticator: ProductCatalogGrpcBearerAuthenticator) -> Self {
        Self {
            authenticator,
            additional_authenticators: Vec::new(),
        }
    }

    pub fn with_additional_authenticator(
        mut self,
        authenticator: ProductCatalogGrpcBearerAuthenticator,
    ) -> Self {
        self.additional_authenticators.push(authenticator);
        self
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
        let authority = self.authenticator.authenticate(&request).or_else(|error| {
            self.additional_authenticators
                .iter()
                .find_map(|authenticator| authenticator.authenticate(&request).ok())
                .ok_or(error)
        })?;
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
            storefront_http_read_port: None,
            variant_identity_read_port: None,
            attached_values_read_port: None,
            variant_replay_port: None,
            locale_replay_port: None,
        }
    }

    pub fn with_storefront_http_read_port(
        mut self,
        provider: Arc<dyn ProductStorefrontHttpReadPort>,
    ) -> Self {
        self.storefront_http_read_port = Some(provider);
        self
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

    pub fn with_attached_values_read_port(
        mut self,
        provider: Arc<dyn ProductAttachedValuesReadPort>,
    ) -> Self {
        self.attached_values_read_port = Some(provider);
        self
    }

    pub fn with_variant_replay_port(mut self, provider: Arc<dyn ProductVariantReplayPort>) -> Self {
        self.variant_replay_port = Some(provider);
        self
    }

    fn variant_replay_port(&self) -> Result<&dyn ProductVariantReplayPort, Status> {
        self.variant_replay_port
            .as_deref()
            .ok_or_else(|| Status::unimplemented("Product variant replay is not configured"))
    }

    pub fn with_locale_replay_port(mut self, provider: Arc<dyn ProductLocaleReplayPort>) -> Self {
        self.locale_replay_port = Some(provider);
        self
    }

    fn locale_replay_port(&self) -> Result<&dyn ProductLocaleReplayPort, Status> {
        self.locale_replay_port
            .as_deref()
            .ok_or_else(|| Status::unimplemented("Product locale replay is not configured"))
    }
}

#[tonic::async_trait]
impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P>
where
    P: ProductCatalogReadPort + 'static,
{
    async fn scan_variant_replay(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ScanVariantReplay,
        )?;
        let input: ProductVariantReplayScan = decode_input(&request.into_inner().input_json)?;
        let page = self
            .variant_replay_port()?
            .scan_variant_replay(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&page)
    }

    async fn load_variant_replay(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::LoadVariantReplay,
        )?;
        let ids: Vec<uuid::Uuid> = decode_input(&request.into_inner().input_json)?;
        let entries = self
            .variant_replay_port()?
            .load_variant_replay(context, ids)
            .await
            .map_err(port_error_to_status)?;
        json_response(&entries)
    }

    async fn scan_locale_replay(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ScanLocaleReplay,
        )?;
        let input: ProductLocaleReplayScan = decode_input(&request.into_inner().input_json)?;
        let page = self
            .locale_replay_port()?
            .scan_locale_replay(context, input)
            .await
            .map_err(port_error_to_status)?;
        locale_replay_page_response(page)
    }

    async fn load_locale_replay(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::LoadLocaleReplay,
        )?;
        let keys: Vec<ProductLocaleReplayKey> = decode_input(&request.into_inner().input_json)?;
        let entries = self
            .locale_replay_port()?
            .load_locale_replay(context, keys)
            .await
            .map_err(port_error_to_status)?;
        json_response(&entries)
    }

    async fn read_attached_values(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ReadAttachedValues,
        )?;
        let input: ReadAttachedValuesPayload = decode_input(&request.into_inner().input_json)?;
        let provider = self.attached_values_read_port.as_deref().ok_or_else(|| {
            Status::unimplemented("Product attached values capability is not configured")
        })?;
        let value = provider
            .read_attached_values(context, input.product_id, &input.fallback_locale)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

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

    async fn list_legacy_storefront_products(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ListLegacyStorefrontProducts,
        )?;
        let input = decode_input(&request.into_inner().input_json)?;
        let result = self
            .provider
            .list_legacy_storefront_products(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&result)
    }

    async fn list_legacy_storefront_http_products(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::ListLegacyStorefrontHttpProducts,
        )?;
        let provider = self.storefront_http_read_port.as_deref().ok_or_else(|| {
            Status::unimplemented("Product storefront HTTP read capability is not configured")
        })?;
        let input = decode_input(&request.into_inner().input_json)?;
        let result = provider
            .list_legacy_storefront_http_products(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&result)
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
    attached_values_write_port: Option<Arc<dyn ProductAttachedValuesWritePort>>,
}

impl<P> ProductCatalogCommandGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            attached_values_write_port: None,
        }
    }

    pub fn with_attached_values_write_port(
        mut self,
        provider: Arc<dyn ProductAttachedValuesWritePort>,
    ) -> Self {
        self.attached_values_write_port = Some(provider);
        self
    }

    fn attached_values_write_port(&self) -> Result<&dyn ProductAttachedValuesWritePort, Status> {
        self.attached_values_write_port.as_deref().ok_or_else(|| {
            Status::unimplemented("Product attached values write capability is not configured")
        })
    }
}

#[tonic::async_trait]
impl<P> ProductCatalogCommandService for ProductCatalogCommandGrpcService<P>
where
    P: ProductCatalogCommandPort + 'static,
{
    async fn update_attached_values(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::UpdateAttachedValues,
        )?;
        let input: ProductInputPayload<Option<serde_json::Value>> =
            decode_input(&request.into_inner().input_json)?;
        let response = self
            .attached_values_write_port()?
            .update_attached_values(context, input.product_id, input.input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&response)
    }

    async fn delete_attached_values(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::DeleteAttachedValues,
        )?;
        let input: ProductIdPayload = decode_input(&request.into_inner().input_json)?;
        self.attached_values_write_port()?
            .delete_attached_values(context, input.product_id)
            .await
            .map_err(port_error_to_status)?;
        json_response(&())
    }
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

    async fn update_variant_with_revision(
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

    async fn delete_variant_with_revision(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            ProductCatalogGrpcOperation::DeleteVariant,
        )?;
        let payload: DeleteVariantPayload = decode_input(&request.into_inner().input_json)?;
        self.provider
            .delete_variant(context, payload.variant_id, payload.expected_revision)
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

// Leave space for the protobuf bytes field and gRPC frame within the default 4 MiB receive limit.
const LOCALE_REPLAY_JSON_LIMIT: usize = 4 * 1024 * 1024 - 16;

fn locale_replay_page_response(
    page: ProductLocaleReplayPage,
) -> Result<Response<JsonResponse>, Status> {
    #[derive(Serialize)]
    struct Page<'a> {
        entries: &'a [ProductLocaleReplayEntry],
        next: Option<ProductLocaleReplayKey>,
    }

    let mut count = page.entries.len();
    let mut output = BoundedReplayJson::default();
    loop {
        let entries = &page.entries[..count];
        let next = if count == page.entries.len() {
            page.next.clone()
        } else {
            entries.last().map(ProductLocaleReplayEntry::key)
        };
        match serde_json::to_writer(&mut output, &Page { entries, next }) {
            Ok(()) => {
                return Ok(Response::new(JsonResponse {
                    output_json: output.bytes,
                }));
            }
            Err(_) if output.exceeded && count > 1 => {
                count /= 2;
                output.bytes.clear();
                output.exceeded = false;
            }
            Err(_) if output.exceeded => {
                return Err(port_error_to_status(PortError::invariant_violation(
                    "product.locale_replay_entry_too_large",
                    "A Product locale replay entry exceeds the gRPC response limit",
                )));
            }
            Err(error) => {
                return Err(port_error_to_status(PortError::invariant_violation(
                    "product.transport_encode",
                    error.to_string(),
                )));
            }
        }
    }
}

#[derive(Default)]
struct BoundedReplayJson {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl io::Write for BoundedReplayJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > LOCALE_REPLAY_JSON_LIMIT - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other(
                "Product locale replay response exceeds the byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
        LOCALE_REPLAY_JSON_LIMIT, ProductCatalogGrpcBearerAuthenticator,
        ProductCatalogGrpcBearerInterceptor, ProductCatalogGrpcOperation,
        TrustedProductCatalogAuthority, locale_replay_page_response, port_error_to_status,
        trusted_context,
    };
    use rustok_api::{PortActor, PortContext, PortError};
    use tonic::metadata::MetadataValue;
    use tonic::service::Interceptor;
    use tonic::{Code, Request};
    use uuid::Uuid;

    use crate::grpc::auth::{AUTHORIZATION_METADATA, TENANT_ID_METADATA};

    fn locale_replay_entry(id: u128, description: String) -> crate::ProductLocaleReplayEntry {
        serde_json::from_value(serde_json::json!({
            "tenant_id": Uuid::from_u128(1),
            "product_id": Uuid::from_u128(id),
            "source_version": 1,
            "product_source_version": 1,
            "locale": "en",
            "state": {"live": {
                "status": "draft", "title": "Product", "handle": "product",
                "description": description, "seller_id": null, "vendor": null,
                "product_type": null, "primary_category_id": null, "tag_ids": [],
                "created_at": "2026-11-22T00:00:00Z", "published_at": null,
                "attribute_terms": [], "variant_ids": [], "sales_channel_ids": []
            }}
        }))
        .unwrap()
    }

    #[test]
    fn locale_replay_page_preserves_continuation_when_shortened_by_encoded_size() {
        let entries: Vec<_> = (1..=10)
            .map(|id| locale_replay_entry(id, "\"\\\n".repeat(128 * 1024)))
            .collect();
        let response = locale_replay_page_response(crate::ProductLocaleReplayPage {
            entries: entries.clone(),
            next: None,
        })
        .unwrap()
        .into_inner();
        assert!(response.output_json.len() <= LOCALE_REPLAY_JSON_LIMIT);
        let page: crate::ProductLocaleReplayPage =
            serde_json::from_slice(&response.output_json).unwrap();
        assert!(!page.entries.is_empty());
        assert!(page.entries.len() < entries.len());
        assert_eq!(page.next, page.entries.last().map(|entry| entry.key()));
        assert_eq!(
            serde_json::to_value(&page.entries).unwrap(),
            serde_json::to_value(&entries[..page.entries.len()]).unwrap()
        );
    }

    #[test]
    fn locale_replay_page_preserves_owner_continuation_and_empty_terminal_page() {
        let entry = locale_replay_entry(1, "описание".to_owned());
        let next = Some(entry.key());
        let response = locale_replay_page_response(crate::ProductLocaleReplayPage {
            entries: vec![entry],
            next: next.clone(),
        })
        .unwrap()
        .into_inner();
        let page: crate::ProductLocaleReplayPage =
            serde_json::from_slice(&response.output_json).unwrap();
        assert_eq!(page.next, next);
        let response = locale_replay_page_response(crate::ProductLocaleReplayPage {
            entries: Vec::new(),
            next: None,
        })
        .unwrap()
        .into_inner();
        let page: crate::ProductLocaleReplayPage =
            serde_json::from_slice(&response.output_json).unwrap();
        assert!(page.entries.is_empty());
        assert!(page.next.is_none());
    }

    #[test]
    fn locale_replay_page_refuses_an_oversized_single_entry_without_retry() {
        let status = locale_replay_page_response(crate::ProductLocaleReplayPage {
            entries: vec![locale_replay_entry(1, "x".repeat(LOCALE_REPLAY_JSON_LIMIT))],
            next: None,
        })
        .unwrap_err();
        let error: PortError = serde_json::from_slice(status.details()).unwrap();
        assert_eq!(error.code, "product.locale_replay_entry_too_large");
        assert!(!error.retryable);
    }

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
    fn bearer_interceptor_accepts_every_configured_additional_credential() {
        let tenant_id = Uuid::new_v4().to_string();
        let mut interceptor = ProductCatalogGrpcBearerInterceptor::from_bearer_token(
            "catalog-read-secret",
            PortActor::service("rustok-server"),
        )
        .expect("read credential should be accepted")
        .with_additional_authenticator(
            ProductCatalogGrpcBearerAuthenticator::new(
                "attached-values-secret",
                PortActor::service("rustok-server"),
            )
            .expect("attached values credential should be accepted")
            .with_allowed_operations([ProductCatalogGrpcOperation::ReadAttachedValues]),
        )
        .with_additional_authenticator(
            ProductCatalogGrpcBearerAuthenticator::new(
                "variant-replay-secret",
                PortActor::service("product-index"),
            )
            .expect("variant replay credential should be accepted")
            .with_allowed_operations([ProductCatalogGrpcOperation::ScanVariantReplay]),
        )
        .with_additional_authenticator(
            ProductCatalogGrpcBearerAuthenticator::new(
                "locale-replay-secret",
                PortActor::service("product-index"),
            )
            .expect("locale replay credential should be accepted")
            .with_allowed_operations([ProductCatalogGrpcOperation::ScanLocaleReplay]),
        );

        for (secret, operation) in [
            (
                "attached-values-secret",
                ProductCatalogGrpcOperation::ReadAttachedValues,
            ),
            (
                "variant-replay-secret",
                ProductCatalogGrpcOperation::ScanVariantReplay,
            ),
            (
                "locale-replay-secret",
                ProductCatalogGrpcOperation::ScanLocaleReplay,
            ),
        ] {
            let mut request = Request::new(());
            let authorization = format!("Bearer {secret}");
            request.metadata_mut().insert(
                AUTHORIZATION_METADATA,
                MetadataValue::try_from(authorization.as_str())
                    .expect("test bearer metadata should be valid"),
            );
            request.metadata_mut().insert(
                TENANT_ID_METADATA,
                MetadataValue::try_from(tenant_id.as_str())
                    .expect("test tenant metadata should be valid"),
            );

            let authenticated = interceptor
                .call(request)
                .expect("each configured credential should authenticate");
            let authority = authenticated
                .extensions()
                .get::<TrustedProductCatalogAuthority>()
                .expect("authenticated authority should be installed");
            assert_eq!(authority.tenant_id, tenant_id);
            assert!(
                authority.allowed_operations.contains(&operation),
                "credential must retain its own operation grant: {operation:?}",
            );
            assert_eq!(authority.allowed_operations.len(), 1);
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
    fn bearer_interceptor_rejects_nil_tenant_metadata() {
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
            MetadataValue::try_from(Uuid::nil().to_string().as_str()).unwrap(),
        );

        let status = interceptor
            .call(request)
            .expect_err("nil tenant must fail closed");
        assert_eq!(status.code(), Code::Unauthenticated);
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
