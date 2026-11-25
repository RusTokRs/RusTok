use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use tonic::{
    Code, Request, Status,
    metadata::{Ascii, MetadataValue},
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint},
};
use url::{Host, Url};
use uuid::Uuid;

use super::{
    AUTHORIZATION, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, authorization, non_nil_uuid,
    proto::{ReadTermNamesRequest, term_names_service_client::TermNamesServiceClient},
    status_code, timeout, validate_context,
};
use crate::{
    MAX_TAXONOMY_TERM_NAMES_PER_CALL, ReadTaxonomyTermNamesRequest, TaxonomyTermNamesReadPort,
    authorize_term_names_read,
};

#[derive(Clone)]
pub struct GrpcTaxonomyTermNamesProvider {
    client: TermNamesServiceClient<Channel>,
    authorization: MetadataValue<Ascii>,
}

impl GrpcTaxonomyTermNamesProvider {
    /// The channel connects on demand and reconnects after provider restarts.
    pub fn new(
        endpoint: &str,
        token: &str,
        allow_insecure_loopback: bool,
        ca_pem: Option<&[u8]>,
    ) -> Result<Self, PortError> {
        let url = Url::parse(endpoint).map_err(|_| invalid_endpoint())?;
        if url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(invalid_endpoint());
        }
        let loopback = match url.host() {
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        let endpoint = Endpoint::from_shared(url.to_string())
            .map_err(|_| invalid_endpoint())?
            .connect_timeout(Duration::from_secs(5));
        let endpoint = match url.scheme() {
            "https" => {
                let mut tls = ClientTlsConfig::new().with_webpki_roots();
                if let Some(ca) = ca_pem {
                    tls = tls.ca_certificate(Certificate::from_pem(ca));
                }
                endpoint.tls_config(tls).map_err(|_| invalid_endpoint())?
            }
            "http" if allow_insecure_loopback && loopback && ca_pem.is_none() => endpoint,
            _ => return Err(invalid_endpoint()),
        };
        let authorization = authorization(token)?;
        let client = TermNamesServiceClient::new(endpoint.connect_lazy())
            .max_decoding_message_size(MAX_RESPONSE_BYTES)
            .max_encoding_message_size(MAX_REQUEST_BYTES);
        Ok(Self {
            client,
            authorization,
        })
    }
}

#[async_trait]
impl TaxonomyTermNamesReadPort for GrpcTaxonomyTermNamesProvider {
    async fn read_term_names(
        &self,
        context: PortContext,
        request: ReadTaxonomyTermNamesRequest,
    ) -> Result<BTreeMap<Uuid, String>, PortError> {
        authorize_term_names_read(&context)?;
        let deadline_ms = context.deadline_ms.unwrap_or_default();
        let duration = timeout(deadline_ms)?;
        if request.term_ids.len() > MAX_TAXONOMY_TERM_NAMES_PER_CALL
            || request.term_ids.iter().any(Uuid::is_nil)
        {
            return Err(PortError::validation(
                "taxonomy.term_name_ids_invalid",
                "Taxonomy term identities are invalid",
            ));
        }
        let requested = request.term_ids.iter().copied().collect::<HashSet<_>>();
        let wire = ReadTermNamesRequest {
            tenant_id: context.tenant_id,
            locale: context.locale,
            fallback_locale: request.fallback_locale,
            term_ids: request.term_ids.iter().map(Uuid::to_string).collect(),
            correlation_id: context.correlation_id,
            traceparent: context.traceparent,
            causation_id: context.causation_id,
            channel: context.channel,
            deadline_ms,
        };
        validate_context(&wire)?;
        let mut rpc = Request::new(wire);
        rpc.metadata_mut()
            .insert(AUTHORIZATION, self.authorization.clone());
        rpc.set_timeout(duration);
        let mut client = self.client.clone();
        let response = tokio::time::timeout(duration, client.read_term_names(rpc))
            .await
            .map_err(|_| deadline_error())?
            .map_err(port_error)?
            .into_inner();
        let mut names = BTreeMap::new();
        for entry in response.names {
            let id = non_nil_uuid(&entry.term_id)
                .filter(|id| requested.contains(id))
                .ok_or_else(invalid_response)?;
            if names.insert(id, entry.name).is_some() {
                return Err(invalid_response());
            }
        }
        Ok(names)
    }
}

fn invalid_endpoint() -> PortError {
    PortError::validation(
        "taxonomy.grpc_endpoint_invalid",
        "Taxonomy gRPC requires HTTPS or explicitly enabled IP loopback HTTP",
    )
}

fn invalid_response() -> PortError {
    PortError::invariant_violation(
        "taxonomy.grpc_response_invalid",
        "Taxonomy returned invalid term identities",
    )
}

fn deadline_error() -> PortError {
    PortError::timeout(
        "taxonomy.grpc_deadline_elapsed",
        "Taxonomy term name read timed out",
    )
}

fn port_error(status: Status) -> PortError {
    if status.details().len() <= 1024
        && let Ok(error) = serde_json::from_slice::<PortError>(status.details())
        && status_code(&error.kind) == status.code()
    {
        return error;
    }
    match status.code() {
        Code::InvalidArgument => PortError::validation(
            "taxonomy.grpc_request_invalid",
            "Taxonomy gRPC request is invalid",
        ),
        Code::PermissionDenied | Code::Unauthenticated => {
            PortError::forbidden("taxonomy.grpc_access_denied", "Taxonomy gRPC access denied")
        }
        Code::Cancelled | Code::DeadlineExceeded => deadline_error(),
        Code::Unavailable | Code::ResourceExhausted => PortError::unavailable(
            "taxonomy.grpc_unavailable",
            "Taxonomy service is unavailable",
        ),
        Code::Unimplemented => PortError::new(
            PortErrorKind::Unavailable,
            "taxonomy.grpc_operation_unsupported",
            "The configured Taxonomy provider does not serve term names",
            false,
        ),
        _ => invalid_response(),
    }
}
