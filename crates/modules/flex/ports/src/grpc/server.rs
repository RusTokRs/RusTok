use std::{collections::HashSet, sync::Arc};

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::{
    AUTHORIZATION, MAX_MESSAGE_BYTES, authorization, call_timeout, payload,
    proto::{
        AttachedSchemaSnapshot, ReadVersionedAttachedSchemaRequest,
        attached_schema_service_server::{AttachedSchemaService, AttachedSchemaServiceServer},
    },
    tenant_uuid,
};
use crate::{FlexVersionedAttachedSchemaReadPort, PRODUCT_ENTITY_TYPE};

pub struct FlexAttachedSchemaGrpcService {
    owner: Arc<dyn FlexVersionedAttachedSchemaReadPort>,
    authorization_digest: [u8; 32],
    tenants: HashSet<Uuid>,
}

impl FlexAttachedSchemaGrpcService {
    pub fn new(
        owner: Arc<dyn FlexVersionedAttachedSchemaReadPort>,
        token: &str,
        tenants: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, PortError> {
        let authorization_digest = Sha256::digest(authorization(token)?.as_encoded_bytes()).into();
        let tenants: HashSet<_> = tenants.into_iter().collect();
        if tenants.is_empty() || tenants.len() > 128 || tenants.contains(&Uuid::nil()) {
            return Err(PortError::validation(
                "flex.grpc_snapshot_tenants_invalid",
                "Flex gRPC requires a bounded tenant allowlist",
            ));
        }
        Ok(Self {
            owner,
            authorization_digest,
            tenants,
        })
    }

    pub fn into_server(self) -> AttachedSchemaServiceServer<Self> {
        AttachedSchemaServiceServer::new(self)
            .max_decoding_message_size(4096)
            .max_encoding_message_size(MAX_MESSAGE_BYTES)
    }
}

#[tonic::async_trait]
impl AttachedSchemaService for FlexAttachedSchemaGrpcService {
    async fn read_versioned_attached_schema(
        &self,
        request: Request<ReadVersionedAttachedSchemaRequest>,
    ) -> Result<Response<AttachedSchemaSnapshot>, Status> {
        let mut credentials = request.metadata().get_all(AUTHORIZATION).into_iter();
        let credential = credentials
            .next()
            .ok_or_else(|| Status::unauthenticated("Flex service credential required"))?;
        let digest = Sha256::digest(credential.as_encoded_bytes());
        if credentials.next().is_some()
            || !bool::from(self.authorization_digest.ct_eq(digest.as_slice()))
        {
            return Err(Status::unauthenticated("Flex service credential invalid"));
        }
        let request = request.into_inner();
        let tenant = tenant_uuid(&request.tenant_id)
            .ok_or_else(|| Status::invalid_argument("Flex tenant is invalid"))?;
        if !self.tenants.contains(&tenant) || request.entity_type != PRODUCT_ENTITY_TYPE {
            return Err(Status::permission_denied("Flex schema scope denied"));
        }
        let timeout = call_timeout(request.deadline_ms)
            .ok_or_else(|| Status::invalid_argument("Flex gRPC deadline is invalid"))?;
        if request.correlation_id.len() > 128 {
            return Err(Status::invalid_argument("Flex correlation ID is too long"));
        }
        let context = PortContext::new(
            tenant.to_string(),
            PortActor::service("product.flex-schema-sync"),
            PLATFORM_FALLBACK_LOCALE,
            request.correlation_id,
        )
        .with_deadline(timeout);
        let snapshot = tokio::time::timeout(
            timeout,
            self.owner
                .read_versioned_attached_schema(context, &request.entity_type),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Flex schema read timed out"))?
        .map_err(port_status)?;
        snapshot
            .verify_scope(tenant, &request.entity_type)
            .map_err(port_status)?;
        Ok(Response::new(
            payload::encode(snapshot).map_err(port_status)?,
        ))
    }
}

fn port_status(error: PortError) -> Status {
    match error.kind {
        PortErrorKind::Validation => Status::invalid_argument("Flex schema request is invalid"),
        PortErrorKind::NotFound => Status::not_found("Flex schema scope was not found"),
        PortErrorKind::Conflict => Status::aborted("Flex schema read conflicted"),
        PortErrorKind::Forbidden => Status::permission_denied("Flex schema scope denied"),
        PortErrorKind::Unavailable => Status::unavailable("Flex schema is unavailable"),
        PortErrorKind::Timeout => Status::deadline_exceeded("Flex schema read timed out"),
        PortErrorKind::InvariantViolation => {
            Status::internal("Flex schema could not be read safely")
        }
    }
}
