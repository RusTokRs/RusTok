use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct UploadInput {
    pub tenant_id: Uuid,
    pub uploaded_by: Option<Uuid>,
    pub original_name: String,
    pub content_type: String,
    pub data: bytes::Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
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
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

impl UpsertTranslationInput {
    /// Normalizes user-entered translation metadata at the module boundary.
    ///
    /// The media runtime accepts host-selected locales, but the stored locale key
    /// must be explicit, short, and path/header-safe because it is reused by
    /// GraphQL, REST, and admin transport adapters. Optional text fields are
    /// trimmed and empty strings are stored as `NULL` to keep read-side fallback
    /// semantics deterministic.
    pub fn normalize(self) -> std::result::Result<NormalizedTranslationInput, String> {
        let locale = normalize_locale(self.locale)?;

        Ok(NormalizedTranslationInput {
            locale,
            title: normalize_string(self.title),
            alt_text: normalize_string(self.alt_text),
            caption: normalize_string(self.caption),
        })
    }
}

fn normalize_locale(value: String) -> std::result::Result<String, String> {
    let locale = value.trim().to_ascii_lowercase().replace('_', "-");
    let valid = !locale.is_empty()
        && locale.len() <= 32
        && locale
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');

    valid.then_some(locale).ok_or(value)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTranslationItem {
    pub id: Uuid,
    pub media_id: Uuid,
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
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

pub const ALLOWED_MIME_PREFIXES: &[&str] = &["image/", "video/", "audio/", "application/pdf"];

pub const DEFAULT_MAX_SIZE: u64 = 50 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::{
        MediaImageDeliveryProfile, MediaImageDescriptor, MediaImagePublicUrlPolicy,
        UpsertTranslationInput,
    };

    #[test]
    fn upsert_translation_input_normalizes_locale_and_optional_text() {
        let normalized = UpsertTranslationInput {
            locale: " EN_us ".to_string(),
            title: Some("  Hero  ".to_string()),
            alt_text: Some("   ".to_string()),
            caption: Some("Caption".to_string()),
        }
        .normalize()
        .expect("input should normalize");

        assert_eq!(normalized.locale, "en-us");
        assert_eq!(normalized.title.as_deref(), Some("Hero"));
        assert_eq!(normalized.alt_text, None);
        assert_eq!(normalized.caption.as_deref(), Some("Caption"));
    }

    #[test]
    fn upsert_translation_input_rejects_empty_or_unsafe_locale() {
        for locale in [
            "   ",
            "en/us",
            "ru@test",
            "abcdefghijklmnopqrstuvwxyzabcdefg",
        ] {
            assert!(
                UpsertTranslationInput {
                    locale: locale.to_string(),
                    title: None,
                    alt_text: None,
                    caption: None,
                }
                .normalize()
                .is_err(),
                "locale `{locale}` should be rejected"
            );
        }
    }

    #[test]
    fn media_image_descriptor_normalizes_mime_and_derived_fields() {
        let descriptor = MediaImageDescriptor::from_parts(
            "https://cdn.example.com/assets/hero.webp?version=2",
            Some(" Hero image ".to_string()),
            Some(1200),
            Some(630),
            None,
        )
        .expect("descriptor should be created for valid URL");

        assert_eq!(descriptor.alt.as_deref(), Some("Hero image"));
        assert_eq!(descriptor.mime_type.as_deref(), Some("image/webp"));
        assert_eq!(descriptor.file_extension().as_deref(), Some("webp"));
        assert!(descriptor.has_alt());
        assert!(descriptor.has_size());
        assert_eq!(descriptor.pixel_count(), Some(756000));
        assert_eq!(descriptor.aspect_ratio(), Some(1200.0 / 630.0));
    }

    #[test]
    fn media_image_descriptor_trims_explicit_mime_and_rejects_invalid_dimensions() {
        let descriptor = MediaImageDescriptor::from_parts(
            " https://cdn.example.com/assets/photo.JPG#hero ",
            Some("   ".to_string()),
            Some(0),
            Some(-10),
            Some(" image/jpeg ".to_string()),
        )
        .expect("descriptor should keep trimmed URL with explicit mime");

        assert_eq!(
            descriptor.url,
            "https://cdn.example.com/assets/photo.JPG#hero"
        );
        assert_eq!(descriptor.alt, None);
        assert_eq!(descriptor.width, None);
        assert_eq!(descriptor.height, None);
        assert_eq!(descriptor.mime_type.as_deref(), Some("image/jpeg"));
        assert!(!descriptor.has_alt());
        assert!(!descriptor.has_size());
        assert_eq!(descriptor.pixel_count(), None);
        assert_eq!(descriptor.aspect_ratio(), None);
        assert_eq!(descriptor.file_extension().as_deref(), Some("jpg"));
    }

    #[test]
    fn media_image_descriptor_infers_mime_after_query_and_fragment_cleanup() {
        let descriptor = MediaImageDescriptor::from_parts(
            "https://cdn.example.com/assets/banner.png?signature=abc#fragment",
            None,
            Some(320),
            Some(160),
            None,
        )
        .expect("descriptor should infer mime from cleaned path");

        assert_eq!(descriptor.mime_type.as_deref(), Some("image/png"));
        assert_eq!(descriptor.file_extension().as_deref(), Some("png"));
        assert_eq!(descriptor.pixel_count(), Some(51200));
        assert_eq!(descriptor.aspect_ratio(), Some(2.0));
        assert_eq!(
            descriptor.delivery_profile(),
            MediaImageDeliveryProfile::AbsolutePublicUrl
        );
        assert_eq!(
            descriptor.public_url_policy(),
            MediaImagePublicUrlPolicy::DirectPublic
        );
        assert!(descriptor.is_publicly_addressable());
        assert!(descriptor.should_emit_to_public_metadata());
        assert_eq!(
            descriptor.normalized_public_url(),
            Some("https://cdn.example.com/assets/banner.png?signature=abc#fragment")
        );
    }

    #[test]
    fn media_image_descriptor_classifies_public_and_storage_delivery_profiles() {
        let root_relative =
            MediaImageDescriptor::from_parts("/media/hero.jpg", None, None, None, None)
                .expect("root-relative URL should create descriptor");
        let storage_relative =
            MediaImageDescriptor::from_parts("tenant/object.webp", None, None, None, None)
                .expect("storage path should create descriptor");
        let opaque = MediaImageDescriptor::from_parts("asset-key", None, None, None, None)
            .expect("opaque key should create descriptor");

        assert_eq!(
            root_relative.delivery_profile(),
            MediaImageDeliveryProfile::RootRelativePublicUrl
        );
        assert!(root_relative.is_publicly_addressable());
        assert_eq!(
            storage_relative.delivery_profile(),
            MediaImageDeliveryProfile::StorageRelativePath
        );
        assert_eq!(
            storage_relative.public_url_policy(),
            MediaImagePublicUrlPolicy::ProxyRequired
        );
        assert!(!storage_relative.is_publicly_addressable());
        assert!(storage_relative.requires_public_proxy());
        assert!(!storage_relative.should_emit_to_public_metadata());
        assert_eq!(storage_relative.normalized_public_url(), None);
        assert_eq!(
            opaque.delivery_profile(),
            MediaImageDeliveryProfile::OpaqueReference
        );
        assert_eq!(
            opaque.public_url_policy(),
            MediaImagePublicUrlPolicy::NotAddressable
        );
        assert!(!opaque.is_publicly_addressable());
        assert!(!opaque.requires_public_proxy());
        assert!(!opaque.should_emit_to_public_metadata());
    }

    #[test]
    fn media_image_descriptor_rejects_empty_url() {
        assert!(
            MediaImageDescriptor::from_parts("   ", None, None, None, None).is_none(),
            "empty URL should not create descriptor"
        );
    }
}
