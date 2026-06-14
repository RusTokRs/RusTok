#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};

/// Domain-owned registration entrypoint for product AI verticals.
pub fn register_product_ai_verticals() {
    // Placeholder: runtime adapter registration remains in rustok-ai until the
    // direct handler trait is extracted from the core runtime crate.
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratedProductCopy {
    pub title: Option<String>,
    pub handle: Option<String>,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedProductAttributes {
    pub brand: Option<String>,
    pub material: Option<String>,
    pub color: Option<String>,
    pub size: Option<String>,
    pub dimensions: Option<String>,
    pub compatibility: Option<String>,
    pub care_instructions: Option<String>,
    pub hazmat: Option<String>,
    #[serde(default)]
    pub flex_attributes: Vec<GeneratedFlexAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFlexAttribute {
    pub key: String,
    pub value: String,
}

pub fn validate_product_attributes_payload(
    payload: &GeneratedProductAttributes,
) -> Result<(), String> {
    if payload
        .flex_attributes
        .iter()
        .any(|attr| attr.key.trim().is_empty() || attr.value.trim().is_empty())
    {
        return Err(
            "product_attributes flex_attributes must contain non-empty key/value".to_string(),
        );
    }
    Ok(())
}

pub fn validate_product_copy_payload(payload: &GeneratedProductCopy) -> Result<(), String> {
    let has_text = [
        payload.title.as_deref(),
        payload.handle.as_deref(),
        payload.description.as_deref(),
        payload.meta_title.as_deref(),
        payload.meta_description.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !value.trim().is_empty());

    if !has_text {
        return Err("product_copy must contain at least one non-empty localized field".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        validate_product_attributes_payload, validate_product_copy_payload, GeneratedFlexAttribute,
        GeneratedProductAttributes, GeneratedProductCopy,
    };

    #[test]
    fn accepts_product_attributes_without_flex_attributes() {
        let payload = GeneratedProductAttributes {
            brand: Some("Brand".to_string()),
            material: None,
            color: None,
            size: None,
            dimensions: None,
            compatibility: None,
            care_instructions: None,
            hazmat: None,
            flex_attributes: vec![],
        };
        assert!(validate_product_attributes_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_blank_product_attributes_flex_key_or_value() {
        let payload = GeneratedProductAttributes {
            brand: None,
            material: None,
            color: None,
            size: None,
            dimensions: None,
            compatibility: None,
            care_instructions: None,
            hazmat: None,
            flex_attributes: vec![GeneratedFlexAttribute {
                key: " ".to_string(),
                value: "cotton".to_string(),
            }],
        };
        assert!(validate_product_attributes_payload(&payload).is_err());
    }

    #[test]
    fn accepts_product_copy_with_any_non_empty_field() {
        let payload = GeneratedProductCopy {
            title: Some("Localized title".to_string()),
            ..GeneratedProductCopy::default()
        };
        assert!(validate_product_copy_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_empty_product_copy() {
        let payload = GeneratedProductCopy::default();
        assert!(validate_product_copy_payload(&payload).is_err());
    }
}
