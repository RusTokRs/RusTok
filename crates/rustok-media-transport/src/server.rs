use std::{collections::HashSet, sync::Arc};

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_media::{
    MediaAssetReadPort, MediaAssetWritePort, MediaReconciliationRequest, MediaUploadRequest,
    UpsertTranslationInput,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use crate::proto::media_service_server::MediaService;
use crate::proto::{
    EmptyResponse, IdJsonRequest, IdRequest, ImageDescriptorRequest, JsonRequest, JsonResponse,
    ListRequest,
};

/// Provider-side adapter. The wrapped Media provider retains all policy,
/// persistence, object lifecycle, and binary transport ownership.
pub struct MediaGrpcService<P> {
    provider: Arc<P>,
}

/// Authority established by a server-side authentication/authorization interceptor.
///
/// Network payloads may carry correlation, locale, deadline, and idempotency metadata, but
/// tenant and principal authority are always replaced with this trusted value.
#[derive(Clone, Debug)]
pub struct TrustedMediaAuthority {
    pub tenant_id: String,
    pub actor: PortActor,
    pub claims: Vec<String>,
    pub roles: Vec<String>,
    allowed_operations: HashSet<MediaGrpcOperation>,
}

/// Media operations authorized by the server-side authentication boundary.
///
/// The gRPC service rejects every request unless its operation is explicitly present in the
/// trusted authority attached by an interceptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MediaGrpcOperation {
    GetAsset,
    ListAssets,
    GetImageDescriptor,
    GetTranslations,
    PrepareUpload,
    CompleteUpload,
    DeleteAsset,
    UpsertTranslation,
    ReconcileStorage,
}

impl TrustedMediaAuthority {
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

    pub fn allow_operation(mut self, operation: MediaGrpcOperation) -> Self {
        self.allowed_operations.insert(operation);
        self
    }

    pub fn allow_operations(
        mut self,
        operations: impl IntoIterator<Item = MediaGrpcOperation>,
    ) -> Self {
        self.allowed_operations.extend(operations);
        self
    }
}

impl<P> MediaGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[tonic::async_trait]
impl<P> MediaService for MediaGrpcService<P>
where
    P: MediaAssetReadPort + MediaAssetWritePort + 'static,
{
    async fn get_asset(
        &self,
        request: Request<IdRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::GetAsset,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .get_asset(context, parse_id(&request.id)?)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn list_assets(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::ListAssets,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .list_assets(context, request.limit, request.offset)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn get_image_descriptor(
        &self,
        request: Request<ImageDescriptorRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::GetImageDescriptor,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .get_image_descriptor(context, parse_id(&request.id)?, request.alt)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn get_translations(
        &self,
        request: Request<IdRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::GetTranslations,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .get_translations(context, parse_id(&request.id)?)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn prepare_upload(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::PrepareUpload,
        )?;
        let request = request.into_inner();
        let input: MediaUploadRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .prepare_upload(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn complete_upload(
        &self,
        request: Request<IdRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::CompleteUpload,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .complete_upload(context, parse_id(&request.id)?)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn delete_asset(
        &self,
        request: Request<IdRequest>,
    ) -> Result<Response<EmptyResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::DeleteAsset,
        )?;
        let request = request.into_inner();
        self.provider
            .delete_asset(context, parse_id(&request.id)?)
            .await
            .map_err(port_error_to_status)?;
        Ok(Response::new(EmptyResponse {}))
    }

    async fn upsert_translation(
        &self,
        request: Request<IdJsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::UpsertTranslation,
        )?;
        let request = request.into_inner();
        let input: UpsertTranslationInput = decode_input(&request.input_json)?;
        let value = self
            .provider
            .upsert_translation(context, parse_id(&request.id)?, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn reconcile_storage(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::ReconcileStorage,
        )?;
        let request = request.into_inner();
        let input: MediaReconciliationRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .reconcile_storage(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }
}

fn trusted_context<T>(
    request: &Request<T>,
    mut claimed: PortContext,
    operation: MediaGrpcOperation,
) -> Result<PortContext, Status> {
    let authority = request
        .extensions()
        .get::<TrustedMediaAuthority>()
        .ok_or_else(|| Status::unauthenticated("trusted media authority is missing"))?;
    if !authority.allowed_operations.contains(&operation) {
        return Err(Status::permission_denied(
            "trusted media authority does not allow this operation",
        ));
    }
    if claimed.tenant_id != authority.tenant_id {
        return Err(Status::permission_denied(
            "media tenant does not match authenticated authority",
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
            "media.transport_invalid_json",
            error.to_string(),
        ))
    })
}

fn parse_id(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| {
        port_error_to_status(PortError::validation(
            "media.transport_invalid_id",
            "media gRPC identifiers must be UUIDs",
        ))
    })
}

fn json_response<T: Serialize>(value: &T) -> Result<Response<JsonResponse>, Status> {
    let output_json = serde_json::to_vec(value).map_err(|error| {
        port_error_to_status(PortError::invariant_violation(
            "media.transport_encode",
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
    use super::{MediaGrpcOperation, TrustedMediaAuthority, port_error_to_status, trusted_context};
    use rustok_api::{PortActor, PortContext, PortError};
    use tonic::{Code, Request};

    #[test]
    fn owner_error_is_preserved_in_status_details() {
        let error = PortError::not_found("media.not_found", "missing");
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
        let claimed = PortContext::new("tenant-a", PortActor::user("forged"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::GetAsset)
            .expect_err("authority must be required");

        assert_eq!(error.code(), Code::Unauthenticated);
    }

    #[test]
    fn remote_context_replaces_untrusted_principal_fields() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("verified-service"))
                .with_claim("media:read")
                .with_role("media-worker")
                .allow_operation(MediaGrpcOperation::GetAsset),
        );
        let claimed = PortContext::new("tenant-a", PortActor::user("forged"), "en", "corr")
            .with_claim("admin")
            .with_role("owner");

        let trusted = trusted_context(&request, claimed, MediaGrpcOperation::GetAsset)
            .expect("authority should be applied");

        assert_eq!(trusted.actor.id, "verified-service");
        assert_eq!(trusted.claims, ["media:read"]);
        assert_eq!(trusted.roles, ["media-worker"]);
    }

    #[test]
    fn remote_context_rejects_an_operation_not_authorized_by_the_server() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("verified-service"))
                .allow_operation(MediaGrpcOperation::GetAsset),
        );
        let claimed = PortContext::new("tenant-a", PortActor::user("forged"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::DeleteAsset)
            .expect_err("delete must not inherit read authorization");

        assert_eq!(error.code(), Code::PermissionDenied);
    }
}
