use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_media::{
    MediaAssetReadPort, MediaAssetReference, MediaAssetReferenceAdmission,
    MediaAssetReferenceInput, MediaAssetWritePort, MediaImageDescriptor, MediaItem,
    MediaPublicImageAsset, MediaPublicImageReadPort, MediaReconciliationReport,
    MediaReconciliationRequest, MediaTranslationItem, MediaUploadRequest, MediaUploadTarget,
    UpsertTranslationInput,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};
use uuid::Uuid;

use crate::proto::media_service_client::MediaServiceClient;
use crate::proto::{IdJsonRequest, IdRequest, ImageDescriptorRequest, JsonRequest, ListRequest};

/// Consumer-side gRPC adapter implementing the same owner ports as the embedded Media providers.
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
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
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

    async fn get_asset_reference_admission(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<MediaAssetReferenceAdmission, PortError> {
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
        let payload = IdRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
        };
        let response = self
            .client
            .clone()
            .get_asset_reference_admission(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn lookup_asset_references(
        &self,
        context: PortContext,
        request: rustok_media::MediaAssetReferenceLookupRequest,
    ) -> Result<rustok_media::MediaAssetReferenceLookupResult, PortError> {
        validate_context(&context)?;
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .lookup_asset_references(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn list_asset_references(
        &self,
        context: PortContext,
        request: rustok_media::MediaAssetReferenceListRequest,
    ) -> Result<rustok_media::MediaAssetReferenceListPage, PortError> {
        validate_context(&context)?;
        let payload = JsonRequest {
            context_json: encode(&context)?,
            input_json: encode(&request)?,
        };
        let response = self
            .client
            .clone()
            .list_asset_references(with_deadline(payload, &context))
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
        validate_context(&context)?;
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
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
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
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
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
impl MediaPublicImageReadPort for GrpcMediaProvider {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError> {
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
        let payload = ImageDescriptorRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
            alt,
        };
        let response = self
            .client
            .clone()
            .get_public_image_asset(with_deadline(payload, &context))
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
        validate_context(&context)?;
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
        validate_context(&context)?;
        validate_non_nil_id(session_id, "session_id")?;
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
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
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

    async fn retain_asset_reference(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: MediaAssetReferenceInput,
    ) -> Result<MediaAssetReference, PortError> {
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
        let payload = IdJsonRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
            input_json: encode(&input)?,
        };
        let response = self
            .client
            .clone()
            .retain_asset_reference(with_deadline(payload, &context))
            .await
            .map_err(status_to_port_error)?
            .into_inner();
        decode(&response.output_json)
    }

    async fn release_asset_reference(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: MediaAssetReferenceInput,
    ) -> Result<(), PortError> {
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
        let payload = IdJsonRequest {
            context_json: encode(&context)?,
            id: media_id.to_string(),
            input_json: encode(&input)?,
        };
        self.client
            .clone()
            .release_asset_reference(with_deadline(payload, &context))
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
        validate_context(&context)?;
        validate_non_nil_id(media_id, "media_id")?;
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
        validate_context(&context)?;
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

fn validate_non_nil_id(id: Uuid, field: &'static str) -> Result<(), PortError> {
    if id.is_nil() {
        return Err(PortError::validation(
            "media.transport_nil_id",
            format!("{field} cannot be nil UUID"),
        ));
    }
    Ok(())
}

fn validate_context(context: &PortContext) -> Result<(), PortError> {
    let tenant_id = context.tenant_id.trim();
    if tenant_id.is_empty() {
        return Err(PortError::forbidden(
            "media.unauthenticated",
            "tenant_id cannot be empty",
        ));
    }
    if let Ok(tenant_uuid) = Uuid::parse_str(tenant_id)
        && tenant_uuid.is_nil()
    {
        return Err(PortError::forbidden(
            "media.unauthenticated",
            "tenant_id cannot be nil UUID",
        ));
    }
    Ok(())
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
    let message = if status.message().trim().is_empty() {
        status.code().description().to_string()
    } else {
        status.message().to_string()
    };
    PortError::new(
        kind,
        format!(
            "media.grpc.{}",
            status.code().description().replace(' ', "_")
        ),
        message,
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::{status_to_port_error, validate_context, validate_non_nil_id};
    use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
    use tonic::{Code, Status};
    use uuid::Uuid;

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

    #[test]
    fn empty_status_message_falls_back_to_status_description() {
        let error = status_to_port_error(Status::new(Code::DeadlineExceeded, ""));
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(
            error.message,
            Code::DeadlineExceeded.description()
        );
        assert!(error.retryable);
    }

    #[test]
    fn permission_denied_maps_to_forbidden() {
        let error = status_to_port_error(Status::permission_denied("access denied"));
        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(
            error.code,
            format!(
                "media.grpc.{}",
                Code::PermissionDenied.description().replace(' ', "_")
            )
        );
        assert_eq!(error.message, "access denied");
        assert!(!error.retryable);
    }

    #[test]
    fn nil_uuid_and_invalid_context_are_rejected() {
        assert!(validate_non_nil_id(Uuid::nil(), "media_id").is_err());
        assert!(validate_non_nil_id(Uuid::new_v4(), "media_id").is_ok());

        let empty_ctx = PortContext::new(
            "",
            PortActor::service("test"),
            "en",
            "corr-1",
        );
        assert!(validate_context(&empty_ctx).is_err());

        let nil_tenant_ctx = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::service("test"),
            "en",
            "corr-1",
        );
        assert!(validate_context(&nil_tenant_ctx).is_err());

        let valid_ctx = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("test"),
            "en",
            "corr-1",
        );
        assert!(validate_context(&valid_ctx).is_ok());
    }
}
