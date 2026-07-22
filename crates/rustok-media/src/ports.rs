use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    DEFAULT_MAX_SIZE, MediaError, MediaImageDescriptor, MediaItem, MediaReconciliationReport,
    MediaService, MediaTranslationItem, PrepareUploadSessionInput, UpsertTranslationInput,
};

const MAX_MEDIA_LIST_LIMIT: u64 = 100;
const MAX_MEDIA_RECONCILIATION_LIMIT: u64 = 1_000;

/// Owner-controlled streaming upload endpoint used by the embedded Media deployment.
///
/// Upload bytes do not cross a generic port DTO. An S3-compatible provider replaces this
/// target with a Media-issued presigned upload target without changing consumer ownership.
pub const MEDIA_OWNER_STREAMING_UPLOAD_PATH: &str = "/api/media";

/// Metadata supplied before a caller sends binary data to a Media-owned upload transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUploadRequest {
    pub original_name: String,
    pub content_type: String,
    pub content_length: Option<u64>,
}

/// Transport selected and owned by Media for one upload operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaUploadTransport {
    OwnerStreamingRest,
    PresignedObjectStore,
}

/// A Media-owned target for binary upload data.
///
/// `endpoint` is only an upload destination. It does not carry a blob, storage credential, or
/// storage handle through the cross-module control contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUploadTarget {
    pub transport: MediaUploadTransport,
    pub endpoint: String,
    pub session_id: Option<Uuid>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Bounded request for owner-local storage reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaReconciliationRequest {
    pub limit: u64,
}

/// Transport-neutral read boundary for media asset metadata and SEO image descriptors.
#[async_trait]
pub trait MediaAssetReadPort: Send + Sync {
    async fn get_asset(&self, context: PortContext, media_id: Uuid)
    -> Result<MediaItem, PortError>;

    async fn list_assets(
        &self,
        context: PortContext,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64), PortError>;

