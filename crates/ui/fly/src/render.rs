use crate::safe_url::{
    self, UrlAttributeKind, UrlPolicy, absolute_url_has_authority, normalized_url_candidate,
    safe_data_image,
};
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
            canonical_url: metadata
                .canonical_url
                .as_deref()
                .and_then(|url| safe_head_url(url, false)),
            robots: metadata.no_index.then_some("noindex,nofollow".to_string()),
            open_graph_title: metadata
                .effective_open_graph_title()
                .map(ToString::to_string),
            open_graph_description: metadata
                .effective_open_graph_description()
                .map(ToString::to_string),
            open_graph_image: metadata
                .open_graph_image
                .as_deref()
                .and_then(|url| safe_head_url(url, true)),
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
        compose_document_html(&self.head, &self.css, &self.html)
    }
}

/// Compose one complete, standalone HTML document from a typed page head, stylesheet and body.
///
/// Keeping this function in the renderer core gives all adapters the same deterministic document
/// envelope instead of rebuilding it in Leptos, Dioxus or transport code.
pub fn compose_document_html(head: &PageHead, css: &str, body_html: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">{}<style>{}</style></head><body>{}</body></html>",
        head.render_html(),
        escape_style_element_text(css),
        body_html,
    )
}

/// Make a stylesheet safe to embed inside a raw `<style>` element.
///
/// The HTML tokenizer leaves "raw text" mode at the first literal `</` regardless of CSS syntax,
/// so any `</style>` reaching this point would end the element and let the remainder of the
/// stylesheet be parsed as markup. CSS string/identifier escapes are invisible to the HTML
/// tokenizer but fully understood by the CSS parser, so escaping the `<` keeps the stylesheet
/// semantically identical while removing the breakout. `<!--` is neutralised for the same reason.
fn escape_style_element_text(css: &str) -> String {
    if !css.contains('<') {
        return css.to_string();
    }
    css.replace('<', "\\00003c ")
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
        render_project_styles(document, page, policy)
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
    let void_tag = matches!(tag, "img" | "input" | "hr" | "br" | "source");

    output.push('<');
    output.push_str(tag);
    if policy.emit_style_hooks
        && let Some(component_id) = component_id
    {
        write_attribute(output, "data-fly-style-id", component_id);
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
        let name = name.to_ascii_lowercase();
        if !safe_attribute_name(&name)
            || renderer_manages_attribute(&name, policy)
            || matches!(
                name.as_str(),
                "style" | "srcdoc" | "srcset" | "xlink:href" | "ping" | "background"
            )
        {
            continue;
        }
        if let Value::Bool(enabled) = value {
            if *enabled {
                output.push(' ');
                output.push_str(&name);
            }
            continue;
        }
        let Some(value) = scalar_string(value) else {
            continue;
        };
        if let Some(kind) = UrlAttributeKind::for_attribute(&name)
            && !url_allowed(&value, kind, policy)
        {
            continue;
        }
        write_attribute(output, &name, &value);
    }

    if !policy.emit_style_hooks
        && let Some(style) = component.style.as_ref().and_then(Value::as_object)
    {
        let declarations = style
            .iter()
            .filter_map(|(name, value)| safe_style(name, value, policy))
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
        push_escaped_html(output, content);
    }
    for (child_index, child) in component.children().iter().enumerate() {
        render_node(child, component_id, child_index, policy, output);
    }
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
}

fn render_project_styles(
    document: &ProjectDocument,
    page: &ProjectPage,
    policy: &RenderPolicy,
) -> String {
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
            .filter_map(|(name, value)| safe_style(name, value, policy))
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
    if let Some(root) = page.component.as_ref() {
        append_component_style_rules(root, policy, &mut css);
    }
    css
}

