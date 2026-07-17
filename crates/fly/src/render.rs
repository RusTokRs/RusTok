use crate::{
    ComponentNode, ComponentObject, FlyError, FlyResult, PageMetadata, ProjectDocument,
    ProjectPage, StyleRuleCatalog, StyleRuleScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageSelection {
    First,
    Index(usize),
    Id(String),
    Slug(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderPolicy {
    pub instrument_components: bool,
    pub emit_style_hooks: bool,
    pub allow_http: bool,
    pub allow_https: bool,
    pub allow_relative_urls: bool,
    pub allow_hash_urls: bool,
    pub allow_mailto: bool,
    pub allow_tel: bool,
    pub allow_data_images: bool,
    pub include_opaque_text_nodes: bool,
}

impl Default for RenderPolicy {
    fn default() -> Self {
        Self {
            instrument_components: false,
            emit_style_hooks: true,
            allow_http: true,
            allow_https: true,
            allow_relative_urls: true,
            allow_hash_urls: true,
            allow_mailto: true,
            allow_tel: true,
            allow_data_images: true,
            include_opaque_text_nodes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PageHead {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub robots: Option<String>,
    pub open_graph_title: Option<String>,
    pub open_graph_description: Option<String>,
    pub open_graph_image: Option<String>,
}

impl PageHead {
    pub fn from_metadata(metadata: &PageMetadata) -> Self {
        Self {
            title: metadata.title.clone(),
            description: metadata.description.clone(),
            canonical_url: metadata.canonical_url.clone(),
            robots: metadata.no_index.then_some("noindex,nofollow".to_string()),
            open_graph_title: metadata
                .effective_open_graph_title()
                .map(ToString::to_string),
            open_graph_description: metadata
                .effective_open_graph_description()
                .map(ToString::to_string),
            open_graph_image: metadata.open_graph_image.clone(),
        }
    }

    pub fn render_html(&self) -> String {
        let mut html = String::new();
        if let Some(title) = &self.title {
            html.push_str("<title>");
            html.push_str(&escape_html(title));
            html.push_str("</title>");
        }
        push_meta(
            &mut html,
            "name",
            "description",
            self.description.as_deref(),
        );
        push_meta(&mut html, "name", "robots", self.robots.as_deref());
        push_meta(
            &mut html,
            "property",
            "og:title",
            self.open_graph_title.as_deref(),
        );
        push_meta(
            &mut html,
            "property",
            "og:description",
            self.open_graph_description.as_deref(),
        );
        push_meta(
            &mut html,
            "property",
            "og:image",
            self.open_graph_image.as_deref(),
        );
        if let Some(canonical_url) = &self.canonical_url {
            html.push_str("<link rel=\"canonical\" href=\"");
            html.push_str(&escape_attribute(canonical_url));
            html.push_str("\">");
        }
        html
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderedPage {
    pub page_index: usize,
    pub page_id: Option<String>,
    pub metadata: PageMetadata,
    pub head: PageHead,
    pub html: String,
    pub css: String,
}

impl RenderedPage {
    pub fn document_html(&self) -> String {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">{}<style>{}</style></head><body>{}</body></html>",
            self.head.render_html(),
            self.css,
            self.html,
        )
    }
}

pub fn render_page(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
) -> FlyResult<RenderedPage> {
    let (page_index, page) = resolve_page(document, selection)?;
    let root = page
        .component
        .as_ref()
        .ok_or_else(|| FlyError::MissingPageRoot(page_index.to_string()))?;
    let mut html = String::new();
    render_node(root, None, 0, policy, &mut html);
    let metadata = PageMetadata::from_page(page);
    let head = PageHead::from_metadata(&metadata);
    let css = if policy.emit_style_hooks {
        render_project_styles(document, page)
    } else {
        String::new()
    };
    Ok(RenderedPage {
        page_index,
        page_id: page.id.clone(),
        metadata,
        head,
        html,
        css,
    })
}

pub fn resolve_page<'a>(
    document: &'a ProjectDocument,
    selection: &PageSelection,
) -> FlyResult<(usize, &'a ProjectPage)> {
    let index = match selection {
        PageSelection::First => 0,
        PageSelection::Index(index) => *index,
        PageSelection::Id(id) => document
            .project
            .pages
            .iter()
            .position(|page| page.id.as_deref() == Some(id.as_str()))
            .ok_or_else(|| FlyError::PageNotFound(id.clone()))?,
        PageSelection::Slug(slug) => document
            .project
            .pages
            .iter()
            .position(|page| PageMetadata::from_page(page).slug.as_deref() == Some(slug.as_str()))
            .ok_or_else(|| FlyError::PageNotFound(slug.clone()))?,
    };
    document
        .project
        .pages
        .get(index)
        .map(|page| (index, page))
        .ok_or_else(|| FlyError::PageNotFound(index.to_string()))
}

fn render_node(
    node: &ComponentNode,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    output: &mut String,
) {
    match node {
        ComponentNode::Object(component) => {
            render_component(component, parent_id, index, policy, output)
        }
        ComponentNode::Opaque(value) if policy.include_opaque_text_nodes => {
            render_opaque(value, output)
        }
        ComponentNode::Opaque(_) => {}
    }
}

fn render_component(
    component: &ComponentObject,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    output: &mut String,
) {
    let component_id = component.id.as_deref();
    let tag = safe_tag(component);
    let void_tag = matches!(tag, "img" | "input" | "hr" | "br");

    output.push('<');
    output.push_str(tag);
    if policy.emit_style_hooks {
        if let Some(component_id) = component_id {
            write_attribute(output, "data-fly-style-id", component_id);
        }
    }
    if policy.instrument_components {
        if let Some(component_id) = component_id {
            write_attribute(output, "data-fly-component-id", component_id);
        }
        write_attribute(output, "data-fly-index", &index.to_string());
        if let Some(parent_id) = parent_id {
            write_attribute(output, "data-fly-parent-id", parent_id);
        }
    }

    for (name, value) in &component.attributes {
        if !safe_attribute_name(name) || matches!(name.as_str(), "style" | "srcdoc") {
            continue;
        }
        if let Value::Bool(enabled) = value {
            if *enabled {
                output.push(' ');
                output.push_str(name);
            }
            continue;
        }
        let Some(value) = scalar_string(value) else {
            continue;
        };
        if matches!(
            name.as_str(),
            "href" | "src" | "poster" | "action" | "formaction"
        ) && !url_allowed(&value, policy)
        {
            continue;
        }
        write_attribute(output, name, &value);
    }

    if let Some(style) = component.style.as_ref().and_then(Value::as_object) {
        let declarations = style
            .iter()
            .filter_map(|(name, value)| safe_style(name, value))
            .collect::<Vec<_>>()
            .join(";");
        if !declarations.is_empty() {
            write_attribute(output, "style", &declarations);
        }
    }

    output.push('>');
    if void_tag {
        return;
    }

    if let Some(content) = component.extensions.get("content").and_then(Value::as_str) {
        output.push_str(&escape_html(&strip_tags(content)));
    }
    for (child_index, child) in component.children().iter().enumerate() {
        render_node(child, component_id, child_index, policy, output);
    }
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
}

fn render_project_styles(document: &ProjectDocument, page: &ProjectPage) -> String {
    let mut component_ids = Vec::new();
    if let Some(root) = page.component.as_ref() {
        root.collect_ids(&mut component_ids);
    }
    let component_ids = component_ids.into_iter().collect::<BTreeSet<_>>();
    let catalog = StyleRuleCatalog::from_document(document);
    let mut css = String::new();
    for rule in catalog.rules {
        let Some(component_id) = rule.component_id else {
            continue;
        };
        if !component_ids.contains(&component_id) {
            continue;
        }
        let declarations = rule
            .declarations
            .iter()
            .filter_map(|(name, value)| safe_style(name, value))
            .collect::<Vec<_>>()
            .join(";");
        if declarations.is_empty() {
            continue;
        }
        let selector = format!(
            "[data-fly-style-id=\"{}\"]",
            escape_css_attribute(&component_id)
        );
        match rule.scope {
            StyleRuleScope::Base => push_rule(&mut css, &selector, &declarations),
            StyleRuleScope::Media { query } if safe_media_query(&query) => {
                css.push_str("@media ");
                css.push_str(query.trim());
                css.push('{');
                push_rule(&mut css, &selector, &declarations);
                css.push('}');
            }
            StyleRuleScope::Media { .. } => {}
        }
    }
    css
}

fn push_rule(css: &mut String, selector: &str, declarations: &str) {
    css.push_str(selector);
    css.push('{');
    css.push_str(declarations);
    css.push('}');
}

fn push_meta(html: &mut String, kind: &str, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    html.push_str("<meta ");
    html.push_str(kind);
    html.push_str("=\"");
    html.push_str(&escape_attribute(key));
    html.push_str("\" content=\"");
    html.push_str(&escape_attribute(value));
    html.push_str("\">");
}

fn safe_tag(component: &ComponentObject) -> &'static str {
    let requested = component
        .tag_name
        .as_deref()
        .unwrap_or_else(|| match component.component_type() {
            "wrapper" | "container" | "row" | "column" | "grid" | "spacer" => "div",
            "section" => "section",
            "heading" => "h2",
            "text" => "p",
            "list" => "ul",
            "list_item" => "li",
            "link" => "a",
            "image" => "img",
            "video" => "video",
            "media" => "figure",
            "button" => "button",
            "divider" => "hr",
            "form" => "form",
            "label" => "label",
            "input" | "checkbox" => "input",
            "textarea" => "textarea",
            "select" => "select",
            "option" => "option",
            "submit" => "button",
            _ => "div",
        })
        .to_ascii_lowercase();
    match requested.as_str() {
        "div" => "div",
        "main" => "main",
        "section" => "section",
        "article" => "article",
        "header" => "header",
        "footer" => "footer",
        "nav" => "nav",
        "aside" => "aside",
        "figure" => "figure",
        "figcaption" => "figcaption",
        "p" => "p",
        "span" => "span",
        "small" => "small",
        "strong" => "strong",
        "em" => "em",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "a" => "a",
        "button" => "button",
        "img" => "img",
        "video" => "video",
        "audio" => "audio",
        "source" => "source",
        "picture" => "picture",
        "ul" => "ul",
        "ol" => "ol",
        "li" => "li",
        "blockquote" => "blockquote",
        "form" => "form",
        "label" => "label",
        "input" => "input",
        "textarea" => "textarea",
        "select" => "select",
        "option" => "option",
        "hr" => "hr",
        "br" => "br",
        _ => "div",
    }
}

fn safe_attribute_name(name: &str) -> bool {
    !name.to_ascii_lowercase().starts_with("on")
        && !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
        })
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn url_allowed(value: &str, policy: &RenderPolicy) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    (policy.allow_relative_urls
        && (normalized.starts_with('/')
            || normalized.starts_with("./")
            || normalized.starts_with("../")))
        || (policy.allow_hash_urls && normalized.starts_with('#'))
        || (policy.allow_http && normalized.starts_with("http://"))
        || (policy.allow_https && normalized.starts_with("https://"))
        || (policy.allow_mailto && normalized.starts_with("mailto:"))
        || (policy.allow_tel && normalized.starts_with("tel:"))
        || (policy.allow_data_images && normalized.starts_with("data:image/"))
}

fn safe_style(name: &str, value: &Value) -> Option<String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '-')
    {
        return None;
    }
    let value = scalar_string(value)?;
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("expression(")
        || normalized.contains("javascript:")
        || normalized.contains("url(")
        || value.contains('<')
        || value.contains('>')
        || value.contains(';')
    {
        return None;
    }
    Some(format!("{name}:{value}"))
}

fn safe_media_query(query: &str) -> bool {
    let normalized = query.trim().to_ascii_lowercase();
    !normalized.is_empty()
        && normalized.len() <= 256
        && !normalized.contains('{')
        && !normalized.contains('}')
        && !normalized.contains(';')
        && !normalized.contains("url(")
        && !normalized.contains("expression(")
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "() :.-_%/,".contains(character))
}

