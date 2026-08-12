use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, PublishedProductsRequest,
    StorefrontProductList, VariantProductProjectionRequest, dto::ProductResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};
use uuid::Uuid;

use crate::auth::{AUTHORIZATION_METADATA, TENANT_ID_METADATA};
use crate::proto::JsonRequest;
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
}

#[async_trait]
impl ProductCatalogReadPort for GrpcProductCatalogReadProvider {
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
    use super::{grpc_request, status_to_port_error};
    use crate::ProductCatalogGrpcBearerToken;
    use crate::auth::{AUTHORIZATION_METADATA, TENANT_ID_METADATA};
    use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
    use tonic::{Code, Status};
    use uuid::Uuid;

    #[test]
    fn typed_error_details_override_lossy_grpc_status_mapping() {
        let expected = PortError::validation("product.exact", "exact owner error");
        let status = Status::with_details(
            Code::InvalidArgument,
            expected.message.clone(),
            serde_json::to_vec(&expected).unwrap().into(),
        );
        assert_eq!(status_to_port_error(status), expected);
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