fn append_component_style_rules(node: &ComponentNode, policy: &RenderPolicy, css: &mut String) {
    let ComponentNode::Object(component) = node else {
        return;
    };
    if let (Some(component_id), Some(style)) = (
        component.id.as_deref(),
        component.style.as_ref().and_then(Value::as_object),
    ) {
        let declarations = style
            .iter()
            .filter_map(|(name, value)| safe_style(name, value, policy))
            .collect::<Vec<_>>()
            .join(";");
        if !declarations.is_empty() {
            let selector = format!(
                "[data-fly-style-id=\"{}\"]",
                escape_css_attribute(component_id)
            );
            push_rule(css, &selector, &declarations);
        }
    }
    for child in component.children() {
        append_component_style_rules(child, policy, css);
    }
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

fn renderer_manages_attribute(name: &str, policy: &RenderPolicy) -> bool {
    (policy.emit_style_hooks && name == "data-fly-style-id")
        || (policy.instrument_components
            && matches!(
                name,
                "data-fly-component-id" | "data-fly-index" | "data-fly-parent-id"
            ))
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

fn safe_head_url(value: &str, allow_data_image: bool) -> Option<String> {
    let value = normalized_url_candidate(value)?;
    let normalized = value.to_ascii_lowercase();
    if value.starts_with('/')
        || absolute_url_has_authority(value, "http://")
        || absolute_url_has_authority(value, "https://")
        || (allow_data_image && safe_data_image(&normalized))
    {
        Some(value.to_string())
    } else {
        None
    }
}

impl RenderPolicy {
    /// Project the render policy onto the shared URL policy.
    fn url_policy(&self) -> UrlPolicy {
        UrlPolicy {
            allow_http: self.allow_http,
            allow_https: self.allow_https,
            allow_relative_urls: self.allow_relative_urls,
            allow_hash_urls: self.allow_hash_urls,
            allow_mailto: self.allow_mailto,
            allow_tel: self.allow_tel,
            allow_data_images: self.allow_data_images,
        }
    }
}

fn url_allowed(value: &str, kind: UrlAttributeKind, policy: &RenderPolicy) -> bool {
    safe_url::url_allowed(value, kind, &policy.url_policy())
}



/// Properties that can execute script regardless of their value.
const DENIED_STYLE_PROPERTIES: &[&str] = &[
    "behavior",
    "-moz-binding",
    "-ms-behavior",
    "expression",
];

/// Decide whether a single CSS declaration may be emitted.
///
/// `url(...)` used to be rejected outright, which is safe but wrong for a page builder: it made
/// `background-image` unusable. References are now allowed when the URL inside them satisfies the
/// same resource policy the renderer applies to `src`, so `url(/hero.png)` works while
/// `url(javascript:...)` and `url(data:text/html,...)` remain blocked.
///
/// # The invariant this upholds
///
/// Within one layer, a URL must be judged the same whether it appears in an attribute or inside
/// `url()`. Accepting an image through `src` and refusing the identical image through
/// `background-image` is not a stricter policy, it is an incoherent one.
///
/// Layers may still differ from each other, and do: static publishing refuses plain `http://`
/// where this renderer allows it. That is a coherent difference, because publishing applies the
/// same stricter rule to attributes *and* references — see `FORBIDDEN_CSS_TOKENS` and
/// `strip_validated_css_urls` in `rustok-page-builder`'s `static_publish_policy`.
fn safe_style(name: &str, value: &Value, policy: &RenderPolicy) -> Option<String> {
    if name.is_empty()
        || name.starts_with("--")
        || !name
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '-')
    {
        return None;
    }
    // These properties execute code by design, so no value is safe. Previously they were blocked
    // only as a side effect of banning `url(` outright; now that safe references are permitted,
    // the property itself has to be refused.
    if DENIED_STYLE_PROPERTIES.contains(&name.to_ascii_lowercase().as_str()) {
        return None;
    }
    let value = scalar_string(value)?;
    if value.chars().any(char::is_control) {
        return None;
    }

    // Validate and blank out every `url(...)` token *before* the structural check, because a
    // legitimate data-image reference legally contains `;` (`data:image/png;base64,...`) and the
    // structural check must not see it. Characters that are dangerous *inside* a reference are
    // rejected by `url_allowed` and by the stricter check in the stripping pass.
    let without_urls = validate_and_strip_url_tokens(&value, policy)?;

    // Structural characters in what remains are what would let a declaration escape into a new
    // rule or a comment.
    if without_urls.contains('\\')
        || without_urls.contains('<')
        || without_urls.contains('>')
        || without_urls.contains(';')
        || without_urls.contains('{')
        || without_urls.contains('}')
        || without_urls.contains("/*")
        || without_urls.contains("*/")
    {
        return None;
    }

    let compact = without_urls
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.contains("expression(")
        || compact.contains("javascript:")
        || compact.contains("@import")
        || compact.contains("behavior:")
        || compact.contains("-moz-binding")
        || compact.contains("data:")
        || compact.contains("url(")
    {
        return None;
    }

    Some(format!("{name}:{value}"))
}

/// Replace each well-formed `url(...)` token with a placeholder, rejecting the declaration if any
/// of them fails the resource URL policy.
fn validate_and_strip_url_tokens(value: &str, policy: &RenderPolicy) -> Option<String> {
    let lowered = value.to_ascii_lowercase();
    // Built once rather than per token: a declaration may carry several references.
    let url_policy = policy.url_policy();
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0usize;

    while let Some(offset) = lowered[cursor..].find("url(") {
        let start = cursor + offset;
        // Something like `blurl(` is not a reference; require a non-identifier character before.
        let preceded_by_identifier = value[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '-');
        if preceded_by_identifier {
            output.push_str(&value[cursor..start + 4]);
            cursor = start + 4;
            continue;
        }

        let open = start + 4;
        // An unterminated reference is rejected outright rather than guessed at.
        let close = value[open..].find(')')? + open;
        let raw = value[open..close].trim();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                raw.strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            })
            .unwrap_or(raw);

        // `;` is legal inside a data URL, but these would still let the reference break out of
        // the declaration or open a comment.
        if unquoted.is_empty()
            || unquoted.contains('{')
            || unquoted.contains('}')
            || unquoted.contains('<')
            || unquoted.contains('>')
            || unquoted.contains("/*")
            || unquoted.contains("*/")
            || !safe_url::url_allowed(unquoted, UrlAttributeKind::Resource, &url_policy)
        {
            return None;
        }

        output.push_str(&value[cursor..start]);
        output.push_str("url-ok");
        cursor = close + 1;
    }

    output.push_str(&value[cursor..]);
    Some(output)
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
        && !normalized.contains("@import")
        && !normalized.contains('\\')
        && normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "() :.-_%/,".contains(character))
}

