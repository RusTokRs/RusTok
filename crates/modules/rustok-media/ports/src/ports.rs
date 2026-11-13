use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{
    DEFAULT_MAX_SIZE, MediaAssetReference, MediaAssetReferenceAdmission, MediaAssetReferenceInput,
    MediaImageDescriptor, MediaItem, MediaPublicImageAsset, MediaReconciliationReport,
    MediaTranslationItem, UpsertTranslationInput,
};

/// An atomically selected read/write capability from ONE owner instance. This
/// prevents consumers from combining a remote read with an embedded write port.
#[derive(Clone)]
pub struct MediaAssetPorts {
    read: Arc<dyn MediaAssetReadPort>,
    write: Arc<dyn MediaAssetWritePort>,
}

impl MediaAssetPorts {
    pub fn new<T: MediaAssetReadPort + MediaAssetWritePort + 'static>(owner: Arc<T>) -> Self {
        Self {
            read: owner.clone(),
            write: owner,
        }
    }

    pub fn from_split(
        read: Arc<dyn MediaAssetReadPort>,
        write: Arc<dyn MediaAssetWritePort>,
    ) -> Self {
        Self { read, write }
    }

    pub fn read(&self) -> Arc<dyn MediaAssetReadPort> {
        self.read.clone()
    }

    pub fn write(&self) -> Arc<dyn MediaAssetWritePort> {
        self.write.clone()
    }
}

pub const MAX_MEDIA_LIST_LIMIT: u64 = 100;
pub const MAX_MEDIA_RECONCILIATION_LIMIT: u64 = 1_000;

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
    #[serde(default)]
    pub owner_module: Option<String>,
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

/// Bounded page request for durable consumer-owned Media reference holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetReferenceListRequest {
    pub owner_module: String,
    pub after_reference_id: Option<Uuid>,
    pub limit: u64,
}

/// Bounded exact lookup for durable consumer-owned Media reference holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetReferenceLookupRequest {
    pub owner_module: String,
    pub reference_ids: Vec<Uuid>,
}

/// Result of a bounded exact lookup. Missing references are intentionally omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetReferenceLookupResult {
    pub references: Vec<MediaAssetReference>,
}

/// One bounded page of durable consumer-owned Media reference holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetReferenceListPage {
    pub references: Vec<MediaAssetReference>,
    pub next_reference_id: Option<Uuid>,
    pub has_more: bool,
}

/// Transport-neutral read boundary for media asset metadata and SEO image descriptors.
#[async_trait]
pub trait MediaAssetReadPort: Send + Sync {
    async fn get_asset(&self, context: PortContext, media_id: Uuid)
    -> Result<MediaItem, PortError>;

    async fn get_asset_reference_admission(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<MediaAssetReferenceAdmission, PortError>;

    async fn list_asset_references(
        &self,
        context: PortContext,
        request: MediaAssetReferenceListRequest,
    ) -> Result<MediaAssetReferenceListPage, PortError>;

    async fn lookup_asset_references(
        &self,
        context: PortContext,
        request: MediaAssetReferenceLookupRequest,
    ) -> Result<MediaAssetReferenceLookupResult, PortError>;

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

    async fn retain_asset_reference(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: MediaAssetReferenceInput,
    ) -> Result<MediaAssetReference, PortError>;

    async fn release_asset_reference(
        &self,
        context: PortContext,
        media_id: Uuid,
        input: MediaAssetReferenceInput,
    ) -> Result<(), PortError>;

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

/// Transport-neutral read boundary for public image assets and presentation descriptors.
#[async_trait]
pub trait MediaPublicImageReadPort: Send + Sync {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError>;
}

pub fn require_media_read_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())
}

pub fn require_media_write_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write())
}

pub fn validate_asset_reference_list_request(
    request: &MediaAssetReferenceListRequest,
) -> Result<(), PortError> {
    if !(1..=MAX_MEDIA_RECONCILIATION_LIMIT).contains(&request.limit) {
        return Err(PortError::validation(
            "media.asset_reference_list_limit_invalid",
            format!(
                "media asset reference list limit must be between 1 and {MAX_MEDIA_RECONCILIATION_LIMIT}"
            ),
        ));
    }
    if request
        .after_reference_id
        .is_some_and(|reference_id| reference_id.is_nil())
    {
        return Err(PortError::validation(
            "media.asset_reference_list_cursor_invalid",
            "media asset reference list cursor must be a non-nil UUID",
        ));
    }
    Ok(())
}

pub fn validate_asset_reference_lookup_request(
    request: &MediaAssetReferenceLookupRequest,
) -> Result<(), PortError> {
    if request.reference_ids.is_empty() {
        return Err(PortError::validation(
            "media.asset_reference_lookup_ids_empty",
            "media asset reference lookup requires at least one reference ID",
        ));
    }
    if request.reference_ids.len() > MAX_MEDIA_RECONCILIATION_LIMIT as usize {
        return Err(PortError::validation(
            "media.asset_reference_lookup_ids_too_large",
            format!(
                "media asset reference lookup is limited to {MAX_MEDIA_RECONCILIATION_LIMIT} reference IDs"
            ),
        ));
    }
    let mut seen = std::collections::HashSet::with_capacity(request.reference_ids.len());
    for reference_id in &request.reference_ids {
        if reference_id.is_nil() {
            return Err(PortError::validation(
                "media.asset_reference_lookup_id_invalid",
                "media asset reference lookup IDs must be non-nil UUIDs",
            ));
        }
        if !seen.insert(*reference_id) {
            return Err(PortError::validation(
                "media.asset_reference_lookup_duplicate_id",
                "media asset reference lookup IDs must be unique",
            ));
        }
    }
    Ok(())
}

pub fn validate_media_list_limit(limit: u64) -> Result<(), PortError> {
    if !(1..=MAX_MEDIA_LIST_LIMIT).contains(&limit) {
        return Err(PortError::validation(
            "media.list_limit_invalid",
            format!("media list limit must be between 1 and {MAX_MEDIA_LIST_LIMIT}"),
        ));
    }
    Ok(())
}

pub fn validate_upload_request(request: &MediaUploadRequest) -> Result<(), PortError> {
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

pub fn validate_asset_reference_input(input: &MediaAssetReferenceInput) -> Result<(), PortError> {
    if input.reference_id.is_nil() {
        return Err(PortError::validation(
            "media.invalid_asset_reference_id",
            "media asset reference_id must be a non-nil UUID",
        ));
    }
    Ok(())
}

pub fn validate_reconciliation_request(
    request: &MediaReconciliationRequest,
) -> Result<(), PortError> {
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

pub fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "media.invalid_tenant_id",
            "media port context must carry a UUID tenant_id",
        )
    })?;
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "media.nil_tenant_id",
            "media port context tenant_id cannot be nil",
        ));
    }
    Ok(tenant_id)
}
