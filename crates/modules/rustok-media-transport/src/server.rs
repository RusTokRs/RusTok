use std::{collections::HashSet, sync::Arc};

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_media::{
    MediaAssetReadPort, MediaAssetReferenceInput, MediaAssetWritePort, MediaPublicImageReadPort,
    MediaReconciliationRequest, MediaUploadRequest, UpsertTranslationInput,
};
use serde::{Serialize, de::DeserializeOwned};
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use crate::proto::media_service_server::MediaService;
use crate::proto::{
    EmptyResponse, IdJsonRequest, IdRequest, ImageDescriptorRequest, JsonRequest, JsonResponse,
    ListRequest,
};

/// Provider-side adapter. The wrapped Media providers retain all policy,
/// persistence, object lifecycle, URL selection, and binary transport ownership.
pub struct MediaGrpcService<P> {
    provider: Arc<P>,
    public_image_provider: Option<Arc<dyn MediaPublicImageReadPort>>,
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
    GetAssetReferenceAdmission,
    ListAssetReferences,
    LookupAssetReferences,
    ListAssets,
    GetImageDescriptor,
    GetPublicImageAsset,
    GetTranslations,
    PrepareUpload,
    CompleteUpload,
    DeleteAsset,
    RetainAssetReference,
    ReleaseAssetReference,
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
        let input: rustok_media::MediaAssetReferenceLookupRequest =
            decode_input(&request.input_json)?;
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
        let input: rustok_media::MediaAssetReferenceListRequest =
            decode_input(&request.input_json)?;
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
    if authority.actor.id.trim().is_empty() {
        return Err(Status::unauthenticated(
            "trusted media authority actor id cannot be empty",
        ));
    }
    if claimed.tenant_id.trim().is_empty()
        || claimed.tenant_id.trim() != authority.tenant_id.trim()
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
    use super::{
        MediaGrpcOperation, MediaGrpcService, TrustedMediaAuthority, parse_id, port_error_to_status,
        trusted_context,
    };
    use crate::proto::{ImageDescriptorRequest, media_service_server::MediaService};
    use async_trait::async_trait;
    use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
    use std::sync::Arc;
    use tonic::{Code, Request};
    use uuid::Uuid;

    struct DummyProvider;

    #[async_trait]
    impl rustok_media::MediaAssetReadPort for DummyProvider {
        async fn get_asset(
            &self,
            _context: PortContext,
            _media_id: Uuid,
        ) -> Result<rustok_media::MediaItem, PortError> {
            Err(PortError::not_found("media.not_found", "asset not found"))
        }

        async fn get_asset_reference_admission(
            &self,
            _context: PortContext,
            _media_id: Uuid,
        ) -> Result<rustok_media::MediaAssetReferenceAdmission, PortError> {
            Err(PortError::not_found("media.not_found", "admission not found"))
        }

        async fn lookup_asset_references(
            &self,
            _context: PortContext,
            _request: rustok_media::MediaAssetReferenceLookupRequest,
        ) -> Result<rustok_media::MediaAssetReferenceLookupResult, PortError> {
            Ok(rustok_media::MediaAssetReferenceLookupResult {
                references: Vec::new(),
            })
        }

        async fn list_asset_references(
            &self,
            _context: PortContext,
            _request: rustok_media::MediaAssetReferenceListRequest,
        ) -> Result<rustok_media::MediaAssetReferenceListPage, PortError> {
            Ok(rustok_media::MediaAssetReferenceListPage {
                references: Vec::new(),
                next_reference_id: None,
                has_more: false,
            })
        }

        async fn list_assets(
            &self,
            _context: PortContext,
            _limit: u64,
            _offset: u64,
        ) -> Result<(Vec<rustok_media::MediaItem>, u64), PortError> {
            Ok((Vec::new(), 0))
        }

        async fn get_image_descriptor(
            &self,
            _context: PortContext,
            _media_id: Uuid,
            _alt: Option<String>,
        ) -> Result<Option<rustok_media::MediaImageDescriptor>, PortError> {
            Ok(None)
        }