    async fn get_image_descriptor(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<Option<MediaImageDescriptor>, PortError>;

    async fn get_translations(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>, PortError>;
}

/// Transport-neutral owner boundary for Media write and control operations.
///
/// The binary body of an upload stays on a Media-owned streaming REST endpoint or a presigned
/// object-store flow. gRPC is reserved for the metadata/control operations below.
#[async_trait]
pub trait MediaAssetWritePort: Send + Sync {
    async fn prepare_upload(
        &self,
        context: PortContext,
        request: MediaUploadRequest,
    ) -> Result<MediaUploadTarget, PortError>;

    async fn complete_upload(
        &self,
        context: PortContext,
        session_id: Uuid,
    ) -> Result<MediaItem, PortError>;

    async fn delete_asset(&self, context: PortContext, media_id: Uuid) -> Result<(), PortError>;

    async fn upsert_translation(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: UpsertTranslationInput,
    ) -> Result<MediaTranslationItem, PortError>;

    async fn reconcile_storage(
        &self,
        context: PortContext,
        request: MediaReconciliationRequest,
    ) -> Result<MediaReconciliationReport, PortError>;
}

#[async_trait]
impl MediaAssetReadPort for MediaService {
    async fn get_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<MediaItem, PortError> {
        require_media_read_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)
    }

    async fn list_assets(
        &self,
        context: PortContext,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<MediaItem>, u64), PortError> {
        require_media_read_policy(&context)?;
        validate_media_list_limit(limit)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.list(tenant_id, limit, offset)
            .await
            .map_err(media_error_to_port_error)
    }

    async fn get_image_descriptor(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<Option<MediaImageDescriptor>, PortError> {
        require_media_read_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        let item = self
            .get(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)?;
        Ok(MediaImageDescriptor::from_media_item(&item, alt))
    }

    async fn get_translations(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<Vec<MediaTranslationItem>, PortError> {
        require_media_read_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get_translations(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error)
    }
}

#[async_trait]
impl MediaAssetWritePort for MediaService {
    async fn prepare_upload(
        &self,
        context: PortContext,
        request: MediaUploadRequest,
    ) -> Result<MediaUploadTarget, PortError> {
        require_media_write_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        validate_upload_request(&request)?;
        let lease = match admit_write(
            self,
            &context,
            tenant_id,
            "prepare_upload",
            &serde_json::json!({ "actor": &context.actor, "request": &request }),
        )
        .await?
        {
            WriteAdmission::Run(lease) => lease,
            WriteAdmission::Replay(value) => return decode_replay(value),
            WriteAdmission::ReplayError(error) => return Err(error),
        };

        let result = if self.supports_presigned_upload() {
            let expiry = std::time::Duration::from_millis(
                context.deadline_ms.unwrap_or(600_000).clamp(1_000, 900_000),
            );
            let prepared = self
                .prepare_upload_session_with_id(
                    lease.operation_id,
                    PrepareUploadSessionInput {
                        tenant_id,
                        actor_id: Uuid::parse_str(&context.actor.id).ok(),
                        original_name: request.original_name,
                        content_type: request.content_type,
                        content_length: request.content_length,
                        expires_in: expiry,
                    },
                )
                .await
                .map_err(media_error_to_port_error);
            prepared.map(|prepared| MediaUploadTarget {
                transport: MediaUploadTransport::PresignedObjectStore,
                endpoint: prepared.endpoint,
                session_id: Some(prepared.id),
                expires_at: Some(prepared.expires_at),
            })
        } else {
            Ok(MediaUploadTarget {
                transport: MediaUploadTransport::OwnerStreamingRest,
                endpoint: MEDIA_OWNER_STREAMING_UPLOAD_PATH.to_string(),
                session_id: None,
                expires_at: None,
            })
        };
        finish_write(self, lease, result).await
    }

    async fn complete_upload(
        &self,
        context: PortContext,
        session_id: Uuid,
    ) -> Result<MediaItem, PortError> {
        require_media_write_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        let lease = match admit_write(
            self,
            &context,
            tenant_id,
            "complete_upload",
            &serde_json::json!({ "actor": &context.actor, "session_id": session_id }),
        )
        .await?
        {
            WriteAdmission::Run(lease) => lease,
            WriteAdmission::Replay(value) => return decode_replay(value),
            WriteAdmission::ReplayError(error) => return Err(error),
        };
        let result = self
            .complete_upload_session(tenant_id, session_id)
            .await
            .map_err(media_error_to_port_error);
        finish_write(self, lease, result).await
    }

    async fn delete_asset(&self, context: PortContext, media_id: Uuid) -> Result<(), PortError> {
        require_media_write_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        let lease = match admit_write(
            self,
            &context,
            tenant_id,
            "delete_asset",
            &serde_json::json!({ "actor": &context.actor, "media_id": media_id }),
        )
        .await?
        {
            WriteAdmission::Run(lease) => lease,
            WriteAdmission::Replay(value) => return decode_replay(value),
            WriteAdmission::ReplayError(error) => return Err(error),
        };
        let result = self
            .delete(tenant_id, media_id)
            .await
            .map_err(media_error_to_port_error);
        finish_write(self, lease, result).await
    }

    async fn upsert_translation(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: UpsertTranslationInput,
    ) -> Result<MediaTranslationItem, PortError> {
        require_media_write_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        let lease = match admit_write(
            self,
            &context,
            tenant_id,
            "upsert_translation",
            &serde_json::json!({ "actor": &context.actor, "media_id": media_id, "input": &input }),
        )
        .await?
        {
            WriteAdmission::Run(lease) => lease,
            WriteAdmission::Replay(value) => return decode_replay(value),
            WriteAdmission::ReplayError(error) => return Err(error),
        };
        let result = self
            .upsert_translation(tenant_id, media_id, input)
            .await
            .map_err(media_error_to_port_error);
        finish_write(self, lease, result).await
    }

    async fn reconcile_storage(
        &self,
        context: PortContext,
        request: MediaReconciliationRequest,
    ) -> Result<MediaReconciliationReport, PortError> {
        require_media_write_policy(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        validate_reconciliation_request(&request)?;
        let lease = match admit_write(
            self,
            &context,
            tenant_id,
            "reconcile_storage",
            &serde_json::json!({ "actor": &context.actor, "request": &request }),
        )
        .await?
        {
            WriteAdmission::Run(lease) => lease,
            WriteAdmission::Replay(value) => return decode_replay(value),
            WriteAdmission::ReplayError(error) => return Err(error),
        };
        let result = self
            .reconcile_storage(tenant_id, request.limit)
            .await
            .map_err(media_error_to_port_error);
        finish_write(self, lease, result).await
    }
}

enum WriteAdmission {
    Run(crate::idempotency::OperationLease),
    Replay(serde_json::Value),
    ReplayError(PortError),
}

async fn admit_write<T: Serialize>(
    service: &MediaService,
    context: &PortContext,
    tenant_id: Uuid,
    operation: &str,
    request: &T,
) -> Result<WriteAdmission, PortError> {
    let key = context.idempotency_key.as_deref().unwrap_or_default();
    match crate::idempotency::admit(service.database(), tenant_id, key, operation, request).await? {
        crate::idempotency::Admission::Run(lease) => Ok(WriteAdmission::Run(lease)),
        crate::idempotency::Admission::Replay(value) => Ok(WriteAdmission::Replay(value)),
        crate::idempotency::Admission::ReplayError(error) => Ok(WriteAdmission::ReplayError(error)),
    }
}

async fn finish_write<T: Serialize>(
    service: &MediaService,
    lease: crate::idempotency::OperationLease,
    result: Result<T, PortError>,
) -> Result<T, PortError> {
    match result {
        Ok(value) => {
            crate::idempotency::complete(service.database(), lease, &value).await?;
            Ok(value)
        }
        Err(error) => {
            if let Err(receipt_error) =
                crate::idempotency::fail(service.database(), lease, &error).await
            {
                tracing::error!(
                    operation_id = %lease.operation_id,
                    error = %receipt_error.message,
                    "Failed to persist Media port failure receipt"
                );
            }
            Err(error)
        }
    }
}

fn decode_replay<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("media.idempotency_receipt_corrupt", error.to_string())
    })
}

fn validate_media_list_limit(limit: u64) -> Result<(), PortError> {
    if !(1..=MAX_MEDIA_LIST_LIMIT).contains(&limit) {
        return Err(PortError::validation(
            "media.list_limit_invalid",
            format!("media list limit must be between 1 and {MAX_MEDIA_LIST_LIMIT}"),
        ));
    }
    Ok(())
}

fn require_media_read_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())
}

