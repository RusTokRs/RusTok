use crate::{PagePatch, ProjectPage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FLY_PAGE_METADATA_FIELD: &str = "flyPageMeta";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(default)]
    pub no_index: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_graph_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_graph_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_graph_image: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl PageMetadata {
    pub fn from_page(page: &ProjectPage) -> Self {
        page.extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Map::new()))
    }

    pub fn into_page_patch(self) -> PagePatch {
        PagePatch {
            fields: Map::from_iter([(FLY_PAGE_METADATA_FIELD.to_string(), self.into_value())]),
            ..PagePatch::default()
        }
    }

    pub fn normalized(mut self) -> Self {
        self.title = normalize_optional(self.title);
        self.description = normalize_optional(self.description);
        self.slug = normalize_optional(self.slug).map(normalize_slug);
        self.canonical_url = normalize_optional(self.canonical_url);
        self.open_graph_title = normalize_optional(self.open_graph_title);
        self.open_graph_description = normalize_optional(self.open_graph_description);
        self.open_graph_image = normalize_optional(self.open_graph_image);
        self
    }

    pub fn effective_open_graph_title(&self) -> Option<&str> {
        self.open_graph_title.as_deref().or(self.title.as_deref())
    }

    pub fn effective_open_graph_description(&self) -> Option<&str> {
        self.open_graph_description
            .as_deref()
            .or(self.description.as_deref())
    }
}

pub fn normalize_slug(value: String) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;
    for character in value.trim().to_lowercase().chars() {
        if character.is_alphanumeric() {
            if separator_pending && !slug.is_empty() {
                slug.push('-');
            }
            separator_pending = false;
            slug.push(character);
        } else if !slug.is_empty() {
            separator_pending = true;
        }
    }
    slug
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_metadata_preserves_unknown_fields() {
        let page = ProjectPage {
            extensions: Map::from_iter([(
                FLY_PAGE_METADATA_FIELD.to_string(),
                json!({
                    "title": "Landing",
                    "providerFuture": { "enabled": true }
                }),
            )]),
            ..ProjectPage::default()
        };
        let mut metadata = PageMetadata::from_page(&page);
        metadata.description = Some("Description".to_string());
        let value = metadata.into_value();
        assert_eq!(value["providerFuture"]["enabled"], true);
        assert_eq!(value["description"], "Description");
    }

    #[test]
    fn metadata_normalizes_empty_values_and_slug() {
        let metadata = PageMetadata {
            title: Some("  ".to_string()),
            slug: Some(" Hello, Rust World! ".to_string()),
            ..PageMetadata::default()
        }
        .normalized();
        assert_eq!(metadata.title, None);
        assert_eq!(metadata.slug.as_deref(), Some("hello-rust-world"));
    }

    #[test]
    fn open_graph_falls_back_to_standard_metadata() {
        let metadata = PageMetadata {
            title: Some("Title".to_string()),
            description: Some("Description".to_string()),
            ..PageMetadata::default()
        };
        assert_eq!(metadata.effective_open_graph_title(), Some("Title"));
        assert_eq!(
            metadata.effective_open_graph_description(),
            Some("Description")
        );
    }
}
