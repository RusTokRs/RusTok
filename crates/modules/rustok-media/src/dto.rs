use rustok_api::TenantLocale;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use rustok_media_ports::{
    ALLOWED_MIME_PREFIXES, DEFAULT_MAX_SIZE, MediaAssetKind, MediaAssetReference,
    MediaAssetReferenceAdmission, MediaAssetReferenceAdmissionState, MediaAssetReferenceInput,
    MediaAssetReferenceListPage, MediaAssetReferenceListRequest,
    MediaAssetReferenceLookupRequest, MediaAssetReferenceLookupResult, MediaAssetSummary,
    MediaAssetUsageProfile, MediaImageDeliveryProfile, MediaImageDescriptor,
    MediaImagePublicUrlPolicy, MediaItem, MediaPublicImageAsset, MediaReconciliationReport,
    MediaReconciliationRequest, MediaTranslationItem, MediaUploadRequest, MediaUploadTarget,
    MediaUploadTransport, NormalizedTranslationInput, PreparedUploadSession,
    UpsertTranslationInput,
};

pub struct UploadInput {
    pub tenant_id: Uuid,
    pub owner_module: Option<String>,
    pub uploaded_by: Option<Uuid>,
    pub original_name: String,
    pub content_type: String,
    pub data: bytes::Bytes,
}

#[derive(Debug, Clone)]
pub struct CreateRenditionInput {
    pub tenant_id: Uuid,
    pub asset_id: Uuid,
    pub purpose: String,
    pub recipe: crate::image::ImageRecipe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRenditionItem {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub source_blob_id: Uuid,
    pub result_blob_id: Uuid,
    pub purpose: String,
    pub recipe_hash: String,
    pub mime_type: String,
    pub size: i64,
    pub width: i32,
    pub height: i32,
    pub storage_path: String,
    pub public_url: String,
}

#[derive(Debug, Clone)]
pub struct PrepareUploadSessionInput {
    pub tenant_id: Uuid,
    pub owner_module: Option<String>,
    pub actor_id: Option<Uuid>,
    pub original_name: String,
    pub content_type: String,
    pub content_length: Option<u64>,
    pub expires_in: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyExactMediaTranslationInput {
    pub source_locale: TenantLocale,
    pub target: NormalizedTranslationInput,
    pub expected_resource_revision: String,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
}
