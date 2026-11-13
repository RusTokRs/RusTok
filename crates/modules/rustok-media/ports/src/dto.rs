use chrono::{DateTime, Utc};
use rustok_api::TenantLocale;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_MAX_SIZE: u64 = 50 * 1024 * 1024;
pub const ALLOWED_MIME_PREFIXES: &[&str] = &["image/", "video/", "audio/", "application/pdf"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub owner_module: String,
    pub uploaded_by: Option<Uuid>,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_path: String,
    pub storage_driver: String,
    pub public_url: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaAssetReferenceAdmissionState {
    Admitted,
    DeletePending,
    Deleted,
    Failed,
    NotReady,
}

impl MediaAssetReferenceAdmissionState {
    pub const fn is_admitted(self) -> bool {
        matches!(self, Self::Admitted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaAssetReferenceAdmission {
    pub media_id: Uuid,
    pub tenant_id: Uuid,
    pub state: MediaAssetReferenceAdmissionState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaAssetReferenceInput {
    pub owner_module: String,
    pub reference_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaAssetReference {
    pub media_id: Uuid,
    pub tenant_id: Uuid,
    pub owner_module: String,
    pub reference_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTranslationInput {
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTranslationInput {
    pub locale: TenantLocale,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

impl UpsertTranslationInput {
    pub fn normalize(self) -> Result<NormalizedTranslationInput, String> {
        let locale = TenantLocale::new(&self.locale).map_err(|_| self.locale.clone())?;

        Ok(NormalizedTranslationInput {
            locale,
            title: normalize_string(self.title),
            alt_text: normalize_string(self.alt_text),
            caption: normalize_string(self.caption),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaTranslationItem {
    pub id: Uuid,
    pub media_id: Uuid,
    pub locale: String,
    pub revision: i64,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum MediaAssetKind {
    Image,
    Video,
    Audio,
    Document,
    Binary,
}

impl MediaAssetKind {
    pub fn from_mime_type(mime_type: &str) -> Self {
        let normalized = mime_type.trim().to_ascii_lowercase();
        if normalized.starts_with("image/") {
            Self::Image
        } else if normalized.starts_with("video/") {
            Self::Video
        } else if normalized.starts_with("audio/") {
            Self::Audio
        } else if normalized == "application/pdf" {
            Self::Document
        } else {
            Self::Binary
        }
    }

    pub fn is_streamable(self) -> bool {
        matches!(self, Self::Video | Self::Audio)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum MediaAssetUsageProfile {
    PublicImageMetadata,
    PublicDownload,
    EmbeddedPlayer,
    InternalOnly,
}

impl MediaAssetUsageProfile {
    pub fn for_asset(kind: MediaAssetKind, image: Option<&MediaImageDescriptor>) -> Self {
        match kind {
            MediaAssetKind::Image
                if image.is_some_and(MediaImageDescriptor::should_emit_to_public_metadata) =>
            {
                Self::PublicImageMetadata
            }
            MediaAssetKind::Image => Self::PublicDownload,
            MediaAssetKind::Video | MediaAssetKind::Audio => Self::EmbeddedPlayer,
            MediaAssetKind::Document => Self::PublicDownload,
            MediaAssetKind::Binary => Self::InternalOnly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaAssetSummary {
    pub id: Uuid,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub public_url: String,
    pub kind: MediaAssetKind,
    pub usage_profile: MediaAssetUsageProfile,
    pub image: Option<MediaImageDescriptor>,
}

impl MediaAssetSummary {
    pub fn from_media_item(item: &MediaItem, alt: Option<String>) -> Self {
        let kind = MediaAssetKind::from_mime_type(item.mime_type.as_str());
        let image = (kind == MediaAssetKind::Image)
            .then(|| MediaImageDescriptor::from_media_item(item, alt))
            .flatten();
        let usage_profile = MediaAssetUsageProfile::for_asset(kind, image.as_ref());

        Self {
            id: item.id,
            filename: item.filename.clone(),
            original_name: item.original_name.clone(),
            mime_type: item.mime_type.clone(),
            size: item.size,
            public_url: item.public_url.clone(),
            kind,
            usage_profile,
            image,
        }
    }

    pub fn is_public_metadata_ready(&self) -> bool {
        self.usage_profile == MediaAssetUsageProfile::PublicImageMetadata
    }

    pub fn requires_public_proxy(&self) -> bool {
        self.image
            .as_ref()
            .is_some_and(MediaImageDescriptor::requires_public_proxy)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum MediaImageDeliveryProfile {
    AbsolutePublicUrl,
    RootRelativePublicUrl,
    StorageRelativePath,
    OpaqueReference,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum MediaImagePublicUrlPolicy {
    DirectPublic,
    ProxyRequired,
    NotAddressable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct MediaImageDescriptor {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

impl MediaImageDescriptor {
    pub fn from_parts(
        url: impl Into<String>,
        alt: Option<String>,
        width: Option<i32>,
        height: Option<i32>,
        mime_type: Option<String>,
    ) -> Option<Self> {
        let url = normalize_string(Some(url.into()))?;
        let width = normalize_dimension(width);
        let height = normalize_dimension(height);
        let mime_type = normalize_string(mime_type).or_else(|| infer_mime_type(url.as_str()));

        Some(Self {
            url,
            alt: normalize_string(alt),
            width,
            height,
            mime_type,
        })
    }

    pub fn from_media_item(item: &MediaItem, alt: Option<String>) -> Option<Self> {
        Self::from_parts(
            item.public_url.clone(),
            alt,
            item.width,
            item.height,
            Some(item.mime_type.clone()),
        )
    }

    pub fn has_alt(&self) -> bool {
        self.alt
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    }

    pub fn has_size(&self) -> bool {
        self.width.is_some() && self.height.is_some()
    }

    pub fn pixel_count(&self) -> Option<i64> {
        let width = self.width?;
        let height = self.height?;
        Some(i64::from(width) * i64::from(height))
    }

    pub fn aspect_ratio(&self) -> Option<f64> {
        let width = f64::from(self.width?);
        let height = f64::from(self.height?);
        if height <= 0.0 {
            return None;
        }
        Some(width / height)
    }

    pub fn file_extension(&self) -> Option<String> {
        file_extension(self.url.as_str())
    }

    pub fn delivery_profile(&self) -> MediaImageDeliveryProfile {
        delivery_profile(self.url.as_str())
    }

    pub fn is_publicly_addressable(&self) -> bool {
        matches!(
            self.delivery_profile(),
            MediaImageDeliveryProfile::AbsolutePublicUrl
                | MediaImageDeliveryProfile::RootRelativePublicUrl
        )
    }

    pub fn public_url_policy(&self) -> MediaImagePublicUrlPolicy {
        public_url_policy(self.delivery_profile())
    }

    pub fn requires_public_proxy(&self) -> bool {
        self.public_url_policy() == MediaImagePublicUrlPolicy::ProxyRequired
    }

    pub fn should_emit_to_public_metadata(&self) -> bool {
        self.public_url_policy() == MediaImagePublicUrlPolicy::DirectPublic
    }

    pub fn normalized_public_url(&self) -> Option<&str> {
        self.should_emit_to_public_metadata()
            .then_some(self.url.as_str())
    }

    pub fn proxy_path(&self, prefix: &str) -> Option<String> {
        if !self.requires_public_proxy() {
            return None;
        }

        Some(format!(
            "{}/{}",
            prefix.trim_end_matches('/'),
            self.url.trim_start_matches('/')
        ))
    }
}

fn delivery_profile(url: &str) -> MediaImageDeliveryProfile {
    let trimmed = url.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        MediaImageDeliveryProfile::AbsolutePublicUrl
    } else if trimmed.starts_with('/') {
        MediaImageDeliveryProfile::RootRelativePublicUrl
    } else if trimmed.contains('/') {
        MediaImageDeliveryProfile::StorageRelativePath
    } else {
        MediaImageDeliveryProfile::OpaqueReference
    }
}

fn public_url_policy(profile: MediaImageDeliveryProfile) -> MediaImagePublicUrlPolicy {
    match profile {
        MediaImageDeliveryProfile::AbsolutePublicUrl
        | MediaImageDeliveryProfile::RootRelativePublicUrl => {
            MediaImagePublicUrlPolicy::DirectPublic
        }
        MediaImageDeliveryProfile::StorageRelativePath => MediaImagePublicUrlPolicy::ProxyRequired,
        MediaImageDeliveryProfile::OpaqueReference => MediaImagePublicUrlPolicy::NotAddressable,
    }
}

fn normalize_string(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn normalize_dimension(value: Option<i32>) -> Option<i32> {
    value.filter(|value| *value > 0)
}

fn infer_mime_type(url: &str) -> Option<String> {
    let path = url.split('#').next().unwrap_or(url);
    let path = path.split('?').next().unwrap_or(path);
    mime_guess::from_path(path)
        .first_raw()
        .map(ToOwned::to_owned)
}

fn file_extension(url: &str) -> Option<String> {
    let path = url.split('#').next().unwrap_or(url);
    let path = path.split('?').next().unwrap_or(path);
    std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedUploadSession {
    pub id: Uuid,
    pub endpoint: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPublicImageAsset {
    pub asset: MediaItem,
    pub descriptor: Option<MediaImageDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MediaReconciliationReport {
    pub inspected: u64,
    pub healthy: u64,
    pub missing_marked: u64,
    pub deletions_completed: u64,
    pub upload_sessions_inspected: u64,
    pub upload_sessions_expired: u64,
    pub staging_objects_deleted: u64,
    pub retry_later: u64,
}

