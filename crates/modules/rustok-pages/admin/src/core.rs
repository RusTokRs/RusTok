use crate::model::{CreatePageDraft, PageDetail};
pub use crate::starters::*;
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_ui_core::{normalize_ui_text, parse_ui_csv};
use serde_json::{Value, json};

pub fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn parse_channel_slugs(value: &str) -> Vec<String> {
    let mut items = parse_ui_csv(value)
        .into_iter()
        .map(|item| item.to_ascii_lowercase())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

pub fn optional_ui_text(value: &str) -> Option<String> {
    normalize_ui_text(value)
}

pub fn ui_text_or_default(value: &str) -> String {
    normalize_ui_text(value).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRequiredField {
    Title,
    Slug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageDraftFormInput<'a> {
    pub locale: &'a str,
    pub title: &'a str,
    pub slug: &'a str,
    pub channel_slugs: &'a str,
    pub starter_id: Option<&'a str>,
    pub template: Option<&'a str>,
}

pub fn build_create_page_draft(
    input: PageDraftFormInput<'_>,
    project_data: Value,
) -> CreatePageDraft {
    CreatePageDraft {
        locale: ui_text_or_default(input.locale),
        title: ui_text_or_default(input.title),
        slug: ui_text_or_default(input.slug),
        document: project_data,
        template: input.template.and_then(optional_ui_text).or_else(|| Some("default".to_string())),
        channel_slugs: parse_channel_slugs(input.channel_slugs),
    }
}

pub fn build_create_page_draft_with_starter(input: PageDraftFormInput<'_>) -> CreatePageDraft {
    let starter_id = input.starter_id.unwrap_or("blank");
    let doc = instantiate_starter_preset(starter_id, input.title)
        .unwrap_or_else(|_| default_project_data(input.title));
    build_create_page_draft(input, doc)
}

pub fn missing_required_page_field(draft: &CreatePageDraft) -> Option<PageRequiredField> {
    if draft.title.is_empty() {
        Some(PageRequiredField::Title)
    } else if draft.slug.is_empty() {
        Some(PageRequiredField::Slug)
    } else {
        None
    }
}

pub fn status_badge_class(status: &str) -> &'static str {
    rustok_ui_core::badges::status_badge_class(status)
}

#[derive(Debug, Clone)]
pub struct EditFormSeed {
    pub title: String,
    pub project_data_text: String,
}

pub fn edit_form_seed_from_page(page: &PageDetail, _default_locale: &str) -> EditFormSeed {
    let title = page
        .translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or_default();
    let project_data_text = page
        .body
        .as_ref()
        .and_then(body_to_project_data)
        .map(|project| project_to_pretty_json(&project))
        .unwrap_or_else(|| default_project_data_text(title.as_str()));

    EditFormSeed {
        title,
        project_data_text,
    }
}

fn body_to_project_data(body: &crate::model::PageBody) -> Option<Value> {
    if let Some(project) = body.content_json.as_ref() {
        return Some(project.clone());
    }

    if body.format == PAGE_BUILDER_DOCUMENT_FORMAT {
        serde_json::from_str::<Value>(body.content.as_str()).ok()
    } else {
        None
    }
}

pub fn default_project_data(title: &str) -> Value {
    let normalized_title = normalize_ui_text(title);
    let title = normalized_title.as_deref().unwrap_or("New page");

    json!({
        "assets": [],
        "styles": [],
        "pages": [
            {
                "id": "main",
                "name": title,
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "heading",
                            "type": "text",
                            "content": format!("<h1>{}</h1>", escape_html(title))
                        },
                        {
                            "id": "intro",
                            "type": "text",
                            "content": "<p>Build this page with Fly.</p>"
                        }
                    ]
                }
            }
        ]
    })
}

pub fn default_project_data_text(title: &str) -> String {
    project_to_pretty_json(&default_project_data(title))
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProjectDataParseError {
    #[error("Validation error: invalid project JSON ({0})")]
    InvalidJson(String),
    #[error("Validation error: project JSON root must be an object")]
    RootNotObject,
}

pub fn parse_project_data(raw: &str) -> Result<Value, ProjectDataParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default_project_data(""));
    }

    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|error| ProjectDataParseError::InvalidJson(error.to_string()))?;

    if !parsed.is_object() {
        return Err(ProjectDataParseError::RootNotObject);
    }

    Ok(parsed)
}