fn write_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    push_escaped_attribute(output, value);
    output.push('"');
}

fn render_opaque(value: &Value, output: &mut String) {
    match value {
        Value::String(value) => push_escaped_html(output, value),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        _ => {}
    }
}

/// Append `value` to `output` as HTML text content.
///
/// Escaping alone is the whole defence here. The renderer previously also ran a hand-rolled
/// `strip_tags` pass first, which was both redundant (the escape already neutralises markup) and
/// lossy: authored text such as `5 < 10 and 3 > 2` silently lost everything between the angle
/// brackets.
fn push_escaped_html(output: &mut String, value: &str) {
    output.reserve(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

/// Append `value` to `output` as a double-quoted attribute value.
fn push_escaped_attribute(output: &mut String, value: &str) {
    output.reserve(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
}

fn escape_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    push_escaped_html(&mut output, value);
    output
}

fn escape_attribute(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    push_escaped_attribute(&mut output, value);
    output
}

/// Escape a value for use inside a double-quoted CSS attribute-selector string.
///
/// Allow-list based on purpose: anything that is not an unambiguously inert identifier character
/// is emitted as a CSS numeric escape (`\\XX `). A deny-list here is not sufficient, because the
/// result is embedded in a `<style>` element and therefore has to survive both the CSS grammar
/// and the HTML tokenizer.
fn escape_css_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':') {
            escaped.push(character);
        } else {
            escaped.push_str(&format!("\\{:x} ", character as u32));
        }
    }
    escaped
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
                        "style": { "margin-top": "12px" },
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
        assert!(rendered.css.contains("margin-top:12px"));
        assert!(!rendered.html.contains(" style=\""));
    }

    #[test]
    fn renderer_normalizes_attribute_names_before_policy_checks() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "link",
                        "type": "link",
                        "content": "Open",
                        "attributes": {
                            "HREF": "javascript:alert(1)",
                            "STYLE": "color:red",
                            "DATA-SAFE": "yes"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render page");

        assert!(
            rendered.html.contains(r#"data-safe="yes""#),
            "{}",
            rendered.html
        );
        assert!(!rendered.html.contains("DATA-SAFE"), "{}", rendered.html);
        assert!(!rendered.html.contains("javascript:"), "{}", rendered.html);
        assert!(!rendered.html.contains("STYLE="), "{}", rendered.html);
        assert!(!rendered.html.contains(" style="), "{}", rendered.html);
    }

    #[test]
    fn renderer_emits_source_as_void_element() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "picture",
                        "type": "media",
                        "tagName": "picture",
                        "components": [{
                            "id": "source",
                            "type": "source",
                            "tagName": "source",
                            "attributes": {
                                "src": "https://cdn.example.com/hero.webp",
                                "type": "image/webp"
                            }
                        }]
                    }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render page");

        assert!(rendered.html.contains("<source"), "{}", rendered.html);
        assert!(!rendered.html.contains("</source>"), "{}", rendered.html);
    }

    #[test]
    fn url_policy_distinguishes_navigation_resources_and_forms() {
        let policy = RenderPolicy::default();
        assert!(url_allowed(
            "pricing?ref=home",
            UrlAttributeKind::Navigation,
            &policy
        ));
        assert!(url_allowed(
            "/images/hero.webp",
            UrlAttributeKind::Resource,
            &policy
        ));
        assert!(url_allowed(
            "data:image/png;base64,AAAA",
            UrlAttributeKind::Resource,
            &policy
        ));
        assert!(!url_allowed(
            "data:image/svg+xml,<svg/>",
            UrlAttributeKind::Resource,
            &policy
        ));
        assert!(url_allowed(
            "https://example.com/submit",
            UrlAttributeKind::FormAction,
            &policy
        ));
        assert!(url_allowed(
            "/contact",
            UrlAttributeKind::FormAction,
            &policy
        ));
    }

    #[test]
    fn url_policy_rejects_scheme_relative_controls_and_backslashes() {
        let policy = RenderPolicy::default();
        for value in [
            "//evil.example/x",
            "javascript:alert(1)",
            "http://",
            "mailto:",
            "\\evil.example",
            "a\n/b",
            "/has space",
        ] {
            assert!(!url_allowed(value, UrlAttributeKind::Navigation, &policy));
        }
    }

    #[test]
    fn style_policy_rejects_resource_loading_and_custom_properties() {
        let policy = RenderPolicy::default();
        assert!(safe_style("color", &Value::String("red".to_string()), &policy).is_some());
        for (name, value) in [
            ("--payload", "red"),
            ("background", "u r l(https://evil.example/x)"),
            ("color", "\\75rl(https://evil.example/x)"),
            ("behavior", "url(x.htc)"),
        ] {
            assert!(
                safe_style(name, &Value::String(value.to_string()), &policy).is_none(),
                "accepted {name}: {value}"
            );
        }
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

    #[test]
    fn renderer_does_not_duplicate_managed_fly_attributes() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "attributes": {
                        "data-fly-style-id": "attacker",
                        "data-fly-component-id": "attacker"
                    }
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(
            &document,
            &PageSelection::First,
            &RenderPolicy {
                instrument_components: true,
                ..RenderPolicy::default()
            },
        )
        .expect("render page");
        assert_eq!(rendered.html.matches("data-fly-style-id").count(), 1);
        assert_eq!(rendered.html.matches("data-fly-component-id").count(), 1);
        assert!(!rendered.html.contains("attacker"));
    }

    #[test]
    fn renderer_filters_unsafe_head_urls() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "flyPageMeta": {
                    "canonical_url": "javascript:alert(1)",
                    "open_graph_image": "data:image/svg+xml,<svg/>"
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render page");
        let head = rendered.head.render_html();
        assert!(!head.contains("javascript:"));
        assert!(!head.contains("data:image/svg"));
        assert!(!head.contains("canonical"));
        assert!(!head.contains("og:image"));
    }

    #[test]
    fn safe_url_references_are_allowed_in_declarations() {
        let policy = RenderPolicy::default();
        for value in [
            "url(/hero.png)",
            "url(\"/hero.png\")",
            "url('/hero.png')",
            "url(https://cdn.example/hero.png)",
            "url(  /hero.png  )",
            "linear-gradient(red, blue), url(/hero.png)",
            "url(data:image/png;base64,iVBORw0KGgo=)",
        ] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy)
                    .is_some(),
                "rejected legitimate {value}"
            );
        }
    }

    #[test]
    fn hostile_url_references_are_still_refused() {
        let policy = RenderPolicy::default();
        for value in [
            "url(javascript:alert(1))",
            "url(data:text/html;base64,PHNjcmlwdD4=)",
            "url(//attacker.example/x.png)",
            "url()",
            "url(/a.png) ; background: url(javascript:alert(1))",
            "url(/unterminated.png",
        ] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy)
                    .is_none(),
                "accepted hostile {value}"
            );
        }
    }

    #[test]
    fn a_disabled_resource_scheme_also_disables_it_inside_url() {
        // The declaration path must honour the same policy flags as the attribute path, rather
        // than quietly applying a more permissive rule of its own.
        let policy = RenderPolicy {
            allow_data_images: false,
            ..RenderPolicy::default()
        };
        assert!(
            safe_style(
                "background-image",
                &Value::String("url(data:image/png;base64,iVBORw0KGgo=)".to_string()),
                &policy
            )
            .is_none()
        );
    }

    #[test]
    fn component_id_cannot_break_out_of_the_style_element() {
        let hostile_id = "x\"]{}</style><script>alert(1)</script><style>{a:b";
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": hostile_id,
                        "type": "section",
                        "style": { "margin-top": "12px" }
                    }]
                }
            }]
        }))
        .expect("decode");

        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render");
        let html = rendered.document_html();

        // Exactly one `</style>`: the real closing tag. A second one would mean the stylesheet
        // ended the element early and the rest of it is being parsed as markup.
        assert_eq!(
            html.matches("</style>").count(),
            1,
            "stylesheet terminated the style element: {html}"
        );
        assert!(
            !html.contains("<script>"),
            "stylesheet injected markup: {html}"
        );
        assert!(
            !rendered.css.contains('<'),
            "raw `<` survived CSS escaping: {}",
            rendered.css
        );
        assert!(
            rendered.css.contains("margin-top:12px"),
            "escaping dropped the declaration: {}",
            rendered.css
        );
    }

    #[test]
    fn text_content_keeps_angle_brackets_as_literal_text() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "note",
                        "type": "text",
                        "content": "5 < 10 and 3 > 2 & true"
                    }]
                }
            }]
        }))
        .expect("decode");

        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render");

        assert!(
            rendered.html.contains("5 &lt; 10 and 3 &gt; 2 &amp; true"),
            "authored text was mangled: {}",
            rendered.html
        );
    }

    #[test]
    fn style_element_escaping_preserves_ordinary_css() {
        let css = "[data-fly-style-id=\"hero\"]{margin-top:12px}";
        assert_eq!(escape_style_element_text(css), css);
    }

    #[test]
    fn fragment_attributes_must_point_inside_the_document() {
        let policy = RenderPolicy::default();
        assert!(url_allowed("#hero-map", UrlAttributeKind::Fragment, &policy));
        for value in [
            "https://evil.example/map",
            "/local/map",
            "#",
            "javascript:alert(1)",
        ] {
            assert!(
                !url_allowed(value, UrlAttributeKind::Fragment, &policy),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn every_url_attribute_the_publish_policy_classifies_is_classified_here_too() {
        // The two layers must agree on *what counts as a URL*. Where they disagreed, an
        // attribute was validated at publish and emitted unchecked by the renderer.
        for attribute in ["href", "src", "poster", "action", "formaction", "cite", "usemap"] {
            assert!(
                UrlAttributeKind::for_attribute(attribute).is_some(),
                "`{attribute}` carries a URL but the renderer does not classify it"
            );
        }
    }

    #[test]
    fn hostile_cite_and_usemap_values_never_reach_the_output() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "quote",
                        "type": "text",
                        "tagName": "blockquote",
                        "attributes": {
                            "cite": "javascript:alert(1)",
                            "usemap": "https://evil.example/map"
                        },
                        "content": "Quoted"
                    }]
                }
            }]
        }))
        .expect("decode");

        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render");
        assert!(!rendered.html.contains("javascript:"), "{}", rendered.html);
        assert!(!rendered.html.contains("evil.example"), "{}", rendered.html);
        assert!(rendered.html.contains("Quoted"), "{}", rendered.html);
    }

    #[test]
    fn legitimate_cite_and_usemap_values_survive() {
        // Tightening must not make the attributes unusable.
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "quote",
                        "type": "text",
                        "tagName": "blockquote",
                        "attributes": {
                            "cite": "https://example.com/source",
                            "usemap": "#hero-map"
                        },
                        "content": "Quoted"
                    }]
                }
            }]
        }))
        .expect("decode");

        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render");
        assert!(
            rendered.html.contains("https://example.com/source"),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains("#hero-map"), "{}", rendered.html);
    }
}

