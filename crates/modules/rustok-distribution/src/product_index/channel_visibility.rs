use std::collections::BTreeSet;

use serde_json::Value as JsonValue;
use thiserror::Error;

pub(crate) const MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS: usize = 1024;
pub(crate) const MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUG_BYTES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProductChannelVisibility {
    Unrestricted,
    Restricted(Vec<String>),
}

impl ProductChannelVisibility {
    pub(crate) fn is_unrestricted(&self) -> bool {
        matches!(self, Self::Unrestricted)
    }

    /// Stable opaque witness stored by the Product owner. It is deliberately not an Index field.
    pub(crate) fn freshness_key(&self) -> String {
        match self {
            Self::Unrestricted => "all".to_owned(),
            Self::Restricted(slugs) => format!(
                "restricted:{}",
                serde_json::to_string(slugs)
                    .expect("canonical Product channel visibility strings are serializable")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ProductChannelVisibilityError {
    #[error("Product channel visibility is invalid")]
    Invalid,
    #[error("Product channel visibility contains too many slugs")]
    TooManySlugs,
    #[error("Product channel visibility slug is too long")]
    SlugTooLong,
}

pub(crate) fn decode_product_visibility(
    metadata: &JsonValue,
) -> Result<ProductChannelVisibility, ProductChannelVisibilityError> {
    let object = metadata
        .as_object()
        .ok_or(ProductChannelVisibilityError::Invalid)?;
    let Some(channel_visibility) = object.get("channel_visibility") else {
        return Ok(ProductChannelVisibility::Unrestricted);
    };
    let channel_visibility = channel_visibility
        .as_object()
        .ok_or(ProductChannelVisibilityError::Invalid)?;
    let allowed_channel_slugs = channel_visibility
        .get("allowed_channel_slugs")
        .ok_or(ProductChannelVisibilityError::Invalid)?
        .as_array()
        .ok_or(ProductChannelVisibilityError::Invalid)?;
    if allowed_channel_slugs.is_empty() {
        return Ok(ProductChannelVisibility::Unrestricted);
    }
    if allowed_channel_slugs.len() > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS {
        return Err(ProductChannelVisibilityError::TooManySlugs);
    }

    let mut canonical = BTreeSet::new();
    for value in allowed_channel_slugs {
        let raw = value
            .as_str()
            .ok_or(ProductChannelVisibilityError::Invalid)?;
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized != raw || !canonical.insert(normalized.clone()) {
            return Err(ProductChannelVisibilityError::Invalid);
        }
        if normalized.len() > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUG_BYTES {
            return Err(ProductChannelVisibilityError::SlugTooLong);
        }
    }

    Ok(ProductChannelVisibility::Restricted(
        canonical.into_iter().collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_key_is_canonical_and_distinguishes_unrestricted() {
        assert_eq!(
            decode_product_visibility(&serde_json::json!({}))
                .unwrap()
                .freshness_key(),
            "all"
        );
        assert_eq!(
            decode_product_visibility(&serde_json::json!({
                "channel_visibility": {"allowed_channel_slugs": ["alpha", "beta"]}
            }))
            .unwrap()
            .freshness_key(),
            "restricted:[\"alpha\",\"beta\"]"
        );
    }

    #[test]
    fn visibility_rejects_noncanonical_duplicate_and_oversized_slugs() {
        for metadata in [
            serde_json::json!({"channel_visibility": {"allowed_channel_slugs": [" Alpha "]}}),
            serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["alpha", "alpha"]}}),
            serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["x".repeat(101)]}}),
        ] {
            assert!(decode_product_visibility(&metadata).is_err());
        }
    }
}
