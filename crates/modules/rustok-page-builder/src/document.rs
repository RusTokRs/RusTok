use serde_json::Value;

pub const PAGE_BUILDER_DOCUMENT_FORMAT: &str = "grapesjs";

pub fn validate_page_builder_document(document: &Value) -> Result<(), String> {
    let object = document
        .as_object()
        .ok_or_else(|| "Page Builder document must be a JSON object".to_string())?;

    for collection in ["pages", "styles", "assets"] {
        if let Some(value) = object.get(collection)
            && !value.is_array()
        {
            return Err(format!(
                "Page Builder document field '{collection}' must be an array"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_page_builder_document;

    #[test]
    fn accepts_a_minimal_document() {
        assert!(validate_page_builder_document(&serde_json::json!({})).is_ok());
        assert!(
            validate_page_builder_document(&serde_json::json!({
                "pages": [],
                "styles": [],
                "assets": [],
            }))
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_objects_and_invalid_collections() {
        assert!(validate_page_builder_document(&serde_json::json!([])).is_err());
        assert!(validate_page_builder_document(&serde_json::json!({ "pages": {} })).is_err());
        assert!(validate_page_builder_document(&serde_json::json!({ "styles": {} })).is_err());
        assert!(validate_page_builder_document(&serde_json::json!({ "assets": {} })).is_err());
    }
}
