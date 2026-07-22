use crate::{LOCALIZED_VALUES_FIELD, PagePatch, ProjectPage};
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
        let Some(mut fields) = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
            .cloned()
        else {
            return Self::default();
        };

        let title = take_plain_or_localized_preview(&mut fields, "title");
        let description = take_plain_or_localized_preview(&mut fields, "description");
        let slug = take_plain_or_localized_preview(&mut fields, "slug");
        let canonical_url = take_plain_or_localized_preview(&mut fields, "canonical_url");
        let open_graph_title = take_plain_or_localized_preview(&mut fields, "open_graph_title");
        let open_graph_description =
            take_plain_or_localized_preview(&mut fields, "open_graph_description");
        let open_graph_image = take_plain_or_localized_preview(&mut fields, "open_graph_image");
        let no_index = fields
            .remove("no_index")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        Self {
            title,
            description,
            slug,
            canonical_url,
            no_index,
            open_graph_title,
            open_graph_description,
            open_graph_image,
            extensions: fields,
        }
    }

    pub fn into_value(self) -> Value {
        let Self {
            title,
            description,
            slug,
            canonical_url,
            no_index,
            open_graph_title,
            open_graph_description,
            open_graph_image,
            mut extensions,
        } = self;

        merge_text_field(&mut extensions, "title", title, normalize_text);
        merge_text_field(&mut extensions, "description", description, normalize_text);
        merge_text_field(&mut extensions, "slug", slug, |value| {
            normalize_slug(value.to_string())
        });
        merge_text_field(
            &mut extensions,
            "canonical_url",
            canonical_url,
            normalize_text,
        );
        merge_text_field(
            &mut extensions,
            "open_graph_title",
            open_graph_title,
            normalize_text,
        );
        merge_text_field(
            &mut extensions,
            "open_graph_description",
            open_graph_description,
            normalize_text,
        );
        merge_text_field(
            &mut extensions,
            "open_graph_image",
            open_graph_image,
            normalize_text,
        );
        if no_index {
            extensions.insert("no_index".to_string(), Value::Bool(true));
        } else {
            extensions.remove("no_index");
        }
        Value::Object(extensions)
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

fn take_plain_or_localized_preview(fields: &mut Map<String, Value>, field: &str) -> Option<String> {
    match fields.get(field) {
        Some(Value::String(_)) => fields
            .remove(field)
            .and_then(|value| value.as_str().map(ToString::to_string))
            .and_then(|value| normalize_optional(Some(value))),
        Some(value) => localized_preview(value),
        None => None,
    }
}

fn localized_preview(value: &Value) -> Option<String> {
    value
        .as_object()
        .and_then(|wrapper| wrapper.get(LOCALIZED_VALUES_FIELD))
        .and_then(Value::as_object)
        .and_then(|values| {
            values.values().find_map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
            })
        })
}

fn merge_text_field(
    fields: &mut Map<String, Value>,
    field: &str,
    value: Option<String>,
    normalize: fn(&str) -> String,
) {
    let Some(value) = value else {
        return;
    };
    let preserve_localized = fields
        .get(field)
        .and_then(localized_preview)
        .is_some_and(|preview| normalize(&preview) == normalize(&value));
    if !preserve_localized {
        fields.insert(field.to_string(), Value::String(value));
    }
}

fn normalize_text(value: &str) -> String {
    value.trim().to_string()
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
        assert_eq!(value["title"], "Landing");
        assert_eq!(value["description"], "Description");
    }

    #[test]
    fn localized_metadata_exposes_preview_and_round_trips_losslessly() {
        let source = json!({
            "title": {
                "$localized": {
                    "en": "Home",
                    "ru": "Главная"
                }
            },
            "slug": {
                "$localized": {
                    "en": "Home Page",
                    "ru": "Главная"
                }
            },
            "providerFuture": { "enabled": true }
        });
        let page = ProjectPage {
            extensions: Map::from_iter([(FLY_PAGE_METADATA_FIELD.to_string(), source.clone())]),
            ..ProjectPage::default()
        };
        let metadata = PageMetadata::from_page(&page);
        assert!(matches!(
            metadata.title.as_deref(),
            Some("Home" | "Главная")
        ));
        assert!(metadata.slug.is_some());
        let value = metadata.normalized().into_value();
        assert_eq!(value["title"], source["title"]);
        assert_eq!(value["slug"], source["slug"]);
        assert_eq!(value["providerFuture"]["enabled"], true);
    }

    #[test]
    fn editing_plain_preview_replaces_only_the_selected_metadata_field() {
        let page = ProjectPage {
            extensions: Map::from_iter([(
                FLY_PAGE_METADATA_FIELD.to_string(),
                json!({
                    "title": { "$localized": { "en": "Home", "ru": "Главная" } },
                    "description": { "$localized": { "en": "English", "ru": "Русский" } }
                }),
            )]),
            ..ProjectPage::default()
        };
        let mut metadata = PageMetadata::from_page(&page);
        metadata.title = Some("Replacement".to_string());
        let value = metadata.into_value();
        assert_eq!(value["title"], "Replacement");
        assert!(value["description"].is_object());
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
