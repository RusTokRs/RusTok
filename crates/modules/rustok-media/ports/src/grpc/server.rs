use std::sync::Arc;

use bytes::Bytes;
use rustok_api::{PortContext, PortError, PortErrorKind};
use serde::{Serialize, de::DeserializeOwned};
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use crate::dto::{MediaAssetReferenceInput, UpsertTranslationInput};
use crate::ports::{
    MediaAssetReadPort, MediaAssetReferenceListRequest, MediaAssetReferenceLookupRequest,
    MediaAssetWritePort, MediaPublicImageReadPort, MediaReconciliationRequest, MediaUploadRequest,
};

use super::auth::{MediaGrpcOperation, TrustedMediaAuthority};
use super::proto::media_service_server::MediaService;
use super::proto::{
    EmptyResponse, IdJsonRequest, IdRequest, ImageDescriptorRequest, JsonRequest, JsonResponse,
    ListRequest,
};

/// Provider-side adapter. The wrapped Media providers retain all policy,
/// persistence, object lifecycle, URL selection, and binary transport ownership.
pub struct MediaGrpcService<P> {
    provider: Arc<P>,
    public_image_provider: Option<Arc<dyn MediaPublicImageReadPort>>,
}

impl<P> MediaGrpcService<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            public_image_provider: None,
        }
    }

    /// Explicitly attaches the Media owner provider that may select public image URLs.
    ///
    /// The provider returns metadata and an owner-selected descriptor only. Image bytes remain on
    /// the Media-owned HTTP capability endpoint and never enter this gRPC service.
    pub fn with_public_image_provider(
        mut self,
        provider: Arc<dyn MediaPublicImageReadPort>,
    ) -> Self {
        self.public_image_provider = Some(provider);
        self
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

    async fn get_asset_reference_admission(
        &self,
        request: Request<IdRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::GetAssetReferenceAdmission,
        )?;
        let request = request.into_inner();
        let value = self
            .provider
            .get_asset_reference_admission(context, parse_id(&request.id)?)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn lookup_asset_references(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::LookupAssetReferences,
        )?;
        let request = request.into_inner();
        let input: MediaAssetReferenceLookupRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .lookup_asset_references(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn list_asset_references(
        &self,
        request: Request<JsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::ListAssetReferences,
        )?;
        let request = request.into_inner();
        let input: MediaAssetReferenceListRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .list_asset_references(context, input)
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

    async fn get_public_image_asset(
        &self,
        request: Request<ImageDescriptorRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::GetPublicImageAsset,
        )?;
        let request = request.into_inner();
        let provider = self.public_image_provider.as_ref().ok_or_else(|| {
            port_error_to_status(PortError::unavailable(
                "media.public_image_provider_unavailable",
                "media public image provider is not configured",
            ))
        })?;
        let value = provider
            .get_public_image_asset(context, parse_id(&request.id)?, request.alt)
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

    async fn retain_asset_reference(
        &self,
        request: Request<IdJsonRequest>,
    ) -> Result<Response<JsonResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::RetainAssetReference,
        )?;
        let request = request.into_inner();
        let input: MediaAssetReferenceInput = decode_input(&request.input_json)?;
        let value = self
            .provider
            .retain_asset_reference(context, parse_id(&request.id)?, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }

    async fn release_asset_reference(
        &self,
        request: Request<IdJsonRequest>,
    ) -> Result<Response<EmptyResponse>, Status> {
        let context = trusted_context(
            &request,
            decode_context(&request.get_ref().context_json)?,
            MediaGrpcOperation::ReleaseAssetReference,
        )?;
        let request = request.into_inner();
        let input: MediaAssetReferenceInput = decode_input(&request.input_json)?;
        self.provider
            .release_asset_reference(context, parse_id(&request.id)?, input)
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
    if authority.tenant_id.trim().is_empty() {
        return Err(Status::unauthenticated(
            "trusted media authority tenant cannot be empty",
        ));
    }
    if let Ok(tenant_uuid) = Uuid::parse_str(authority.tenant_id.trim())
        && tenant_uuid.is_nil()
    {
        return Err(Status::unauthenticated(
            "trusted media authority tenant cannot be nil UUID",
        ));
    }
    if authority.actor.id.trim().is_empty() {
        return Err(Status::unauthenticated(
            "trusted media authority actor id cannot be empty",
        ));
    }
    if let Ok(actor_uuid) = Uuid::parse_str(authority.actor.id.trim())
        && actor_uuid.is_nil()
    {
        return Err(Status::unauthenticated(
            "trusted media authority actor id cannot be nil UUID",
        ));
    }
    if claimed.tenant_id.trim().is_empty() || claimed.tenant_id.trim() != authority.tenant_id.trim()
    {
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
    let trimmed = value.trim();
    let id = Uuid::parse_str(trimmed).map_err(|_| {
        port_error_to_status(PortError::validation(
            "media.transport_invalid_id",
            "media gRPC identifiers must be UUIDs",
        ))
    })?;
    if id.is_nil() {
        return Err(port_error_to_status(PortError::validation(
            "media.transport_nil_id",
            "media gRPC identifiers cannot be nil",
        )));
    }
    Ok(id)
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
    use super::{MediaGrpcOperation, TrustedMediaAuthority, parse_id, trusted_context};
    use rustok_api::{PortActor, PortContext};
    use tonic::Request;
    use uuid::Uuid;

    #[test]
    fn parse_id_rejects_nil_and_invalid_uuid() {
        assert!(parse_id(&Uuid::nil().to_string()).is_err());
        assert!(parse_id("not-a-uuid").is_err());
        assert!(parse_id(&Uuid::new_v4().to_string()).is_ok());
    }

    #[test]
    fn trusted_context_enforces_operation_and_tenant() {
        let tenant_id = Uuid::new_v4().to_string();
        let actor = PortActor::service("test");
        let authority = TrustedMediaAuthority::new(&tenant_id, actor.clone())
            .allow_operation(MediaGrpcOperation::GetAsset);

        let mut request = Request::new(());
        request.extensions_mut().insert(authority);

        let matching_ctx = PortContext::new(&tenant_id, actor.clone(), "en", "corr-1");
        assert!(trusted_context(&request, matching_ctx, MediaGrpcOperation::GetAsset).is_ok());

        let mismatch_ctx = PortContext::new(Uuid::new_v4().to_string(), actor, "en", "corr-1");
        assert!(trusted_context(&request, mismatch_ctx, MediaGrpcOperation::GetAsset).is_err());

        let disallowed_op_ctx =
            PortContext::new(&tenant_id, PortActor::service("test"), "en", "corr-1");
        assert!(
            trusted_context(&request, disallowed_op_ctx, MediaGrpcOperation::DeleteAsset).is_err()
        );
    }
}
