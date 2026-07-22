use rustok_media::MediaImageDescriptor;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub const CATEGORY_ICON_KEY_MAX_BYTES: usize = 64;
pub const CATEGORY_COVER_MAX_BYTES: i64 = 10 * 1024 * 1024;
pub const CATEGORY_COVER_MIN_DIMENSION: i32 = 64;
pub const CATEGORY_COVER_MAX_DIMENSION: i32 = 8_192;

const CATEGORY_COVER_MIME_TYPES: &[&str] = &[
    "image/avif",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
];

/// Transport-neutral Media metadata required to evaluate a category cover.
///
/// A Media adapter may construct this value from `MediaAssetReadPort` results,
/// but storage paths, drivers, credentials and blob data never enter Forum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryCoverMediaCandidate {
    pub media_id: Uuid,
    pub tenant_id: Uuid,
    pub mime_type: String,
    pub size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub descriptor: Option<MediaImageDescriptor>,
}

/// Normalize a category icon into a bounded design-system token.
///
/// Forum stores only a semantic kebab-case key. CSS classes, markup, URLs and
/// arbitrary file paths are intentionally outside this contract.
pub fn normalize_category_icon_key(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > CATEGORY_ICON_KEY_MAX_BYTES {
        return None;
    }

    let mut previous_was_separator = true;
    for character in normalized.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            previous_was_separator = false;
        } else if character == '-' && !previous_was_separator {
            previous_was_separator = true;
        } else {
            return None;
        }
    }

    (!previous_was_separator).then_some(normalized)
}

/// Validate the Media-owned metadata currently available for a category cover.
///
/// Quarantine/deletion state is not currently published by the Media read port.
/// A persistent `cover_media_id` command must remain disabled until those owner
/// states are included in the candidate produced by the Media adapter.
pub fn validate_category_cover_candidate(
    expected_tenant_id: Uuid,
    candidate: &CategoryCoverMediaCandidate,
) -> ForumResult<MediaImageDescriptor> {
    if candidate.tenant_id != expected_tenant_id {
        return Err(ForumError::Validation(
            "Category cover media belongs to another tenant".to_string(),
        ));
    }

    let mime_type = candidate.mime_type.trim().to_ascii_lowercase();
    if !CATEGORY_COVER_MIME_TYPES.contains(&mime_type.as_str()) {
        return Err(ForumError::Validation(
            "Category cover media must be a supported public image".to_string(),
        ));
    }

    if !(1..=CATEGORY_COVER_MAX_BYTES).contains(&candidate.size) {
        return Err(ForumError::Validation(format!(
            "Category cover media must be between 1 and {CATEGORY_COVER_MAX_BYTES} bytes"
        )));
    }

    let (Some(width), Some(height)) = (candidate.width, candidate.height) else {
        return Err(ForumError::Validation(
            "Category cover media requires known image dimensions".to_string(),
        ));
    };
    if !(CATEGORY_COVER_MIN_DIMENSION..=CATEGORY_COVER_MAX_DIMENSION).contains(&width)
        || !(CATEGORY_COVER_MIN_DIMENSION..=CATEGORY_COVER_MAX_DIMENSION).contains(&height)
    {
        return Err(ForumError::Validation(format!(
            "Category cover dimensions must be between {CATEGORY_COVER_MIN_DIMENSION} and {CATEGORY_COVER_MAX_DIMENSION} pixels"
        )));
    }

    let descriptor = candidate.descriptor.clone().ok_or_else(|| {
        ForumError::Validation("Category cover media has no image descriptor".to_string())
    })?;
    if !descriptor.should_emit_to_public_metadata() {
        return Err(ForumError::Validation(
            "Category cover media is not directly publicly addressable".to_string(),
        ));
    }
    if descriptor.mime_type.as_deref() != Some(mime_type.as_str()) {
        return Err(ForumError::Validation(
            "Category cover descriptor MIME does not match the media asset".to_string(),
        ));
    }
    if descriptor.width != Some(width) || descriptor.height != Some(height) {
        return Err(ForumError::Validation(
            "Category cover descriptor dimensions do not match the media asset".to_string(),
        ));
    }

    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use rustok_media::MediaImageDescriptor;
    use uuid::Uuid;

    use super::{
        CategoryCoverMediaCandidate, normalize_category_icon_key, validate_category_cover_candidate,
    };

    fn image_candidate(tenant_id: Uuid) -> CategoryCoverMediaCandidate {
        CategoryCoverMediaCandidate {
            media_id: Uuid::new_v4(),
            tenant_id,
            mime_type: "image/webp".to_string(),
            size: 1024,
            width: Some(1200),
            height: Some(630),
            descriptor: Some(image_descriptor()),
        }
    }

    fn image_descriptor() -> MediaImageDescriptor {
        MediaImageDescriptor::from_parts(
            "/media/cover.webp",
            Some("Category cover".to_string()),
            Some(1200),
            Some(630),
            Some("image/webp".to_string()),
        )
        .expect("descriptor should be valid")
    }

    #[test]
    fn icon_key_normalizes_bounded_kebab_case_tokens() {
        assert_eq!(
            normalize_category_icon_key("  Message-Square  ").as_deref(),
            Some("message-square")
        );
        assert_eq!(
            normalize_category_icon_key("support2").as_deref(),
            Some("support2")
        );
    }

    #[test]
    fn icon_key_rejects_css_markup_urls_and_paths() {
        for value in [
            "message_square",
            "icon class",
            "<svg>",
            "https://example.invalid/icon.svg",
            "../icon",
            "message--square",
            "-message",
            "message-",
        ] {
            assert_eq!(
                normalize_category_icon_key(value),
                None,
                "accepted {value:?}"
            );
        }
    }

    #[test]
    fn cover_candidate_requires_tenant_image_bounds_and_public_descriptor() {
        let tenant_id = Uuid::new_v4();
        let candidate = image_candidate(tenant_id);
        let descriptor = validate_category_cover_candidate(tenant_id, &candidate)
            .expect("valid public image should pass");
        assert_eq!(descriptor.mime_type.as_deref(), Some("image/webp"));
    }

    #[test]
    fn cover_candidate_rejects_foreign_or_non_public_media() {
        let tenant_id = Uuid::new_v4();
        let candidate = image_candidate(Uuid::new_v4());
        assert!(validate_category_cover_candidate(tenant_id, &candidate).is_err());

        let mut candidate = image_candidate(tenant_id);
        candidate.descriptor = MediaImageDescriptor::from_parts(
            "opaque-reference",
            None,
            Some(1200),
            Some(630),
            Some("image/webp".to_string()),
        );
        assert!(validate_category_cover_candidate(tenant_id, &candidate).is_err());
    }
}
