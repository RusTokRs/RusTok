use crate::model::{PageBlock, PageDetail};
use rustok_api::{WritePathIssue, WritePathIssueKind};
use serde_json::{json, Value};

pub const GRAPESJS_FORMAT: &str = "grapesjs_v1";

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
    let mut items = value
        .split(',')
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{}: {}", context, error)
}

pub fn write_path_issue_with_context(context: &str, error: &str) -> WritePathIssue {
    WritePathIssue::with_context(context, error)
}

pub fn issue_banner_class(kind: WritePathIssueKind) -> &'static str {
    match kind {
        WritePathIssueKind::Validation | WritePathIssueKind::Sanitization => {
            "rounded-xl border border-amber-300/50 bg-amber-50 px-4 py-3 text-sm text-amber-900"
        }
        WritePathIssueKind::Runtime => {
            "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        }
    }
}

pub fn issue_label<'a>(
    kind: WritePathIssueKind,
    validation_label: &'a str,
    sanitization_label: &'a str,
    runtime_label: &'a str,
) -> &'a str {
    match kind {
        WritePathIssueKind::Validation => validation_label,
        WritePathIssueKind::Sanitization => sanitization_label,
        WritePathIssueKind::Runtime => runtime_label,
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

pub fn busy_key_with_id(action: &str, page_id: &str) -> String {
    format!("{}:{}", action, page_id)
}

pub fn busy_key_for_save(page_id: Option<&str>) -> String {
    page_id
        .map(|id| busy_key_with_id("save", id))
        .unwrap_or_else(|| "create".to_string())
}

#[derive(Debug, Clone)]
pub struct EditFormSeed {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub project_data_text: String,
    pub channel_slugs_text: String,
    pub publish_now: bool,
    pub body_format: String,
    pub body_updated_at: Option<String>,
    pub existing_blocks: Vec<PageBlock>,
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
    let body_format = page
        .body
        .as_ref()
        .map(|page_body| page_body.format.clone())
        .unwrap_or_else(|| GRAPESJS_FORMAT.to_string());
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
        body_format,
        body_updated_at: page.body.as_ref().map(|body| body.updated_at.clone()),
        existing_blocks: page.blocks.clone(),
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
    let normalized_title = title.trim();
    let title = if normalized_title.is_empty() {
        "New page"
    } else {
        normalized_title
    };

    json!({
        "assets": [],
        "styles": [],
        "pages": [
            {
                "id": "main",
                "name": title,
                "frames": [
                    {
                        "component": {
                            "type": "wrapper",
                            "components": [
                                {
                                    "type": "text",
                                    "content": format!("<h1>{}</h1>", escape_html(title)),
                                },
                                {
                                    "type": "text",
                                    "content": "<p>Contract-safe starter project for grapesjs_v1.</p>",
                                }
                            ]
                        }
                    }
                ]
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

pub fn project_tree(project: &Value) -> Vec<String> {
    let mut labels = Vec::new();

    let pages = project
        .get("pages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if pages.is_empty() {
        labels.push("page: (missing pages[])".to_string());
        return labels;
    }

    for (index, page) in pages.iter().enumerate() {
        let page_name = page
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| page.get("id").and_then(Value::as_str))
            .unwrap_or("untitled");
        labels.push(format!("page {} · {}", index + 1, page_name));

        let maybe_components = page
            .get("frames")
            .and_then(Value::as_array)
            .and_then(|frames| frames.first())
            .and_then(|frame| frame.get("component"))
            .and_then(|component| component.get("components"))
            .and_then(Value::as_array);

        match maybe_components {
            Some(components) if !components.is_empty() => {
                for component in components {
                    collect_component_nodes(component, 1, &mut labels);
                }
            }
            _ => labels.push("  • canvas is empty".to_string()),
        }
    }

    labels
}

pub fn preview_html(title: &str, slug: &str, locale: &str, project: &Value) -> String {
    let safe_title = escape_html(title.trim());
    let safe_slug = escape_html(slug.trim());
    let safe_locale = escape_html(locale.trim());

    let tree = project_tree(project);
    let tree_markup = tree
        .iter()
        .take(8)
        .map(|line| format!("<li>{}</li>", escape_html(line)))
        .collect::<String>();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\" /><style>body{{font-family:Inter,Segoe UI,sans-serif;margin:0;padding:1rem;background:#f8fafc;color:#0f172a;}}h1{{margin:0 0 .5rem;}}.meta{{font-size:.85rem;color:#475569;margin-bottom:.75rem;}}ul{{margin:.5rem 0 0 1rem;padding:0;}}li{{margin:.2rem 0;}}</style></head><body><h1>{}</h1><div class=\"meta\">slug: {} · locale: {} · format: {}</div><p>Contract-safe preview surface for grapesjs_v1. Tree snapshot:</p><ul>{}</ul></body></html>",
        if safe_title.is_empty() {
            "New page"
        } else {
            safe_title.as_str()
        },
        if safe_slug.is_empty() {
            "(draft)"
        } else {
            safe_slug.as_str()
        },
        if safe_locale.is_empty() {
            "(default)"
        } else {
            safe_locale.as_str()
        },
        GRAPESJS_FORMAT,
        tree_markup
    )
}

fn collect_component_nodes(component: &Value, depth: usize, labels: &mut Vec<String>) {
    if labels.len() >= 64 {
        return;
    }

    let node_name = component
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| component.get("tagName").and_then(Value::as_str))
        .or_else(|| component.get("type").and_then(Value::as_str))
        .unwrap_or("component");
    let indent = "  ".repeat(depth);
    labels.push(format!("{}• {}", indent, node_name));

    if depth >= 5 {
        return;
    }

    if let Some(children) = component.get("components").and_then(Value::as_array) {
        for child in children {
            collect_component_nodes(child, depth + 1, labels);
        }
    }
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

pub fn status_badge_css(status: &str) -> String {
    format!(
        "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}",
        status_badge_class(status)
    )
}

pub fn busy_key_matches_action(busy_key: Option<&str>, action: &str) -> bool {
    let prefix = format!("{}:", action);
    busy_key
        .map(|key| key.starts_with(prefix.as_str()))
        .unwrap_or(false)
}

pub fn empty_edit_form_seed(default_locale: &str) -> EditFormSeed {
    EditFormSeed {
        locale: default_locale.to_string(),
        title: String::new(),
        slug: String::new(),
        project_data_text: default_project_data_text(""),
        channel_slugs_text: String::new(),
        publish_now: false,
        body_format: GRAPESJS_FORMAT.to_string(),
        body_updated_at: None,
        existing_blocks: Vec::new(),
    }
}

pub fn count_label(template: &str, count: u64) -> String {
    template.replace("{count}", &count.to_string())
}

pub fn label_with_id(template: &str, id: &str) -> String {
    template.replace("{id}", id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_ascii_words() {
        assert_eq!(slugify("Hello, Rustok Pages!"), "hello-rustok-pages");
    }

    #[test]
    fn parse_channel_slugs_trims_sorts_and_deduplicates() {
        assert_eq!(
            parse_channel_slugs(" web, mobile-app,WEB, , mobile-app "),
            vec!["mobile-app".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn helper_formatting_stays_consistent() {
        assert_eq!(
            error_with_context("Failed to save page", "timeout"),
            "Failed to save page: timeout"
        );
        assert_eq!(
            status_badge_class("published"),
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        assert_eq!(status_badge_class("draft"), "bg-primary/10 text-primary");
        assert_eq!(busy_key_with_id("edit", "p_1"), "edit:p_1");
        assert_eq!(busy_key_for_save(Some("p_2")), "save:p_2");
        assert_eq!(busy_key_for_save(None), "create");
        assert!(busy_key_matches_action(Some("save:p_1"), "save"));
        assert!(!busy_key_matches_action(Some("edit:p_1"), "save"));
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
    }

    #[test]
    fn label_with_id_replaces_placeholder() {
        assert_eq!(
            label_with_id("Editing page {id}", "page_1"),
            "Editing page page_1"
        );
    }

    #[test]
    fn count_label_replaces_placeholder() {
        assert_eq!(count_label("{count} page(s)", 7), "7 page(s)");
    }

    #[test]
    fn empty_edit_form_seed_uses_default_locale() {
        let seed = empty_edit_form_seed("en");
        assert_eq!(seed.locale, "en");
        assert!(seed.title.is_empty());
        assert!(seed.slug.is_empty());
        assert!(!seed.project_data_text.is_empty());
        assert!(seed.channel_slugs_text.is_empty());
        assert!(!seed.publish_now);
        assert_eq!(seed.body_format, GRAPESJS_FORMAT);
        assert!(seed.body_updated_at.is_none());
        assert!(seed.existing_blocks.is_empty());
    }

    #[test]
    fn edit_form_seed_extracts_values() {
        let page = PageDetail {
            id: "p_1".to_string(),
            status: "published".to_string(),
            template: "default".to_string(),
            channel_slugs: vec!["web".to_string(), "mobile".to_string()],
            translation: Some(crate::model::PageTranslation {
                locale: "ru".to_string(),
                title: Some("Заголовок".to_string()),
                slug: Some("slug".to_string()),
            }),
            body: Some(crate::model::PageBody {
                locale: "ru".to_string(),
                content: String::new(),
                format: GRAPESJS_FORMAT.to_string(),
                content_json: Some(default_project_data("Заголовок")),
                updated_at: "2026-05-23T10:30:00Z".to_string(),
            }),
            blocks: vec![crate::model::PageBlock {
                id: "block_1".to_string(),
                block_type: "hero".to_string(),
                position: 0,
            }],
        };

        let seed = edit_form_seed_from_page(&page, "en");
        assert_eq!(seed.locale, "ru");
        assert_eq!(seed.title, "Заголовок");
        assert_eq!(seed.slug, "slug");
        assert!(seed.project_data_text.contains("\"pages\""));
        assert_eq!(seed.channel_slugs_text, "web, mobile");
        assert!(seed.publish_now);
        assert_eq!(seed.body_format, GRAPESJS_FORMAT);
        assert_eq!(
            seed.body_updated_at.as_deref(),
            Some("2026-05-23T10:30:00Z")
        );
        assert_eq!(seed.existing_blocks.len(), 1);
    }

    #[test]
    fn parse_project_data_rejects_non_object_root() {
        let error = parse_project_data("[]").expect_err("array root must fail");
        assert!(error.contains("Validation error"));
    }

    #[test]
    fn project_tree_extracts_pages_and_components() {
        let project = json!({
            "pages": [
                {
                    "name": "Landing",
                    "frames": [
                        {
                            "component": {
                                "components": [
                                    {
                                        "type": "section",
                                        "components": [
                                            { "type": "text" }
                                        ]
                                    }
                                ]
                            }
                        }
                    ]
                }
            ]
        });

        let tree = project_tree(&project);
        assert!(tree.iter().any(|line| line.contains("page 1 · Landing")));
        assert!(tree.iter().any(|line| line.contains("section")));
        assert!(tree.iter().any(|line| line.contains("text")));
    }
}
