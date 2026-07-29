use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, PublishedProductsRequest,
    StorefrontProductList, VariantProductProjectionRequest,
    dto::ProductResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};

use crate::proto::JsonRequest;
use crate::proto::product_catalog_read_service_client::ProductCatalogReadServiceClient;

/// Consumer-side gRPC adapter implementing the Product-owned catalog read port.
pub struct GrpcProductCatalogReadProvider {
    client: ProductCatalogReadServiceClient<Channel>,
}

impl GrpcProductCatalogReadProvider {
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            client: ProductCatalogReadServiceClient::new(channel),
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
            .read_product_projection(with_deadline(payload, &context))
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
            .read_variant_product_projection(with_deadline(payload, &context))
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
            .list_published_products(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }
}

fn with_deadline<T>(payload: T, context: &PortContext) -> Request<T> {
    let mut request = Request::new(payload);
    if let Some(deadline_ms) = context.deadline_ms.filter(|deadline| *deadline > 0) {
        request.set_timeout(Duration::from_millis(deadline_ms));
    }
    request
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
    use super::status_to_port_error;
    use rustok_api::{PortError, PortErrorKind};
    use tonic::{Code, Status};

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
}