fn require_media_write_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write())
}

fn validate_upload_request(request: &MediaUploadRequest) -> Result<(), PortError> {
    if request.original_name.trim().is_empty() {
        return Err(PortError::validation(
            "media.upload_name_empty",
            "media upload request requires an original file name",
        ));
    }
    if request.content_type.trim().is_empty() {
        return Err(PortError::validation(
            "media.upload_content_type_empty",
            "media upload request requires a declared content type",
        ));
    }
    if request.content_length == Some(0) {
        return Err(PortError::validation(
            "media.upload_content_empty",
            "media upload request must not declare an empty body",
        ));
    }
    if request
        .content_length
        .is_some_and(|size| size > DEFAULT_MAX_SIZE)
    {
        return Err(PortError::validation(
            "media.upload_content_too_large",
            format!("media upload content exceeds the {DEFAULT_MAX_SIZE}-byte limit"),
        ));
    }
    Ok(())
}

fn validate_reconciliation_request(request: &MediaReconciliationRequest) -> Result<(), PortError> {
    if !(1..=MAX_MEDIA_RECONCILIATION_LIMIT).contains(&request.limit) {
        return Err(PortError::validation(
            "media.reconciliation_limit_invalid",
            format!(
                "media reconciliation limit must be between 1 and {MAX_MEDIA_RECONCILIATION_LIMIT}"
            ),
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "media.invalid_tenant_id",
            "media port context must carry a UUID tenant_id",
        )
    })
}