        async fn get_translations(
            &self,
            _context: PortContext,
            _media_id: Uuid,
        ) -> Result<Vec<rustok_media::MediaTranslationItem>, PortError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl rustok_media::MediaAssetWritePort for DummyProvider {
        async fn prepare_upload(
            &self,
            _context: PortContext,
            _request: rustok_media::MediaUploadRequest,
        ) -> Result<rustok_media::MediaUploadTarget, PortError> {
            Err(PortError::unavailable("media.unavailable", "not supported"))
        }

        async fn complete_upload(
            &self,
            _context: PortContext,
            _session_id: Uuid,
        ) -> Result<rustok_media::MediaItem, PortError> {
            Err(PortError::not_found("media.not_found", "session not found"))
        }

        async fn delete_asset(
            &self,
            _context: PortContext,
            _media_id: Uuid,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn retain_asset_reference(
            &self,
            _context: PortContext,
            _media_id: Uuid,
            _input: rustok_media::MediaAssetReferenceInput,
        ) -> Result<rustok_media::MediaAssetReference, PortError> {
            Err(PortError::not_found("media.not_found", "asset not found"))
        }

        async fn release_asset_reference(
            &self,
            _context: PortContext,
            _media_id: Uuid,
            _input: rustok_media::MediaAssetReferenceInput,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn upsert_translation(
            &self,
            _context: PortContext,
            _media_id: Uuid,
            _input: rustok_media::UpsertTranslationInput,
        ) -> Result<rustok_media::MediaTranslationItem, PortError> {
            Err(PortError::not_found("media.not_found", "asset not found"))
        }

        async fn reconcile_storage(
            &self,
            _context: PortContext,
            _request: rustok_media::MediaReconciliationRequest,
        ) -> Result<rustok_media::MediaReconciliationReport, PortError> {
            Err(PortError::unavailable("media.unavailable", "not supported"))
        }
    }

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
    fn remote_context_rejects_empty_authority_tenant() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("   ", PortActor::service("verified-service"))
                .allow_operation(MediaGrpcOperation::GetAsset),
        );
        let claimed = PortContext::new("   ", PortActor::service("verified-service"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::GetAsset)
            .expect_err("empty authority tenant must be rejected");

        assert_eq!(error.code(), Code::Unauthenticated);
    }

    #[test]
    fn remote_context_rejects_empty_authority_actor() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("   "))
                .allow_operation(MediaGrpcOperation::GetAsset),
        );
        let claimed = PortContext::new("tenant-a", PortActor::service("forged"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::GetAsset)
            .expect_err("empty authority actor must be rejected");

        assert_eq!(error.code(), Code::Unauthenticated);
    }

    #[test]
    fn remote_context_rejects_tenant_mismatch() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("verified-service"))
                .allow_operation(MediaGrpcOperation::GetAsset),
        );
        let claimed = PortContext::new("tenant-b", PortActor::user("forged"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::GetAsset)
            .expect_err("tenant mismatch must be rejected");

        assert_eq!(error.code(), Code::PermissionDenied);
    }

    #[test]
    fn remote_context_rejects_exact_reference_lookup_without_explicit_grant() {
        let mut request = Request::new(());
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("verified-service"))
                .allow_operation(MediaGrpcOperation::GetAssetReferenceAdmission),
        );
        let claimed = PortContext::new("tenant-a", PortActor::user("forged"), "en", "corr");

        let error = trusted_context(&request, claimed, MediaGrpcOperation::LookupAssetReferences)
            .expect_err("exact owner-reference lookup requires an explicit trusted grant");

        assert_eq!(error.code(), Code::PermissionDenied);
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

        let error = trusted_context(&request, claimed, MediaGrpcOperation::GetPublicImageAsset)
            .expect_err("public image selection must require an explicit operation grant");

        assert_eq!(error.code(), Code::PermissionDenied);
    }

    #[test]
    fn parse_id_trims_and_parses_valid_uuid() {
        let id = Uuid::new_v4();
        let formatted = format!("  {id}  \n");
        let parsed = parse_id(&formatted).expect("whitespace-padded uuid should parse");
        assert_eq!(parsed, id);
    }

    #[test]
    fn parse_id_rejects_nil_uuid() {
        let status = parse_id(&Uuid::nil().to_string()).expect_err("nil uuid must be rejected");
        assert_eq!(status.code(), Code::InvalidArgument);
        let details: PortError = serde_json::from_slice(status.details()).unwrap();
        assert_eq!(details.kind, PortErrorKind::Validation);
        assert_eq!(details.code, "media.transport_nil_id");
    }

    #[test]
    fn parse_id_rejects_invalid_uuid() {
        let status = parse_id("not-a-uuid").expect_err("invalid uuid must be rejected");
        assert_eq!(status.code(), Code::InvalidArgument);
        let details: PortError = serde_json::from_slice(status.details()).unwrap();
        assert_eq!(details.kind, PortErrorKind::Validation);
        assert_eq!(details.code, "media.transport_invalid_id");
    }

    #[tokio::test]
    async fn unconfigured_public_image_provider_returns_unavailable() {
        let service = MediaGrpcService::new(Arc::new(DummyProvider));
        let mut request = Request::new(ImageDescriptorRequest {
            context_json: serde_json::to_vec(&PortContext::new(
                "tenant-a",
                PortActor::service("test"),
                "en",
                "corr",
            ))
            .unwrap(),
            id: Uuid::new_v4().to_string(),
            alt: None,
        });
        request.extensions_mut().insert(
            TrustedMediaAuthority::new("tenant-a", PortActor::service("test"))
                .allow_operation(MediaGrpcOperation::GetPublicImageAsset),
        );

        let status = service
            .get_public_image_asset(request)
            .await
            .expect_err("unconfigured public image provider must return unavailable");
        assert_eq!(status.code(), Code::Unavailable);
        let details: PortError = serde_json::from_slice(status.details()).unwrap();
        assert_eq!(details.kind, PortErrorKind::Unavailable);
        assert_eq!(details.code, "media.public_image_provider_unavailable");
    }
}
