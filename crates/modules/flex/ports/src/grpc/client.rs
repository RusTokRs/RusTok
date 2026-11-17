use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use tonic::{
    Code, Request, Status,
    metadata::{Ascii, MetadataValue},
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint},
};
use url::{Host, Url};

use super::{
    AUTHORIZATION, MAX_MESSAGE_BYTES, authorization, call_timeout, payload,
    proto::{
        ReadVersionedAttachedSchemaRequest,
        attached_schema_service_client::AttachedSchemaServiceClient,
    },
    tenant_uuid,
};
use crate::{FlexVersionedAttachedSchema, FlexVersionedAttachedSchemaReadPort};

#[derive(Clone)]
pub struct GrpcFlexAttachedSchemaProvider {
    client: AttachedSchemaServiceClient<Channel>,
    authorization: MetadataValue<Ascii>,
}

impl GrpcFlexAttachedSchemaProvider {
    /// The channel reconnects on subsequent reads after a Flex restart.
    pub fn new(
        endpoint: &str,
        token: &str,
        allow_insecure_loopback: bool,
    ) -> Result<Self, PortError> {
        Self::with_trusted_ca(endpoint, token, allow_insecure_loopback, None)
    }

    pub fn with_trusted_ca(
        endpoint: &str,
        token: &str,
        allow_insecure_loopback: bool,
        ca_pem: Option<&[u8]>,
    ) -> Result<Self, PortError> {
        let url = Url::parse(endpoint).map_err(|_| invalid_endpoint())?;
        let loopback = match url.host() {
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(invalid_endpoint());
        }
        let channel = Endpoint::from_shared(url.to_string()).map_err(|_| invalid_endpoint())?;
        let channel = match url.scheme() {
            "https" => {
                let mut tls = ClientTlsConfig::new().with_webpki_roots();
                if let Some(ca) = ca_pem {
                    tls = tls.ca_certificate(Certificate::from_pem(ca));
                }
                channel.tls_config(tls).map_err(|_| invalid_endpoint())?
            }
            "http" if allow_insecure_loopback && loopback && ca_pem.is_none() => channel,
            _ => return Err(invalid_endpoint()),
        };
        let authorization = authorization(token)?;
        Ok(Self {
            client: AttachedSchemaServiceClient::new(channel.connect_lazy())
                .max_decoding_message_size(MAX_MESSAGE_BYTES),
            authorization,
        })
    }
}

#[async_trait]
impl FlexVersionedAttachedSchemaReadPort for GrpcFlexAttachedSchemaProvider {
    async fn read_versioned_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexVersionedAttachedSchema, PortError> {
        context.require_read_semantics()?;
        let tenant = tenant_uuid(&context.tenant_id).ok_or_else(|| {
            PortError::validation("flex.tenant_id_invalid", "Flex tenant is invalid")
        })?;
        let deadline_ms = context.deadline_ms.unwrap_or_default();
        let timeout = call_timeout(deadline_ms).ok_or_else(|| {
            PortError::validation(
                "flex.grpc_deadline_invalid",
                "Flex gRPC deadline exceeds the transport limit",
            )
        })?;
        let mut request = Request::new(ReadVersionedAttachedSchemaRequest {
            tenant_id: tenant.to_string(),
            entity_type: entity_type.to_owned(),
            deadline_ms,
            correlation_id: context.correlation_id,
        });
        request.set_timeout(timeout);
        request
            .metadata_mut()
            .insert(AUTHORIZATION, self.authorization.clone());
        let mut client = self.client.clone();
        let response =
            tokio::time::timeout(timeout, client.read_versioned_attached_schema(request))
                .await
                .map_err(|_| timeout_error())?
                .map_err(status_error)?;
        let snapshot = payload::decode(response.into_inner())?;
        snapshot.verify_scope(tenant, entity_type)?;
        Ok(snapshot)
    }
}

fn invalid_endpoint() -> PortError {
    PortError::validation(
        "flex.grpc_endpoint_invalid",
        "Flex gRPC requires HTTPS or explicitly enabled loopback HTTP",
    )
}

fn timeout_error() -> PortError {
    PortError::timeout("flex.grpc_timeout", "Flex gRPC schema read timed out")
}

fn status_error(status: Status) -> PortError {
    match status.code() {
        Code::DeadlineExceeded | Code::Cancelled => timeout_error(),
        Code::Unavailable => PortError::unavailable(
            "flex.grpc_unavailable",
            "Flex gRPC schema read is unavailable",
        ),
        Code::Unauthenticated | Code::PermissionDenied => {
            PortError::forbidden("flex.grpc_forbidden", "Flex gRPC schema access denied")
        }
        Code::InvalidArgument => PortError::validation(
            "flex.grpc_request_invalid",
            "Flex gRPC schema request is invalid",
        ),
        Code::NotFound => PortError::not_found(
            "flex.grpc_schema_not_found",
            "Flex gRPC schema scope was not found",
        ),
        Code::Aborted => PortError::conflict(
            "flex.grpc_schema_conflict",
            "Flex gRPC schema read conflicted",
        ),
        _ => PortError::invariant_violation(
            "flex.grpc_schema_read_failed",
            "Flex gRPC schema could not be read safely",
        ),
    }
}
