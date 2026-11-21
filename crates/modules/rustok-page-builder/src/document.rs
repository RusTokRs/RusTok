use serde_json::Value;
use thiserror::Error;

pub const PAGE_BUILDER_DOCUMENT_FORMAT: &str = "grapesjs";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PageBuilderDocumentError {
    #[error("Page Builder document must be a JSON object")]
    NotAnObject,
    #[error("Page Builder document field '{0}' must be an array")]
    FieldMustBeArray(&'static str),
}

impl From<PageBuilderDocumentError> for String {
    fn from(error: PageBuilderDocumentError) -> Self {
        error.to_string()
    }
}

pub fn validate_page_builder_document(document: &Value) -> Result<(), PageBuilderDocumentError> {
    let object = document
        .as_object()
        .ok_or(PageBuilderDocumentError::NotAnObject)?;

    for collection in ["pages", "styles", "assets"] {
        if let Some(value) = object.get(collection)
            && !value.is_array()
        {
            return Err(PageBuilderDocumentError::FieldMustBeArray(collection));
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
