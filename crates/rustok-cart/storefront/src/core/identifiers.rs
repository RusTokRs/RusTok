use serde_json::Value;
use uuid::Uuid;

use super::CartCoreError;

pub fn normalize_cart_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn parse_cart_id(value: Option<String>) -> Result<Option<(String, Uuid)>, CartCoreError> {
    match normalize_cart_id(value) {
        Some(cart_id) => {
            let parsed = Uuid::parse_str(cart_id.as_str()).map_err(|_| {
                CartCoreError::Validation("cart_id must be a valid UUID".to_string())
            })?;
            Ok(Some((cart_id, parsed)))
        }
        None => Ok(None),
    }
}

pub fn parse_line_item_id(value: String) -> Result<(String, Uuid), CartCoreError> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(CartCoreError::Validation(
            "line_item_id must not be empty".to_string(),
        ));
    }

    let parsed = Uuid::parse_str(normalized.as_str())
        .map_err(|_| CartCoreError::Validation("line_item_id must be a valid UUID".to_string()))?;
    Ok((normalized, parsed))
}

pub fn parse_adjustment_scope(metadata: &str) -> Option<String> {
    serde_json::from_str::<Value>(metadata)
        .ok()
        .and_then(|value| {
            value
                .get("scope")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

#[allow(dead_code)]
pub fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cart_id_trims_and_drops_empty_values() {
        assert_eq!(normalize_cart_id(None), None);
        assert_eq!(normalize_cart_id(Some("  ".to_string())), None);
        assert_eq!(
            normalize_cart_id(Some("  550e8400-e29b-41d4-a716-446655440000  ".to_string())),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn parse_cart_id_validates_uuid_after_normalization() {
        assert!(parse_cart_id(Some("not-a-uuid".to_string())).is_err());
        let parsed = parse_cart_id(Some(" 550e8400-e29b-41d4-a716-446655440000 ".to_string()))
            .expect("cart id should parse")
            .expect("cart id should be present");

        assert_eq!(parsed.0, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_line_item_id_rejects_empty_and_invalid_values() {
        assert!(parse_line_item_id("  ".to_string()).is_err());
        assert!(parse_line_item_id("abc".to_string()).is_err());
    }

    #[test]
    fn parse_adjustment_scope_reads_language_neutral_metadata() {
        assert_eq!(
            parse_adjustment_scope(r#"{"scope":"line_item","label":"ignored"}"#),
            Some("line_item".to_string())
        );
        assert_eq!(parse_adjustment_scope("not json"), None);
    }

    #[test]
    fn normalize_public_channel_slug_trims_and_lowercases() {
        assert_eq!(normalize_public_channel_slug(None), None);
        assert_eq!(normalize_public_channel_slug(Some("  ")), None);
        assert_eq!(
            normalize_public_channel_slug(Some("  Web-Store  ")),
            Some("web-store".to_string())
        );
    }
}
