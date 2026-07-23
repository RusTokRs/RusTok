use crate::model::{CreatePageDraft, PageDetail};
use rustok_ui_core::{normalize_ui_text, parse_ui_csv};
use serde_json::{Value, json};

pub const GRAPESJS_FORMAT: &str = "grapesjs";

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
    pub publish: bool,
}

pub fn build_create_page_draft(
    input: PageDraftFormInput<'_>,
    project_data: Value,
) -> CreatePageDraft {
    CreatePageDraft {
        locale: ui_text_or_default(input.locale),
        title: ui_text_or_default(input.title),
        slug: ui_text_or_default(input.slug),
        body_content: String::new(),
        body_format: GRAPESJS_FORMAT.to_string(),
        body_content_json: project_data,
        template: Some("default".to_string()),
        channel_slugs: parse_channel_slugs(input.channel_slugs),
        publish: input.publish,
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

pub fn status_badge_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" => {
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        }
        "archived" => "bg-muted text-muted-foreground",
        _ => "bg-primary/10 text-primary",
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EditFormSeed {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub project_data_text: String,
    pub channel_slugs_text: String,
    pub publish_now: bool,
}

pub fn edit_form_seed_from_page(page: &PageDetail, default_locale: &str) -> EditFormSeed {
    let locale = page
        .translation
        .as_ref()
        .map(|translation| translation.locale.clone())
        .or_else(|| page.body.as_ref().map(|page_body| page_body.locale.clone()))
        .unwrap_or_else(|| default_locale.to_string());
    let title = page
        .translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or_default();
    let slug = page
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .unwrap_or_default();
    let project_data_text = page
        .body
        .as_ref()
        .and_then(body_to_project_data)
        .map(|project| project_to_pretty_json(&project))
        .unwrap_or_else(|| default_project_data_text(title.as_str()));

    EditFormSeed {
        locale,
        title,
        slug,
        project_data_text,
        channel_slugs_text: page.channel_slugs.join(", "),
        publish_now: page.status.eq_ignore_ascii_case("published"),
    }
}

fn body_to_project_data(body: &crate::model::PageBody) -> Option<Value> {
    if let Some(project) = body.content_json.as_ref() {
        return Some(project.clone());
    }

    if body.format.eq_ignore_ascii_case(GRAPESJS_FORMAT) {
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

pub fn parse_project_data(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default_project_data(""));
    }

    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|error| format!("Validation error: invalid project JSON ({error})"))?;

    if !parsed.is_object() {
        return Err("Validation error: project JSON root must be an object".to_string());
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
}
