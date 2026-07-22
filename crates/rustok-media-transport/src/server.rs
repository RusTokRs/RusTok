use std::sync::Arc;

use bytes::Bytes;
use rustok_api::{PortContext, PortError, PortErrorKind};
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
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
        let request = request.into_inner();
        let context = decode_context(&request.context_json)?;
        let input: MediaReconciliationRequest = decode_input(&request.input_json)?;
        let value = self
            .provider
            .reconcile_storage(context, input)
            .await
            .map_err(port_error_to_status)?;
        json_response(&value)
    }
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
    use super::port_error_to_status;
    use rustok_api::PortError;
    use tonic::Code;

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
}
