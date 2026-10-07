use crate::model::{CreatePageDraft, PageDetail, PageListQuery, PageStatusFilter};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_ui_core::{normalize_ui_text, parse_ui_csv};
use serde_json::Value;

/// Auto-generates a slug from a title using the shared Pages slug contract, so the editor
/// produces exactly what the server stores (Unicode letters are kept: `О компании` →
/// `о-компании`).
pub fn slugify(value: &str) -> String {
    rustok_page_builder::normalize_page_slug(value)
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
        template: Some("default".to_string()),
        channel_slugs: parse_channel_slugs(input.channel_slugs),
    }
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

/// Page size used by the admin navigator.
pub const PAGE_LIST_PAGE_SIZE: u64 = 25;
/// Mirrors the server search bound so the admin never sends a needle the server would truncate.
pub const PAGE_LIST_MAX_SEARCH_CHARS: usize = 200;

/// Builds a normalized list request: 1-based page, trimmed bounded search, optional status.
pub fn page_list_query(page: u64, search: &str, status: Option<PageStatusFilter>) -> PageListQuery {
    let search = search.trim();
    PageListQuery {
        page: page.max(1),
        per_page: PAGE_LIST_PAGE_SIZE,
        search: (!search.is_empty())
            .then(|| search.chars().take(PAGE_LIST_MAX_SEARCH_CHARS).collect()),
        status,
    }
}

/// Number of list pages for `total` items; always at least one so "page 1 of 1" renders.
pub fn page_list_page_count(total: u64, per_page: u64) -> u64 {
    total.div_ceil(per_page.max(1)).max(1)
}

/// True when the list is empty because of the active filters rather than an empty tenant.
pub fn page_list_is_filtered(query: &PageListQuery) -> bool {
    query.search.is_some() || query.status.is_some()
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

/// Starter document for a new page. It is shared with the server crate and verified there against
/// the static publish policy, so a freshly created page is publishable without manual repair.
pub fn default_project_data(title: &str) -> Value {
    let normalized_title = normalize_ui_text(title);
    rustok_page_builder::starter_page_document(normalized_title.as_deref().unwrap_or_default())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_project_uses_only_the_current_component_contract() {
        let project = default_project_data("Landing");
        assert_eq!(project["pages"][0]["component"]["id"], "root");
        assert_eq!(project["pages"][0]["name"], "Landing");
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
    fn slugify_keeps_cyrillic_titles() {
        assert_eq!(slugify("О компании"), "о-компании");
        let draft = build_create_page_draft(
            PageDraftFormInput {
                locale: "ru",
                title: "О компании",
                slug: &slugify("О компании"),
                channel_slugs: "",
            },
            default_project_data("О компании"),
        );
        assert_eq!(missing_required_page_field(&draft), None);
    }

    #[test]
    fn starter_project_has_no_markup_in_text_content() {
        let project = default_project_data("<Landing>");
        let serialized = project.to_string();
        assert!(!serialized.contains("<h1>"));
        assert!(!serialized.contains("<p>"));
    }

    #[test]
    fn page_list_query_is_normalized() {
        let query = page_list_query(0, "  о компании  ", Some(PageStatusFilter::Draft));
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, PAGE_LIST_PAGE_SIZE);
        assert_eq!(query.search.as_deref(), Some("о компании"));
        assert!(page_list_is_filtered(&query));

        let unfiltered = page_list_query(3, "   ", None);
        assert_eq!(unfiltered.page, 3);
        assert_eq!(unfiltered.search, None);
        assert!(!page_list_is_filtered(&unfiltered));

        let long = "я".repeat(PAGE_LIST_MAX_SEARCH_CHARS + 50);
        assert_eq!(
            page_list_query(1, &long, None)
                .search
                .map(|search| search.chars().count()),
            Some(PAGE_LIST_MAX_SEARCH_CHARS)
        );
    }

    #[test]
    fn page_list_page_count_rounds_up_and_never_returns_zero() {
        assert_eq!(page_list_page_count(0, 25), 1);
        assert_eq!(page_list_page_count(25, 25), 1);
        assert_eq!(page_list_page_count(26, 25), 2);
        assert_eq!(page_list_page_count(10, 0), 10);
    }

    #[test]
    fn page_status_filter_serializes_as_graphql_enum() {
        assert_eq!(
            serde_json::to_value(PageStatusFilter::Published).unwrap(),
            serde_json::json!("PUBLISHED")
        );
        assert_eq!(
            PageStatusFilter::parse(" Draft "),
            Some(PageStatusFilter::Draft)
        );
        assert_eq!(PageStatusFilter::parse("deleted"), None);
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
}