fn media_error_to_port_error(error: MediaError) -> PortError {
    match error {
        MediaError::NotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "media.not_found",
            format!("media asset not found: {id}"),
            false,
        ),
        MediaError::Forbidden => PortError::new(
            PortErrorKind::Forbidden,
            "media.forbidden",
            "media access denied",
            false,
        ),
        MediaError::UnsupportedMimeType(content_type) => PortError::validation(
            "media.unsupported_mime_type",
            format!("unsupported media type: {content_type}"),
        ),
        MediaError::InvalidMediaContent { declared, reason } => PortError::validation(
            "media.invalid_content",
            format!("media content does not match declared type {declared}: {reason}"),
        ),
        MediaError::FileTooLarge { size, max } => PortError::validation(
            "media.file_too_large",
            format!("file too large: {size} bytes; max {max} bytes"),
        ),
        MediaError::InvalidLocale(locale) => {
            PortError::validation("media.invalid_locale", format!("invalid locale: {locale}"))
        }
        MediaError::InvalidRenditionPurpose(purpose) => PortError::validation(
            "media.invalid_rendition_purpose",
            format!("invalid rendition purpose: {purpose}"),
        ),
        MediaError::RenditionInProgress(id) => PortError::conflict(
            "media.rendition_in_progress",
            format!("rendition is already being processed: {id}"),
        ),
        MediaError::UploadSessionExpired(id) => PortError::conflict(
            "media.upload_session_expired",
            format!("upload session has expired: {id}"),
        ),
        MediaError::PresignedUploadUnavailable => PortError::unavailable(
            "media.presigned_upload_unavailable",
            "presigned upload is unavailable for the configured storage backend",
        ),
        MediaError::ImageProcessing(source) => {
            PortError::validation("media.image_processing", source.to_string())
        }
        MediaError::Json(source) => {
            PortError::unavailable("media.json_encoding", source.to_string())
        }
        MediaError::Storage(source) => PortError::unavailable("media.storage", source.to_string()),
        MediaError::StorageKey(source) => {
            PortError::unavailable("media.storage_key", source.to_string())
        }
        MediaError::Db(source) => PortError::unavailable("media.database", source.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortContext, PortErrorKind};
    use uuid::Uuid;

    use super::{
        MEDIA_OWNER_STREAMING_UPLOAD_PATH, MediaReconciliationRequest, MediaUploadRequest,
        MediaUploadTransport, media_error_to_port_error, parse_tenant_id,
        require_media_read_policy, require_media_write_policy, validate_reconciliation_request,
        validate_upload_request,
    };
    use crate::MediaError;

    fn context(tenant_id: impl Into<String>) -> PortContext {
        PortContext::new(
            tenant_id,
            PortActor::service("media-port-test"),
            "en",
            "corr-1",
        )
        .with_deadline(Duration::from_secs(1))
    }

    #[test]
    fn require_media_read_policy_requires_deadline_but_not_idempotency() {
        let without_deadline = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("media-port-test"),
            "en",
            "corr-1",
        );

        let error = require_media_read_policy(&without_deadline)
            .expect_err("read port calls must carry a deadline");
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");

        assert!(require_media_read_policy(&context(Uuid::new_v4().to_string())).is_ok());
    }

    #[test]
    fn require_media_write_policy_requires_deadline_and_idempotency_key() {
        let without_write_metadata = context(Uuid::new_v4().to_string());
        let error = require_media_write_policy(&without_write_metadata)
            .expect_err("write port calls require an idempotency key");
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "port.idempotency_key_required");

        let write_context = without_write_metadata.with_idempotency_key("media-write-1");
        assert!(require_media_write_policy(&write_context).is_ok());
    }

    #[test]
    fn upload_control_validation_keeps_blobs_out_of_the_port_contract() {
        let target = MediaUploadTransport::OwnerStreamingRest;
        assert_eq!(target, MediaUploadTransport::OwnerStreamingRest);
        assert_eq!(MEDIA_OWNER_STREAMING_UPLOAD_PATH, "/api/media");

        assert!(
            validate_upload_request(&MediaUploadRequest {
                original_name: "hero.webp".to_string(),
                content_type: "image/webp".to_string(),
                content_length: Some(1024),
            })
            .is_ok()
        );

        let error = validate_upload_request(&MediaUploadRequest {
            original_name: " ".to_string(),
            content_type: "image/webp".to_string(),
            content_length: None,
        })
        .expect_err("empty file name must fail before transport selection");
        assert_eq!(error.code, "media.upload_name_empty");
    }

    #[test]
    fn reconciliation_control_validation_bounds_tenant_scoped_work() {
        assert!(
            validate_reconciliation_request(&MediaReconciliationRequest { limit: 100 }).is_ok()
        );
        let error = validate_reconciliation_request(&MediaReconciliationRequest { limit: 0 })
            .expect_err("zero cleanup limit must fail");
        assert_eq!(error.code, "media.reconciliation_limit_invalid");
    }

    #[test]
    fn parse_tenant_id_accepts_uuid_context_values() {
        let tenant_id = Uuid::new_v4();

        assert_eq!(
            parse_tenant_id(&context(tenant_id.to_string())).expect("tenant UUID should parse"),
            tenant_id
        );
    }

    #[test]
    fn parse_tenant_id_rejects_non_uuid_context_values_as_validation_errors() {
        let error = parse_tenant_id(&context("tenant-slug")).expect_err("tenant slug must fail");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "media.invalid_tenant_id");
        assert!(!error.retryable);
    }

    #[test]
    fn media_error_to_port_error_preserves_not_found_and_forbidden_semantics() {
        let id = Uuid::new_v4();
        let not_found = media_error_to_port_error(MediaError::NotFound(id));
        let forbidden = media_error_to_port_error(MediaError::Forbidden);

        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "media.not_found");
        assert!(!not_found.retryable);
        assert!(not_found.message.contains(&id.to_string()));

        assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
        assert_eq!(forbidden.code, "media.forbidden");
        assert!(!forbidden.retryable);
    }

    #[test]
    fn media_error_to_port_error_maps_policy_errors_to_non_retryable_validation() {
        let unsupported =
            media_error_to_port_error(MediaError::UnsupportedMimeType("text/html".to_string()));
        let oversized = media_error_to_port_error(MediaError::FileTooLarge { size: 12, max: 10 });
        let invalid_locale =
            media_error_to_port_error(MediaError::InvalidLocale("bad/locale".to_string()));

        for error in [unsupported, oversized, invalid_locale] {
            assert_eq!(error.kind, PortErrorKind::Validation);
            assert!(error.code.starts_with("media."));
            assert!(!error.retryable);
        }
    }

    #[test]
    fn media_error_to_port_error_marks_storage_and_database_failures_retryable() {
        let storage =
            media_error_to_port_error(MediaError::Storage(object_store::Error::Generic {
                store: "test",
                source: Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
            }));
        let database = media_error_to_port_error(MediaError::Db(sea_orm::DbErr::Conn(
            sea_orm::RuntimeErr::Internal("database unavailable".to_string()),
        )));

        assert_eq!(storage.kind, PortErrorKind::Unavailable);
        assert_eq!(storage.code, "media.storage");
        assert!(storage.retryable);

        assert_eq!(database.kind, PortErrorKind::Unavailable);
        assert_eq!(database.code, "media.database");
        assert!(database.retryable);
    }
}
