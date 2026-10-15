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

/// One `hreflang` alternate for a page: locale tag plus the absolute or root-relative URL of
/// the same page in that locale. Supplied by the consumer, which owns localized routing; Fly
/// only validates and renders it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlternateLocaleLink {
    pub locale: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PageHead {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub robots: Option<String>,
    pub open_graph_title: Option<String>,
    pub open_graph_description: Option<String>,
    pub open_graph_image: Option<String>,
    pub open_graph_type: Option<String>,
    pub twitter_card: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
    pub twitter_image: Option<String>,
    /// JSON-LD documents, rendered as `application/ld+json` blocks with script-breakout
    /// escaping.
    pub structured_data: Vec<Value>,
    pub alternate_locale_links: Vec<AlternateLocaleLink>,
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
            open_graph_type: metadata.open_graph_type.clone(),
            twitter_card: metadata.twitter_card.clone().or_else(|| {
                metadata
                    .effective_twitter_image()
                    .map(|_| "summary_large_image".to_string())
            }),
            twitter_title: metadata.effective_twitter_title().map(ToString::to_string),
            twitter_description: metadata
                .effective_twitter_description()
                .map(ToString::to_string),
            twitter_image: metadata
                .effective_twitter_image()
                .and_then(|url| safe_head_url(url, true)),
            structured_data: metadata.structured_data.clone(),
            alternate_locale_links: Vec::new(),
        }
    }

    /// Attach validated `hreflang` alternates. Entries with an unsafe URL or a malformed locale
    /// tag are dropped, matching the fail-closed URL posture of the rest of the head.
    pub fn with_alternate_locale_links(mut self, links: Vec<AlternateLocaleLink>) -> Self {
        self.alternate_locale_links = links
            .into_iter()
            .filter(|link| {
                safe_head_url(&link.url, false).is_some()
                    && !link.locale.is_empty()
                    && link.locale.len() <= 35
                    && link
                        .locale
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
            })
            .collect();
        self
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
        push_meta(
            &mut html,
            "property",
            "og:type",
            self.open_graph_type.as_deref(),
        );
        push_meta(
            &mut html,
            "name",
            "twitter:card",
            self.twitter_card.as_deref(),
        );
        push_meta(
            &mut html,
            "name",
            "twitter:title",
            self.twitter_title.as_deref(),
        );
        push_meta(
            &mut html,
            "name",
            "twitter:description",
            self.twitter_description.as_deref(),
        );
        push_meta(
            &mut html,
            "name",
            "twitter:image",
            self.twitter_image.as_deref(),
        );
        if let Some(canonical_url) = &self.canonical_url {
            html.push_str("<link rel=\"canonical\" href=\"");
            html.push_str(&escape_attribute(canonical_url));
            html.push_str("\">");
        }
        for link in &self.alternate_locale_links {
            html.push_str("<link rel=\"alternate\" hreflang=\"");
            html.push_str(&escape_attribute(&link.locale));
            html.push_str("\" href=\"");
            html.push_str(&escape_attribute(&link.url));
            html.push_str("\">");
        }
        for document in &self.structured_data {
            if !document.is_object() && !document.is_array() {
                continue;
            }
            let Ok(serialized) = serde_json::to_string(document) else {
                continue;
            };
            html.push_str("<script type=\"application/ld+json\">");
            // A JSON string inside the payload can contain `</script>`; unicode-escaping the
            // markup-significant characters keeps the JSON semantically identical while making
            // script-element breakout impossible.
            html.push_str(
                &serialized
                    .replace('<', "\\u003c")
                    .replace('>', "\\u003e")
                    .replace('&', "\\u0026"),
            );
            html.push_str("</script>");
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
    render_page_with_rich_text(document, selection, policy, None)
}

/// Render a page with a host rich-text renderer bound (see
/// [`crate::RichTextHtmlRenderer`]). Components carrying a `flyRichText` payload render
/// through that port; without one they use Fly's escaped fallback projection.
pub fn render_page_with_rich_text(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
) -> FlyResult<RenderedPage> {
    // Resolve references before rendering, including for non-runtime previews.
    // Published materialization has already resolved them; the fast path keeps
    // its document and hash identity untouched.
    let resolved = if crate::document_has_symbol_instances(document) {
        Some(crate::resolve_symbol_instances(document)?)
    } else {
        None
    };
    let document = resolved.as_ref().unwrap_or(document);
    let (page_index, page) = resolve_page(document, selection)?;
    let root = page
        .component
        .as_ref()
        .ok_or_else(|| FlyError::MissingPageRoot(page_index.to_string()))?;
    // Components targeted by toggle/scroll/animate actions must be addressable in the
    // published DOM. Instrumentation attributes are off in storefront renders, so targeted
    // components additionally get a plain `id` attribute (see `render_component`).
    let action_targets = action_target_ids(document);
    let mut html = String::new();
    render_node(root, None, 0, policy, rich_text, &action_targets, &mut html);
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

/// Component ids that toggle/scroll/animate actions point at, collected from both the
/// authored `flyAction` extension and the materialized `data-fly-action` attribute, so
/// addressability works before and after publish materialization.
fn action_target_ids(document: &ProjectDocument) -> std::collections::BTreeSet<String> {
    let mut targets = std::collections::BTreeSet::new();
    for page in &document.project.pages {
        if let Some(root) = page.component.as_ref() {
            collect_action_targets(root, &mut targets);
        }
    }
    targets
}

fn collect_action_targets(node: &ComponentNode, targets: &mut std::collections::BTreeSet<String>) {
    let ComponentNode::Object(component) = node else {
        return;
    };
    let authored = component
        .extensions
        .get(crate::FLY_ACTION_FIELD)
        .and_then(|raw| serde_json::from_value::<crate::ComponentAction>(raw.clone()).ok());
    let materialized = component
        .attributes
        .get(crate::FLY_ACTION_DATA_ATTRIBUTE)
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<crate::ComponentAction>(raw).ok());
    for action in authored.into_iter().chain(materialized) {
        match action {
            crate::ComponentAction::ToggleVisibility { target_id, .. }
            | crate::ComponentAction::ScrollTo { target_id }
            | crate::ComponentAction::Animate { target_id, .. } => {
                targets.insert(target_id);
            }
            _ => {}
        }
    }
    for child in component.children() {
        collect_action_targets(child, targets);
    }
}

fn render_node(
    node: &ComponentNode,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
    action_targets: &std::collections::BTreeSet<String>,
    output: &mut String,
) {
    match node {
        ComponentNode::Object(component) => render_component(
            component,
            parent_id,
            index,
            policy,
            rich_text,
            action_targets,
            output,
        ),
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
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
    action_targets: &std::collections::BTreeSet<String>,
    output: &mut String,
) {
    let component_id = component.id.as_deref();
    if let Some(raw_embed) = component.extensions.get(crate::FLY_EMBED_FIELD) {
        render_embed(
            component,
            raw_embed,
            parent_id,
            index,
            policy,
            action_targets,
            output,
        );
        return;
    }
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
    // Action targets get a real `id` so the published-page runtime can find them without
    // editor instrumentation (which storefront renders disable). An author-supplied `id`
    // attribute always wins; this only fills the gap.
    if let Some(component_id) = component_id
        && action_targets.contains(component_id)
        && !component
            .attributes
            .keys()
            .any(|name| name.eq_ignore_ascii_case("id"))
    {
        write_attribute(output, "id", component_id);
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

    if let Some(raw_payload) = component.extensions.get(crate::FLY_RICH_TEXT_FIELD) {
        // A rich-text component is a leaf: the payload replaces both the plain `content`
        // string and any children, which are preserved in the document but not rendered.
        match crate::decode_rich_text_payload(raw_payload) {
            Ok(payload) => {
                let rendered = rich_text
                    .and_then(|renderer| renderer.render_rich_text(&payload))
                    .unwrap_or_else(|| crate::rich_text_fallback_html(&payload));
                output.push_str(&rendered);
            }
            Err(_) => {
                // Invalid payloads are a validation error at the project gate; rendering
                // degrades to nothing rather than echoing unvalidated structure.
            }
        }
    } else {
        if let Some(content) = component.extensions.get("content").and_then(Value::as_str) {
            push_escaped_html(output, content);
        }
        for (child_index, child) in component.children().iter().enumerate() {
            render_node(
                child,
                component_id,
                child_index,
                policy,
                rich_text,
                action_targets,
                output,
            );
        }
    }
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
}

/// Render an embed component: sandboxed iframe for a validated spec, a plain link or escaped
/// title otherwise. Embeds never contribute arbitrary attributes to the iframe; the provider
/// gate in `embed.rs` is the only way a URL reaches `src`.
fn render_embed(
    component: &ComponentObject,
    raw_spec: &Value,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    action_targets: &std::collections::BTreeSet<String>,
    output: &mut String,
) {
    output.push_str("<div");
    if policy.emit_style_hooks
        && let Some(component_id) = component.id.as_deref()
    {
        write_attribute(output, "data-fly-style-id", component_id);
    }
    if policy.instrument_components {
        if let Some(component_id) = component.id.as_deref() {
            write_attribute(output, "data-fly-component-id", component_id);
        }
        write_attribute(output, "data-fly-index", &index.to_string());
        if let Some(parent_id) = parent_id {
            write_attribute(output, "data-fly-parent-id", parent_id);
        }
    }
    if let Some(component_id) = component.id.as_deref()
        && action_targets.contains(component_id)
    {
        write_attribute(output, "id", component_id);
    }
    write_attribute(output, "class", "fly-embed");
    match crate::decode_embed_spec(raw_spec) {
        Ok(spec) => {
            if let Some(ratio) = spec.aspect_ratio.as_deref() {
                let ratio = ratio.replace('/', " / ");
                write_attribute(output, "style", &format!("aspect-ratio:{ratio}"));
            }
            output.push_str("><iframe");
            write_attribute(output, "src", &spec.url);
            write_attribute(output, "title", &spec.title);
            write_attribute(output, "loading", "lazy");
            write_attribute(output, "sandbox", crate::EMBED_IFRAME_SANDBOX);
            write_attribute(output, "referrerpolicy", "strict-origin-when-cross-origin");
            output.push_str(" allowfullscreen></iframe></div>");
        }
        Err(_) => {
            output.push('>');
            let raw_title = raw_spec
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Embedded content");
            let raw_url = raw_spec
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if crate::safe_url::url_allowed(
                raw_url,
                crate::safe_url::UrlAttributeKind::Resource,
                &crate::safe_url::UrlPolicy::permissive(),
            ) && raw_url
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("https://")
            {
                output.push_str("<a");
                write_attribute(output, "href", raw_url);
                write_attribute(output, "rel", "noopener noreferrer");
                output.push('>');
                push_escaped_html(output, raw_title);
                output.push_str("</a>");
            } else {
                push_escaped_html(output, raw_title);
            }
            output.push_str("</div>");
        }
    }
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
    // Design tokens render first so every later rule can reference them. A malformed theme
    // renders nothing (fail-closed); validation reports it as `theme_token_invalid`.
    if let Ok(tokens) = crate::theme::decode_theme_tokens(document)
        && let Some(root_rule) = crate::theme::theme_root_rule(&tokens)
    {
        css.push_str(&root_rule);
    }
    for rule in catalog.rules {
        let declarations = rule
            .declarations
            .iter()
            .filter_map(|(name, value)| safe_style(name, value, policy))
            .collect::<Vec<_>>()
            .join(";");
        if declarations.is_empty() {
            continue;
        }
        // A rule bound to a component renders against that component's style hook; an unbound
        // rule renders only when every selector is a plain class name, so reusable class rules
        // authored in GrapesJS reach the page instead of being silently dropped.
        let selector_base = match rule.component_id.as_deref() {
            Some(component_id) => {
                if !component_ids.contains(component_id) {
                    continue;
                }
                format!(
                    "[data-fly-style-id=\"{}\"]",
                    escape_css_attribute(component_id)
                )
            }
            None => match class_rule_selector(&rule.selector_names) {
                Some(selector) => selector,
                None => continue,
            },
        };
        // Unsupported pseudo-classes are preserved in the document for round-tripping but must
        // never reach a selector: `pseudo_class()` is the allow-list gate.
        let Some(pseudo_suffix) = rule
            .scope
            .pseudo_class()
            .map(|pseudo| format!(":{pseudo}"))
            .or_else(|| {
                matches!(
                    rule.scope,
                    StyleRuleScope::Base | StyleRuleScope::Media { .. }
                )
                .then(String::new)
            })
        else {
            continue;
        };
        let selector = format!("{selector_base}{pseudo_suffix}");
        match rule.scope.media_query() {
            None => push_rule(&mut css, &selector, &declarations),
            Some(query) if safe_media_query(query) => {
                css.push_str("@media ");
                css.push_str(query.trim());
                css.push('{');
                push_rule(&mut css, &selector, &declarations);
                css.push('}');
            }
            Some(_) => {}
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

/// Build a CSS selector from GrapesJS selector names, accepting plain class names only.
///
/// Selector names arrive as author input (`"card"`, `".card"`, or selector objects flattened to
/// their `name`). Anything that is not a single class name — combinators, element selectors,
/// attribute selectors, `#id` — makes the whole rule unrenderable here: a class rule must never
/// become a selector-injection path. Component-bound rules do not go through this function.
fn class_rule_selector(selector_names: &[String]) -> Option<String> {
    if selector_names.is_empty() {
        return None;
    }
    let mut selector = String::new();
    for name in selector_names {
        let class = name.trim().trim_start_matches('.');
        if class.is_empty()
            || class.len() > 128
            || !class.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
            || class
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
        {
            return None;
        }
        selector.push('.');
        selector.push_str(class);
    }
    Some(selector)
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
            "wrapper" | "container" | "row" | "column" | "grid" | "spacer" | "symbol" => "div",
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
const DENIED_STYLE_PROPERTIES: &[&str] =
    &["behavior", "-moz-binding", "-ms-behavior", "expression"];

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
pub(crate) fn push_escaped_html(output: &mut String, value: &str) {
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
pub(crate) fn push_escaped_attribute(output: &mut String, value: &str) {
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
    fn action_targets_get_a_plain_id_without_instrumentation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "trigger",
                            "type": "button",
                            "content": "Toggle",
                            "flyAction": { "kind": "toggle_visibility", "target_id": "panel" }
                        },
                        { "id": "panel", "type": "section", "content": "Details" },
                        { "id": "plain", "type": "section", "content": "Untargeted" }
                    ]
                }
            }]
        }))
        .expect("document");
        let page = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("rendered");
        assert!(page.html.contains(" id=\"panel\""), "html: {}", page.html);
        assert!(!page.html.contains(" id=\"plain\""), "html: {}", page.html);
        assert!(
            !page.html.contains("data-fly-component-id"),
            "test must run without instrumentation: {}",
            page.html
        );
    }

    #[test]
    fn design_tokens_render_as_root_variables_and_survive_var_references() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyTheme": { "tokens": { "brand": "#0b5fff", "space-md": "1rem" } },
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "style": { "color": "var(--brand)", "padding": "var(--space-md)" },
                    "components": []
                }
            }]
        }))
        .expect("document");
        let page = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("rendered");
        assert!(
            page.css.contains(":root{--brand:#0b5fff;--space-md:1rem;}"),
            "css: {}",
            page.css
        );
        assert!(page.css.contains("color:var(--brand)"), "css: {}", page.css);
    }

    #[test]
    fn malformed_theme_renders_no_root_rule() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyTheme": { "tokens": { "Brand": "red; } body { display: none" } },
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper", "components": [] }
            }]
        }))
        .expect("document");
        let page = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("rendered");
        assert!(!page.css.contains(":root"), "css: {}", page.css);
        assert!(!page.css.contains("display: none"), "css: {}", page.css);
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
    fn renderer_emits_pseudo_state_and_class_rules() {
        let document = GrapesJsCodec::decode_value(json!({
            "styles": [
                {
                    "selectors": [{ "name": "hero", "type": 2 }],
                    "state": "hover",
                    "style": { "background": "#1d4ed8" }
                },
                {
                    "selectors": [{ "name": "card", "type": 1 }],
                    "style": { "border-radius": "12px" }
                },
                {
                    "selectors": [{ "name": "hero", "type": 2 }],
                    "atRuleType": "media",
                    "mediaText": "(max-width: 767px)",
                    "state": "focus-visible",
                    "style": { "outline": "2px solid" }
                },
                {
                    "selectors": [{ "name": "hero", "type": 2 }],
                    "state": "nth-child(2)",
                    "style": { "color": "red" }
                },
                {
                    "selectors": ["div > .injected"],
                    "style": { "color": "red" }
                }
            ],
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render page");
        assert!(
            rendered
                .css
                .contains("[data-fly-style-id=\"hero\"]:hover{background:#1d4ed8}"),
            "{}",
            rendered.css
        );
        assert!(
            rendered.css.contains(".card{border-radius:12px}"),
            "{}",
            rendered.css
        );
        assert!(
            rendered.css.contains(
                "@media (max-width: 767px){[data-fly-style-id=\"hero\"]:focus-visible{outline:2px solid}}"
            ),
            "{}",
            rendered.css
        );
        assert!(
            !rendered.css.contains("nth-child"),
            "unsupported pseudo-classes must not reach the stylesheet: {}",
            rendered.css
        );
        assert!(
            !rendered.css.contains("injected"),
            "non-class selectors must not reach the stylesheet: {}",
            rendered.css
        );
    }

    #[test]
    fn renderer_emits_twitter_og_type_and_json_ld_head() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Welcome",
                    "open_graph_type": "article",
                    "twitter_card": "summary",
                    "structured_data": [{
                        "@context": "https://schema.org",
                        "@type": "WebPage",
                        "name": "Home </script><script>alert(1)</script>"
                    }]
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render page");
        let html = rendered.document_html();
        assert!(
            html.contains("property=\"og:type\" content=\"article\""),
            "{html}"
        );
        assert!(
            html.contains("name=\"twitter:card\" content=\"summary\""),
            "{html}"
        );
        assert!(
            html.contains("name=\"twitter:title\" content=\"Home\""),
            "twitter title falls back to the page title: {html}"
        );
        assert!(html.contains("application/ld+json"), "{html}");
        assert!(
            html.contains("\\u003c/script\\u003e"),
            "JSON-LD payload must not terminate the script element: {html}"
        );
        assert!(!html.contains("<script>alert(1)</script>"), "{html}");
    }

    #[test]
    fn arbitrary_state_strings_only_render_allow_listed_pseudo_classes() {
        for state in [
            "hover",
            ":FOCUS",
            " focus-visible ",
            "nth-child(2)",
            "hover}body{display:none",
            "hover, .injected",
            "",
        ] {
            let document = GrapesJsCodec::decode_value(json!({
                "styles": [{
                    "selectors": [{ "name": "hero", "type": 2 }],
                    "state": state,
                    "style": { "color": "red" }
                }],
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }))
            .expect("document");
            let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
                .expect("render page");
            if crate::is_supported_pseudo_class(state) {
                let pseudo = crate::normalize_pseudo_class(state).expect("normalized");
                assert!(
                    rendered
                        .css
                        .contains(&format!("[data-fly-style-id=\"hero\"]:{pseudo}")),
                    "state `{state}` must render as :{pseudo}: {}",
                    rendered.css
                );
            } else {
                assert!(
                    !rendered.css.contains("color:red"),
                    "state `{state}` must not render: {}",
                    rendered.css
                );
            }
            assert!(!rendered.css.contains("injected"), "{}", rendered.css);
            assert!(!rendered.css.contains("display:none"), "{}", rendered.css);
        }
    }

    #[test]
    fn head_alternate_locale_links_are_validated() {
        let head = PageHead::default().with_alternate_locale_links(vec![
            AlternateLocaleLink {
                locale: "en".to_string(),
                url: "https://example.com/en/home".to_string(),
            },
            AlternateLocaleLink {
                locale: "ru".to_string(),
                url: "javascript:alert(1)".to_string(),
            },
            AlternateLocaleLink {
                locale: "bad locale!".to_string(),
                url: "https://example.com/home".to_string(),
            },
        ]);
        let html = head.render_html();
        assert!(html.contains("hreflang=\"en\""), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(!html.contains("bad locale"), "{html}");
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
                safe_style(
                    "background-image",
                    &Value::String(value.to_string()),
                    &policy
                )
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
                safe_style(
                    "background-image",
                    &Value::String(value.to_string()),
                    &policy
                )
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
        assert!(url_allowed(
            "#hero-map",
            UrlAttributeKind::Fragment,
            &policy
        ));
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
        for attribute in [
            "href",
            "src",
            "poster",
            "action",
            "formaction",
            "cite",
            "usemap",
        ] {
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
