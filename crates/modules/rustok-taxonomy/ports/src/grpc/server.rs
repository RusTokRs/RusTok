use std::{collections::HashSet, sync::Arc};

use rustok_api::{
    PortActor, PortContext, PortError, SHA256_DIGEST_BYTES, fixed_work_sha256_eq, sha256_digest,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::{
    AUTHORIZATION, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, MAX_TOKEN_BYTES, authorization,
    non_nil_uuid,
    proto::{
        ReadTermNamesRequest, ReadTermNamesResponse, TermName,
        term_names_service_server::{TermNamesService, TermNamesServiceServer},
    },
    status_code, timeout, validate_context,
};
use crate::{
    MAX_TAXONOMY_TERM_NAMES_PER_CALL, ReadTaxonomyTermNamesRequest, TAXONOMY_TERM_NAMES_READ_CLAIM,
    TaxonomyTermNamesReadPort,
};

pub struct TaxonomyTermNamesGrpcService {
    owner: Arc<dyn TaxonomyTermNamesReadPort>,
    authorization_digest: [u8; SHA256_DIGEST_BYTES],
    tenants: HashSet<Uuid>,
}

impl TaxonomyTermNamesGrpcService {
    pub fn new(
        owner: Arc<dyn TaxonomyTermNamesReadPort>,
        token: &str,
        tenants: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, PortError> {
        let authorization = authorization(token)?;
        let mut allowed = HashSet::new();
        for tenant in tenants {
            if allowed.len() == 128 || tenant.is_nil() || !allowed.insert(tenant) {
                return Err(invalid_allowlist());
            }
        }
        if allowed.is_empty() {
            return Err(invalid_allowlist());
        }
        Ok(Self {
            owner,
            authorization_digest: sha256_digest(&[authorization.as_encoded_bytes()]),
            tenants: allowed,
        })
    }

    pub fn into_server(self) -> TermNamesServiceServer<Self> {
        TermNamesServiceServer::new(self)
            .max_decoding_message_size(MAX_REQUEST_BYTES)
            .max_encoding_message_size(MAX_RESPONSE_BYTES)
    }

    fn authenticate<T>(&self, request: &Request<T>) -> Result<(), Status> {
        let mut credentials = request.metadata().get_all(AUTHORIZATION).into_iter();
        let credential = credentials
            .next()
            .ok_or_else(|| Status::unauthenticated("Taxonomy service credential required"))?;
        let credential = credential.as_encoded_bytes();
        if credentials.next().is_some()
            || credential.len() > MAX_TOKEN_BYTES + "Bearer ".len()
            || !fixed_work_sha256_eq(&self.authorization_digest, &sha256_digest(&[credential]))
        {
            return Err(Status::unauthenticated(
                "Taxonomy service credential invalid",
            ));
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl TermNamesService for TaxonomyTermNamesGrpcService {
    async fn read_term_names(
        &self,
        request: Request<ReadTermNamesRequest>,
    ) -> Result<Response<ReadTermNamesResponse>, Status> {
        self.authenticate(&request)?;
        let request = request.into_inner();
        let tenant = non_nil_uuid(&request.tenant_id)
            .ok_or_else(|| Status::invalid_argument("Taxonomy tenant is invalid"))?;
        if !self.tenants.contains(&tenant) {
            return Err(Status::permission_denied(
                "Taxonomy tenant is outside the credential scope",
            ));
        }
        let duration = timeout(request.deadline_ms).map_err(port_status)?;
        validate_context(&request).map_err(port_status)?;
        if request.term_ids.len() > MAX_TAXONOMY_TERM_NAMES_PER_CALL {
            return Err(Status::invalid_argument("Taxonomy term batch is too large"));
        }
        let term_ids = request
            .term_ids
            .iter()
            .map(|value| {
                non_nil_uuid(value)
                    .ok_or_else(|| Status::invalid_argument("Taxonomy term identity is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut context = PortContext::new(
            tenant.to_string(),
            PortActor::service("product.taxonomy"),
            request.locale,
            request.correlation_id,
        )
        .with_claim(TAXONOMY_TERM_NAMES_READ_CLAIM)
        .with_deadline(duration);
        context.traceparent = request.traceparent;
        context.causation_id = request.causation_id;
        context.channel = request.channel;

        let names = tokio::time::timeout(
            duration,
            self.owner.read_term_names(
                context,
                ReadTaxonomyTermNamesRequest {
                    term_ids,
                    fallback_locale: request.fallback_locale,
                },
            ),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Taxonomy term name read timed out"))?
        .map_err(port_status)?;
        Ok(Response::new(ReadTermNamesResponse {
            names: names
                .into_iter()
                .map(|(term_id, name)| TermName {
                    term_id: term_id.to_string(),
                    name,
                })
                .collect(),
        }))
    }
}

fn invalid_allowlist() -> PortError {
    PortError::validation(
        "taxonomy.grpc_tenant_allowlist_invalid",
        "Taxonomy gRPC requires one to 128 distinct tenant IDs",
    )
}

fn port_status(error: PortError) -> Status {
    let code = status_code(&error.kind);
    let message = "Taxonomy term name read failed";
    match serde_json::to_vec(&error) {
        Ok(details) if details.len() <= 1024 => Status::with_details(code, message, details.into()),
        _ => Status::new(code, message),
    }
}