fn write_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_attribute(value));
    output.push('"');
}

fn render_opaque(value: &Value, output: &mut String) {
    match value {
        Value::String(value) => output.push_str(&escape_html(&strip_tags(value))),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        _ => {}
    }
}

fn strip_tags(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut inside_tag = false;
    for character in value.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(character),
            _ => {}
        }
    }
    result
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(value: &str) -> String {
    escape_html(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_css_attribute(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\a ")
        .replace('\r', "\\d ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "styles": [{
                "selectors": [{ "name": "hero", "type": 2 }],
                "style": { "padding": "24px" },
                "atRuleType": "media",
                "mediaText": "(max-width: 767px)",
                "flyComponentId": "hero"
            }],
            "pages": [{
                "id": "home",
                "name": "Home",
                "flyPageMeta": {
                    "title": "Home title",
                    "description": "Home description",
                    "slug": "home",
                    "open_graph_image": "https://cdn.example.com/og.png"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "section",
                        "attributes": {
                            "onclick": "alert(1)",
                            "data-safe": "yes"
                        },
                        "components": [{
                            "id": "heading",
                            "type": "heading",
                            "tagName": "h1",
                            "content": "Hello <script>alert(1)</script> world"
                        }]
                    }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn resolves_page_by_id_slug_and_index() {
        let document = document();
        assert_eq!(resolve_page(&document, &PageSelection::First).unwrap().0, 0);
        assert_eq!(
            resolve_page(&document, &PageSelection::Id("home".to_string()))
                .unwrap()
                .0,
            0
        );
        assert_eq!(
            resolve_page(&document, &PageSelection::Slug("home".to_string()))
                .unwrap()
                .0,
            0
        );
    }

    #[test]
    fn storefront_renderer_sanitizes_html_and_emits_metadata() {
        let rendered = render_page(
            &document(),
            &PageSelection::First,
            &RenderPolicy {
                instrument_components: true,
                ..RenderPolicy::default()
            },
        )
        .expect("render page");
        assert!(rendered.html.contains("data-safe=\"yes\""));
        assert!(!rendered.html.contains("onclick="));
        assert!(!rendered.html.contains("<script>alert(1)</script>"));
        assert!(rendered.css.contains("@media (max-width: 767px)"));
        assert!(rendered.html.contains("data-fly-style-id=\"hero\""));
        assert_eq!(rendered.head.title.as_deref(), Some("Home title"));
        assert!(rendered.document_html().contains("property=\"og:image\""));
    }

    #[test]
    fn storefront_renderer_uses_style_hooks_without_editor_instrumentation() {
        let rendered = render_page(&document(), &PageSelection::First, &RenderPolicy::default())
            .expect("render page");
        assert!(!rendered.html.contains("data-fly-component-id"));
        assert!(rendered.html.contains("data-fly-style-id=\"hero\""));
        assert!(rendered.css.contains("data-fly-style-id"));
    }

    #[test]
    fn style_hooks_can_be_disabled_with_project_css() {
        let rendered = render_page(
            &document(),
            &PageSelection::First,
            &RenderPolicy {
                emit_style_hooks: false,
                ..RenderPolicy::default()
            },
        )
        .expect("render page");
        assert!(!rendered.html.contains("data-fly-style-id"));
        assert!(rendered.css.is_empty());
    }
}
