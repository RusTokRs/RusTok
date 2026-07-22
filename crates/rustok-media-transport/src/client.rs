use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_media::{
    MediaAssetReadPort, MediaAssetWritePort, MediaImageDescriptor, MediaItem,
    MediaReconciliationReport, MediaReconciliationRequest, MediaTranslationItem,
    MediaUploadRequest, MediaUploadTarget, UpsertTranslationInput,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};
use uuid::Uuid;

use crate::proto::media_service_client::MediaServiceClient;
use crate::proto::{IdJsonRequest, IdRequest, ImageDescriptorRequest, JsonRequest, ListRequest};

/// Consumer-side gRPC adapter implementing the same owner ports as `MediaService`.
pub struct GrpcMediaProvider {
    client: MediaServiceClient<Channel>,
}

impl GrpcMediaProvider {
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            client: MediaServiceClient::new(channel),
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
impl MediaAssetReadPort for GrpcMediaProvider {
    async fn get_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<MediaItem, PortError> {
        let payload = IdRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
        };
        let response = self
            .client
            .clone()
            .get_asset(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn list_assets(
        &self,
        context: PortContext,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64), PortError> {
        let payload = ListRequest {
            context_json: encode(&context)?,
            limit,
            offset,
        };
        let response = self
            .client
            .clone()
            .list_assets(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn get_image_descriptor(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<Option<MediaImageDescriptor>, PortError> {
        let payload = ImageDescriptorRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
            alt,
        };
        let response = self
            .client
            .clone()
            .get_image_descriptor(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn get_translations(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>, PortError> {
        let payload = IdRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
        };
        let response = self
            .client
            .clone()
            .get_translations(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }
}

#[async_trait]
impl MediaAssetWritePort for GrpcMediaProvider {
    async fn prepare_upload(
        &self,
        context: PortContext,
        request: MediaUploadRequest,
    ) -> Result<MediaUploadTarget, PortError> {
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .prepare_upload(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn complete_upload(
        &self,
        context: PortContext,
        session_id: Uuid,
    ) -> Result<MediaItem, PortError> {
        let payload = IdRequest {
            context_json: encode(&context)?,
            id: session_id.to_string(),
        };
        let response = self
            .client
            .clone()
            .complete_upload(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn delete_asset(&self, context: PortContext, media_id: Uuid) -> Result<(), PortError> {
        let payload = IdRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
        };
        self.client
            .clone()
            .delete_asset(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?;
        Ok(())
    }

    async fn upsert_translation(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: UpsertTranslationInput,
    ) -> Result<MediaTranslationItem, PortError> {
        let payload = IdJsonRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
            input_json: encode(&input)?,
        };
        let response = self
            .client
            .clone()
            .upsert_translation(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn reconcile_storage(
        &self,
        context: PortContext,
        request: MediaReconciliationRequest,
    ) -> Result<MediaReconciliationReport, PortError> {
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .reconcile_storage(with_deadline(payload, &context))
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
        PortError::invariant_violation("media.transport_encode", error.to_string())
    })
}

fn decode<T: DeserializeOwned>(value: &[u8]) -> Result<T, PortError> {
    serde_json::from_slice(value).map_err(|error| {
        PortError::invariant_violation("media.transport_decode", error.to_string())
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
            "media.grpc.{}",
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
        let expected = PortError::validation("media.exact", "exact owner error");
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