fn project_to_pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_project_uses_only_the_current_component_contract() {
        let project = default_project_data("Landing");
        assert_eq!(project["pages"][0]["component"]["id"], "root");
        assert!(project["pages"][0].get("frames").is_none());
    }

    #[test]
    fn channels_are_normalized_and_deduplicated() {
        assert_eq!(
            parse_channel_slugs("Web, mobile, web"),
            vec!["mobile".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn slugify_produces_current_route_slugs() {
        assert_eq!(slugify("Hello, Current Pages!"), "hello-current-pages");
    }

    #[test]
    fn parse_project_data_returns_typed_error_for_invalid_json() {
        let err = parse_project_data("{not json").unwrap_err();
        assert!(matches!(err, ProjectDataParseError::InvalidJson(_)));
    }

    #[test]
    fn parse_project_data_returns_typed_error_for_non_object() {
        let err = parse_project_data("[\"array\"]").unwrap_err();
        assert_eq!(err, ProjectDataParseError::RootNotObject);
    }

    #[test]
    fn template_key_validation() {
        assert_eq!(validate_template_key("landing").unwrap(), "landing");
        assert_eq!(validate_template_key("MAIN_PAGE").unwrap(), "main_page");
        assert_eq!(validate_template_key("with-dash_123").unwrap(), "with-dash_123");
        assert_eq!(validate_template_key("").unwrap_err(), TemplateValidationError::EmptyKey);
        assert_eq!(validate_template_key("bad key!").unwrap_err(), TemplateValidationError::InvalidCharacters);
    }

    #[test]
    fn parse_symbol_id_list_splits_and_normalizes() {
        assert_eq!(
            parse_symbol_id_list("header, footer, nav-bar "),
            vec!["header".to_string(), "footer".to_string(), "nav-bar".to_string()]
        );
    }

    #[test]
    fn symbol_id_and_content_validation() {
        assert_eq!(validate_symbol_id("site-header").unwrap(), "site-header");
        assert_eq!(validate_symbol_id("").unwrap_err(), SymbolValidationError::EmptyId);
        assert_eq!(validate_symbol_id("bad symbol!").unwrap_err(), SymbolValidationError::InvalidCharacters);

        let valid_json = r#"{"type": "section", "components": []}"#;
        assert!(validate_symbol_content_json(valid_json).is_ok());
        assert!(matches!(
            validate_symbol_content_json("not json").unwrap_err(),
            SymbolValidationError::InvalidJson(_)
        ));
        assert_eq!(
            validate_symbol_content_json("[1, 2, 3]").unwrap_err(),
            SymbolValidationError::RootNotObject
        );
    }

    #[test]
    fn symbol_usage_matrix_computation_and_warning() {
        let templates = vec![
            crate::model::PageTemplateSummary {
                key: "default".to_string(),
                locale: "en".to_string(),
                header_symbol_ids: vec!["site-header".to_string()],
                footer_symbol_ids: vec!["site-footer".to_string()],
                revision: 1,
            },
            crate::model::PageTemplateSummary {
                key: "landing".to_string(),
                locale: "en".to_string(),
                header_symbol_ids: vec!["site-header".to_string()],
                footer_symbol_ids: vec![],
                revision: 1,
            },
        ];

        let header_usage = compute_symbol_usage("site-header", &templates, vec!["page-uuid-1".to_string()]);
        assert!(header_usage.is_used());
        assert!(!header_usage.can_delete_safely());
        assert_eq!(header_usage.total_dependents(), 3);
        assert_eq!(header_usage.dependent_templates, vec!["default".to_string(), "landing".to_string()]);
        assert_eq!(header_usage.dependent_page_ids, vec!["page-uuid-1".to_string()]);
        assert!(header_usage.deletion_warning().unwrap().contains("2 template(s): default, landing"));
        assert!(header_usage.deletion_warning().unwrap().contains("1 page(s)"));

        let unused_usage = compute_symbol_usage("promo-banner", &templates, vec![]);
        assert!(!unused_usage.is_used());
        assert!(unused_usage.can_delete_safely());
        assert_eq!(unused_usage.total_dependents(), 0);
        assert_eq!(unused_usage.deletion_warning(), None);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateValidationError {
    EmptyKey,
    InvalidCharacters,
    KeyTooLong,
}

pub fn validate_template_key(key: &str) -> Result<String, TemplateValidationError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(TemplateValidationError::EmptyKey);
    }
    if trimmed.len() > 64 {
        return Err(TemplateValidationError::KeyTooLong);
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
        return Err(TemplateValidationError::InvalidCharacters);
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub fn parse_symbol_id_list(input: &str) -> Vec<String> {
    parse_ui_csv(input)
        .into_iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolValidationError {
    EmptyId,
    IdTooLong,
    InvalidCharacters,
    InvalidJson(String),
    RootNotObject,
}

pub fn validate_symbol_id(id: &str) -> Result<String, SymbolValidationError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(SymbolValidationError::EmptyId);
    }
    if trimmed.len() > 64 {
        return Err(SymbolValidationError::IdTooLong);
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
        return Err(SymbolValidationError::InvalidCharacters);
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub fn validate_symbol_content_json(content: &str) -> Result<Value, SymbolValidationError> {
    let trimmed = content.trim();
    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|e| SymbolValidationError::InvalidJson(e.to_string()))?;
    if !parsed.is_object() {
        return Err(SymbolValidationError::RootNotObject);
    }
    Ok(parsed)
}

pub fn default_symbol_content_json(name: &str) -> String {
    let label = if name.trim().is_empty() {
        "Symbol Component"
    } else {
        name.trim()
    };
    format!(
        "{{\n  \"type\": \"section\",\n  \"classes\": [\"site-symbol-wrapper\"],\n  \"components\": [\n    {{\n      \"type\": \"text\",\n      \"content\": \"{label}\"\n    }}\n  ]\n}}"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteSymbolUsageInfo {
    pub symbol_id: String,
    pub dependent_templates: Vec<String>,
    pub dependent_page_ids: Vec<String>,
}

impl SiteSymbolUsageInfo {
    pub fn is_used(&self) -> bool {
        !self.dependent_templates.is_empty() || !self.dependent_page_ids.is_empty()
    }

    pub fn total_dependents(&self) -> usize {
        self.dependent_templates.len() + self.dependent_page_ids.len()
    }

    pub fn can_delete_safely(&self) -> bool {
        !self.is_used()
    }

    pub fn deletion_warning(&self) -> Option<String> {
        if !self.is_used() {
            return None;
        }
        let mut parts = Vec::new();
        if !self.dependent_templates.is_empty() {
            parts.push(format!(
                "{} template(s): {}",
                self.dependent_templates.len(),
                self.dependent_templates.join(", ")
            ));
        }
        if !self.dependent_page_ids.is_empty() {
            parts.push(format!(
                "{} page(s)",
                self.dependent_page_ids.len()
            ));
        }
        Some(format!(
            "Symbol `{}` is referenced by {}. Deleting it will cause rendering errors or will be rejected by the server.",
            self.symbol_id,
            parts.join(" and ")
        ))
    }
}

pub fn compute_symbol_usage(
    symbol_id: &str,
    templates: &[crate::model::PageTemplateSummary],
    dependent_page_ids: Vec<String>,
) -> SiteSymbolUsageInfo {
    let normalized_id = symbol_id.trim().to_ascii_lowercase();
    let mut dependent_templates: Vec<String> = templates
        .iter()
        .filter(|tpl| {
            tpl.header_symbol_ids
                .iter()
                .any(|id| id.trim().eq_ignore_ascii_case(&normalized_id))
                || tpl.footer_symbol_ids
                    .iter()
                    .any(|id| id.trim().eq_ignore_ascii_case(&normalized_id))
        })
        .map(|tpl| tpl.key.clone())
        .collect();
    dependent_templates.sort();
    dependent_templates.dedup();

    let mut pages = dependent_page_ids;
    pages.sort();
    pages.dedup();

    SiteSymbolUsageInfo {
        symbol_id: normalized_id,
        dependent_templates,
        dependent_page_ids: pages,
    }
}
