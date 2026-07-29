use std::{collections::HashSet, sync::Arc};

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, PublishedProductsRequest,
    VariantProductProjectionRequest,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::{Code, Request, Response, Status};

use crate::proto::product_catalog_read_service_server::ProductCatalogReadService;
use crate::proto::{JsonRequest, JsonResponse};

/// Provider-side adapter. The wrapped Product owner port retains catalog policy and persistence.
pub struct ProductCatalogGrpcService<P> {
    provider: Arc<P>,
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

impl<P> ProductCatalogGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[tonic::async_trait]
impl<P> ProductCatalogReadService for ProductCatalogGrpcService<P>
where
    P: ProductCatalogReadPort + 'static,
{
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
        ProductCatalogGrpcOperation, TrustedProductCatalogAuthority, port_error_to_status,
        trusted_context,
    };
    use rustok_api::{PortActor, PortContext, PortError};
    use tonic::{Code, Request};

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
}
