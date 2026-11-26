use crate::safe_url::{self, UrlAttributeKind, UrlPolicy};
use crate::style_rule::{class_rule_selector, safe_media_query};
use crate::{
    ComponentNode, ComponentObject, FlyError, FlyResult, PageMetadata, ProjectDocument,
    ProjectPage, StyleRuleCatalog, StyleRuleScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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

/// The fixed opening of the document envelope, up to the page head.
const DOCUMENT_ENVELOPE_OPEN: &str = concat!(
    "<!doctype html><html><head><meta charset=\"utf-8\">",
    "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
);
/// The opening tag of the one stylesheet element the envelope writes.
const DOCUMENT_STYLE_OPEN: &str = "<style>";

/// Compose one complete, standalone HTML document from a typed page head, stylesheet and body.
///
/// Keeping this function in the renderer core gives all adapters the same deterministic document
/// envelope instead of rebuilding it in Leptos, Dioxus or transport code.
pub fn compose_document_html(head: &PageHead, css: &str, body_html: &str) -> String {
    let head_html = head.render_html();
    let css = escape_style_element_text(css);
    let mut html = String::with_capacity(
        DOCUMENT_ENVELOPE_OPEN.len() + head_html.len() + css.len() + body_html.len() + 64,
    );
    html.push_str(DOCUMENT_ENVELOPE_OPEN);
    html.push_str(&head_html);
    html.push_str(DOCUMENT_STYLE_OPEN);
    html.push_str(&css);
    html.push_str("</style></head><body>");
    html.push_str(body_html);
    html.push_str("</body></html>");
    html
}

/// The stylesheet of a document [`compose_document_html`] wrote, exactly as the document carries
/// it.
///
/// A host that serves a composed document as its own response has to name the stylesheet's hash in
/// the policy of that response. The document travels as one string and the caller cannot re-derive
/// the CSS from the page model — the envelope escaped style-element text on the way in — so the
/// policy is derived from the rendered bytes instead: one fact, one place.
///
/// Returns `None` for markup that is not this envelope. The caller then has no stylesheet to
/// name and must not guess one.
pub fn document_html_stylesheet(document_html: &str) -> Option<&str> {
    let after_open = document_html.strip_prefix(DOCUMENT_ENVELOPE_OPEN)?;
    let style_open = after_open.find(DOCUMENT_STYLE_OPEN)?;
    // A style in the body is not the envelope's head stylesheet. Head data emitted by
    // PageHead cannot contain a literal closing head tag, so reject premature termination.
    if after_open.as_bytes()[..style_open]
        .windows(b"</head".len())
        .any(|window| window.eq_ignore_ascii_case(b"</head"))
    {
        return None;
    }
    let rest = &after_open[style_open + DOCUMENT_STYLE_OPEN.len()..];
    let close = rest.find("</style>")?;
    if !rest[close..].starts_with("</style></head><body>")
        || !document_html.ends_with("</body></html>")
    {
        return None;
    }
    Some(&rest[..close])
}

/// Return a composed document with `nonce` on the stylesheet element of its envelope.
///
/// A document shown through `srcdoc` inherits the Content Security Policy of the document that
/// embeds it. Under a policy that allows inline styles by nonce only, the browser drops the
/// envelope stylesheet unless it carries the nonce of the embedding response. That nonce is
/// known to the embedder and not to the renderer, and a composed document travels as one
/// string, so the embedder applies it here.
///
/// The envelope is recognised by its fixed opening. The page head that follows is escaped (see
/// [`PageHead::render_html`]) and holds no literal `<style>`, so the first one after the opening
/// is the element [`compose_document_html`] wrote. No other element receives the nonce: a
/// `<style>` in the body is authored markup, and the embedder does not vouch for it.
///
/// Returns `None` for markup that is not this envelope. The caller then embeds it unchanged.
pub fn document_html_with_style_nonce(document_html: &str, nonce: &str) -> Option<String> {
    document_html_stylesheet(document_html)?;
    let after_open = document_html.strip_prefix(DOCUMENT_ENVELOPE_OPEN)?;
    let style_at = DOCUMENT_ENVELOPE_OPEN.len() + after_open.find(DOCUMENT_STYLE_OPEN)?;
    // `<style` without the closing `>`: the attribute is written between the two.
    let attribute_at = style_at + DOCUMENT_STYLE_OPEN.len() - 1;
    let mut html = String::with_capacity(document_html.len() + nonce.len() + 16);
    html.push_str(&document_html[..attribute_at]);
    html.push_str(" nonce=\"");
    push_escaped_attribute(&mut html, nonce);
    html.push('"');
    html.push_str(&document_html[attribute_at..]);
    Some(html)
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

/// Count opaque component descendants without recursive traversal.
fn count_opaque_component_children(
    payload: &serde_json::Value,
    start_depth: usize,
    limits: crate::ValidationLimits,
    node_count: &mut usize,
) -> FlyResult<()> {
    if payload.is_null() {
        return Ok(());
    }
    let mut pending = Vec::new();
    match payload {
        serde_json::Value::Array(children) => {
            pending.extend(children.iter().map(|child| (child, start_depth)));
        }
        value => pending.push((value, start_depth)),
    }

    while let Some((value, depth)) = pending.pop() {
        if depth > limits.maximum_depth {
            return Err(FlyError::MaximumDepthExceeded {
                maximum: limits.maximum_depth,
            });
        }
        *node_count += 1;
        if *node_count > limits.maximum_nodes {
            return Err(FlyError::MaximumNodesExceeded {
                maximum: limits.maximum_nodes,
            });
        }
        crate::codec::ensure_depth_within_limit(value, crate::MAXIMUM_DECODE_DEPTH)?;
        if let Some(children) = value.get("components") {
            match children {
                serde_json::Value::Array(children) => {
                    pending.extend(children.iter().map(|child| (child, depth + 1)));
                }
                value => pending.push((value, depth + 1)),
            }
        }
    }
    Ok(())
}

/// Fail closed before recursive render helpers inspect an untrusted/programmatically-built tree.
pub(crate) fn ensure_render_component_limits(document: &ProjectDocument) -> FlyResult<()> {
    let limits = crate::ValidationLimits::default();
    if document.project().pages.len() > crate::DEFAULT_MAXIMUM_PAGES {
        return Err(FlyError::MaximumPagesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_PAGES,
        });
    }
    if document.project().assets.len() > crate::DEFAULT_MAXIMUM_ASSETS {
        return Err(FlyError::MaximumAssetsExceeded {
            maximum: crate::DEFAULT_MAXIMUM_ASSETS,
        });
    }
    if document.project().styles.len() > crate::DEFAULT_MAXIMUM_STYLE_RULES {
        return Err(FlyError::MaximumStyleRulesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_STYLE_RULES,
        });
    }
    let mut pending = Vec::new();

    for value in document
        .project()
        .extensions
        .values()
        .chain(document.project().styles.iter())
        .chain(document.project().assets.iter())
    {
        crate::codec::ensure_depth_within_limit(value, crate::MAXIMUM_DECODE_DEPTH)?;
    }

    for page in &document.project().pages {
        if let Some(frames) = page.frames.as_ref() {
            crate::codec::ensure_depth_within_limit(frames, crate::MAXIMUM_DECODE_DEPTH)?;
        }
        for value in page.extensions.values() {
            crate::codec::ensure_depth_within_limit(value, crate::MAXIMUM_DECODE_DEPTH)?;
        }
        if let Some(root) = page.component.as_ref() {
            pending.push((root, 0usize));
        }
    }

    let mut node_count = 0usize;
    while let Some((node, depth)) = pending.pop() {
        if depth > limits.maximum_depth {
            return Err(FlyError::MaximumDepthExceeded {
                maximum: limits.maximum_depth,
            });
        }
        node_count += 1;
        if node_count > limits.maximum_nodes {
            return Err(FlyError::MaximumNodesExceeded {
                maximum: limits.maximum_nodes,
            });
        }
        match node {
            ComponentNode::Object(component) => {
                for value in component
                    .attributes
                    .values()
                    .chain(component.extensions.values())
                    .chain(component.style.iter())
                    .chain(component.traits.iter())
                {
                    crate::codec::ensure_depth_within_limit(value, crate::MAXIMUM_DECODE_DEPTH)?;
                }
                match &component.components {
                    crate::ComponentChildren::Nodes(children) => {
                        pending.extend(children.iter().map(|child| (child, depth + 1)));
                    }
                    crate::ComponentChildren::Opaque(value) => {
                        count_opaque_component_children(
                            value,
                            depth + 1,
                            limits,
                            &mut node_count,
                        )?;
                    }
                }
            }
            ComponentNode::Opaque(value) => {
                crate::codec::ensure_depth_within_limit(value, crate::MAXIMUM_DECODE_DEPTH)?;
                if let Some(children) = value.get("components") {
                    count_opaque_component_children(
                        children,
                        depth + 1,
                        limits,
                        &mut node_count,
                    )?;
                }
            }
        }
    }
    Ok(())
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
    let embeds = crate::EmbedProviderRegistry::with_builtins();
    render_page_with_rich_text_and_embed_registry(document, selection, policy, rich_text, &embeds)
}

/// Render a page using a host-provided embed-provider catalog.
pub fn render_page_with_embed_registry(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    embeds: &crate::EmbedProviderRegistry,
) -> FlyResult<RenderedPage> {
    render_page_with_rich_text_and_embed_registry(document, selection, policy, None, embeds)
}

/// Render a page with both rich-text and embed-provider host bindings.
pub fn render_page_with_rich_text_and_embed_registry(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
    embeds: &crate::EmbedProviderRegistry,
) -> FlyResult<RenderedPage> {
    ensure_render_component_limits(document)?;
    // Resolve references before rendering, including for non-runtime previews.
    // Published materialization has already resolved them; the fast path keeps
    // its document and hash identity untouched.
    let resolved = if crate::document_has_symbol_instances(document) {
        Some(crate::resolve_symbol_instances(document)?)
    } else {
        None
    };
    let document = resolved.as_ref().unwrap_or(document);
    ensure_render_component_limits(document)?;
    let (page_index, page) = resolve_page(document, selection)?;
    let root = page
        .component
        .as_ref()
        .ok_or_else(|| FlyError::MissingPageRoot(page_index.to_string()))?;
    // Action targets get an unforgeable hook. A plain HTML id is added only when it would not
    // collide with an authored id or the HTML id a form will receive at materialization.
    let identities = RenderIdentities::new(root, action_target_ids(document));
    let decoded_theme_tokens = crate::decode_theme_tokens(document).unwrap_or_default();
    let theme_tokens = decoded_theme_tokens
        .iter()
        .map(|token| token.css_variable())
        .collect::<BTreeSet<_>>();
    // Inline-only rendering has no :root stylesheet, so place validated custom properties
    // on the rendered root element where they inherit into every descendant.
    let inline_theme_style = if policy.emit_style_hooks {
        String::new()
    } else {
        decoded_theme_tokens
            .iter()
            .map(|token| format!("{}:{}", token.css_variable(), token.value.trim()))
            .collect::<Vec<_>>()
            .join(";")
    };
    let mut html = String::new();
    render_node(
        root,
        None,
        0,
        policy,
        &theme_tokens,
        &inline_theme_style,
        true,
        rich_text,
        embeds,
        &identities,
        &mut html,
    );
    let metadata = PageMetadata::from_rendered_page(page);
    let head = PageHead::from_metadata(&metadata);
    let css = if policy.emit_style_hooks {
        render_project_styles(document, page, policy, &theme_tokens)
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
            .project()
            .pages
            .iter()
            .position(|page| page.id.as_deref() == Some(id.as_str()))
            .ok_or_else(|| FlyError::PageNotFound(id.clone()))?,
        PageSelection::Slug(slug) => document
            .project()
            .pages
            .iter()
            .position(|page| {
                PageMetadata::from_rendered_page(page).slug.as_deref() == Some(slug.as_str())
            })
            .ok_or_else(|| FlyError::PageNotFound(slug.clone()))?,
    };
    document
        .project()
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
    for page in &document.project().pages {
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
            | crate::ComponentAction::Animate { target_id, .. }
            | crate::ComponentAction::ToggleAccordion { target_id, .. } => {
                targets.insert(target_id);
            }
            crate::ComponentAction::SwitchTab { panel_id } => {
                targets.insert(panel_id);
            }
            crate::ComponentAction::SwitchSlide {
                slider_id,
                target_slide,
                ..
            } => {
                if slider_id != "self" {
                    targets.insert(slider_id);
                }
                if let Some(target) = target_slide {
                    targets.insert(target);
                }
            }
            _ => {}
        }
    }
    for child in component.children() {
        collect_action_targets(child, targets);
    }
}

struct RenderIdentities {
    action_targets: BTreeSet<String>,
    dom_owners: BTreeMap<String, usize>,
}

impl RenderIdentities {
    fn new(root: &ComponentNode, action_targets: BTreeSet<String>) -> Self {
        let mut owners = Vec::new();
        root.collect_form_dom_owners(&mut owners);
        let mut dom_owners = BTreeMap::new();
        for (form, dom) in owners {
            // A form may explicitly carry its own form id; that is still one owner.
            for id in form.into_iter().chain(dom).collect::<BTreeSet<_>>() {
                *dom_owners.entry(id).or_insert(0) += 1;
            }
        }
        Self {
            action_targets,
            dom_owners,
        }
    }

    fn id_available_to(&self, component: &ComponentObject, id: &str) -> bool {
        let owned = component
            .attributes
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("id") && value.as_str() == Some(id))
            || component
                .extensions
                .get(crate::FLY_FORM_FIELD)
                .and_then(|form| form.get("id"))
                .and_then(Value::as_str)
                == Some(id);
        self.dom_owners.get(id).copied().unwrap_or(0) <= usize::from(owned)
    }
}

fn render_node(
    node: &ComponentNode,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    theme_tokens: &BTreeSet<String>,
    inline_theme_style: &str,
    is_root: bool,
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
    embeds: &crate::EmbedProviderRegistry,
    identities: &RenderIdentities,
    output: &mut String,
) {
    match node {
        ComponentNode::Object(component) => render_component(
            component,
            parent_id,
            index,
            policy,
            theme_tokens,
            inline_theme_style,
            is_root,
            rich_text,
            embeds,
            identities,
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
    theme_tokens: &BTreeSet<String>,
    inline_theme_style: &str,
    is_root: bool,
    rich_text: Option<&dyn crate::RichTextHtmlRenderer>,
    embeds: &crate::EmbedProviderRegistry,
    identities: &RenderIdentities,
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
            theme_tokens,
            inline_theme_style,
            is_root,
            embeds,
            identities,
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
    // Keep the component identity addressable even when the author has supplied a different
    // HTML id and both editor instrumentation and style hooks are disabled. The real HTML id
    // still belongs to the author; the runtime-owned hook cannot be forged by an attribute.
    if let Some(component_id) = component_id
        && identities.action_targets.contains(component_id)
    {
        write_attribute(output, "data-fly-target-id", component_id);
        if !component
            .attributes
            .keys()
            .any(|name| name.eq_ignore_ascii_case("id"))
            && identities.id_available_to(component, component_id)
        {
            write_attribute(output, "id", component_id);
        }
    }

    for (name, value) in &component.attributes {
        let name = name.to_ascii_lowercase();
        if !safe_attribute_name(&name)
            || renderer_manages_attribute(&name)
            || matches!(
                name.as_str(),
                "style" | "srcdoc" | "xlink:href" | "ping" | "background"
            )
        {
            continue;
        }
        // URL attributes must be strings, including srcset. Check before
        // the generic scalar and boolean emission path.
        if name == "srcset" {
            if let Some(value) = value.as_str().filter(|value| srcset_allowed(value, policy)) {
                write_attribute(output, &name, value);
            }
            continue;
        }
        if let Some(kind) = UrlAttributeKind::for_attribute(&name) {
            if let Some(value) = value.as_str().filter(|value| url_allowed(value, kind, policy)) {
                write_attribute(output, &name, value);
            }
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
        write_attribute(output, &name, &value);
    }

    if let Some(declarations) = inline_style_declarations(
        component,
        policy,
        theme_tokens,
        is_root.then_some(inline_theme_style),
        None,
    ) {
        write_attribute(output, "style", &declarations);
    }

    output.push('>');
    if void_tag {
        return;
    }

    if let Some(raw_payload) = component.extensions.get(crate::FLY_RICH_TEXT_FIELD) {
        // A rich-text component is a leaf: the payload replaces both the plain `content`
        // string and any children, which are preserved in the document but not rendered.
        match crate::decode_rich_text_payload(raw_payload) {
            Ok(mut payload) => {
                let url_policy = policy.url_policy();
                crate::rich_text::filter_rich_text_links_for_policy(&mut payload, &url_policy);
                let rendered = rich_text
                    .and_then(|renderer| renderer.render_rich_text(&payload))
                    .unwrap_or_else(|| {
                        crate::rich_text::rich_text_fallback_html_with_policy(
                            &payload,
                            &url_policy,
                        )
                    });
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
                theme_tokens,
                inline_theme_style,
                false,
                rich_text,
                embeds,
                identities,
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
fn inline_style_declarations(
    component: &ComponentObject,
    policy: &RenderPolicy,
    theme_tokens: &BTreeSet<String>,
    root_theme_style: Option<&str>,
    additional_declaration: Option<&str>,
) -> Option<String> {
    let mut declarations = Vec::new();
    if !policy.emit_style_hooks {
        if let Some(root_style) = root_theme_style.filter(|style| !style.is_empty()) {
            declarations.push(root_style.to_string());
        }
        if let Some(style) = component.style.as_ref().and_then(Value::as_object) {
            declarations.extend(style.iter().filter_map(|(name, value)| {
                safe_style_with_tokens(name, value, policy, theme_tokens)
            }));
        }
    }
    if let Some(additional) = additional_declaration.filter(|value| !value.is_empty()) {
        declarations.push(additional.to_string());
    }
    (!declarations.is_empty()).then(|| declarations.join(";"))
}

fn render_embed(
    component: &ComponentObject,
    raw_spec: &Value,
    parent_id: Option<&str>,
    index: usize,
    policy: &RenderPolicy,
    theme_tokens: &BTreeSet<String>,
    inline_theme_style: &str,
    is_root: bool,
    embeds: &crate::EmbedProviderRegistry,
    identities: &RenderIdentities,
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
        && identities.action_targets.contains(component_id)
    {
        write_attribute(output, "data-fly-target-id", component_id);
        if identities.id_available_to(component, component_id) {
            write_attribute(output, "id", component_id);
        }
    }
    write_attribute(output, "class", "fly-embed");
    match crate::decode_embed_spec_with_registry(raw_spec, embeds) {
        Ok(spec) if url_allowed(&spec.url, UrlAttributeKind::Resource, policy) => {
            let aspect_ratio = spec
                .aspect_ratio
                .as_deref()
                .map(|ratio| format!("aspect-ratio:{}", ratio.replace('/', " / ")));
            if let Some(style) = inline_style_declarations(
                component,
                policy,
                theme_tokens,
                is_root.then_some(inline_theme_style),
                aspect_ratio.as_deref(),
            ) {
                write_attribute(output, "style", &style);
            }
            output.push_str("><iframe");
            write_attribute(output, "src", &spec.url);
            write_attribute(output, "title", &spec.title);
            write_attribute(output, "loading", "lazy");
            write_attribute(output, "sandbox", crate::EMBED_IFRAME_SANDBOX);
            write_attribute(output, "referrerpolicy", "strict-origin-when-cross-origin");
            output.push_str(" allowfullscreen></iframe></div>");
        }
        _ => {
            if let Some(style) = inline_style_declarations(
                component,
                policy,
                theme_tokens,
                is_root.then_some(inline_theme_style),
                None,
            ) {
                write_attribute(output, "style", &style);
            }
            output.push('>');
            let raw_title = raw_spec
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Embedded content");
            let raw_url = raw_spec
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if url_allowed(raw_url, UrlAttributeKind::Navigation, policy) && raw_url
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
    theme_tokens: &BTreeSet<String>,
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
            .filter_map(|(name, value)| {
                safe_style_with_tokens(name, value, policy, theme_tokens)
            })
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
        append_component_style_rules(root, policy, theme_tokens, &mut css);
    }
    css
}

fn append_component_style_rules(
    node: &ComponentNode,
    policy: &RenderPolicy,
    theme_tokens: &BTreeSet<String>,
    css: &mut String,
) {
    let ComponentNode::Object(component) = node else {
        return;
    };
    if let (Some(component_id), Some(style)) = (
        component.id.as_deref(),
        component.style.as_ref().and_then(Value::as_object),
    ) {
        let declarations = style
            .iter()
            .filter_map(|(name, value)| {
                safe_style_with_tokens(name, value, policy, theme_tokens)
            })
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
        append_component_style_rules(child, policy, theme_tokens, css);
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
            "wrapper" | "container" | "row" | "column" | "grid" | "spacer" | "symbol"
            | "product_price" | "tabs" | "accordion" | "slider" | "page_content_slot" => "div",
            "section" | "product_grid" | "pricing_table" => "section",
            "heading" => "h2",
            "text" => "p",
            "list" => "ul",
            "list_item" => "li",
            "link" | "cart_button" => "a",
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
            "product_card" => "article",
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

fn renderer_manages_attribute(name: &str) -> bool {
    // These hooks are runtime identity, never author-controlled attributes. Reserve them even
    // in policies that do not emit them, otherwise an authored hook can impersonate a target.
    matches!(
        name,
        "data-fly-target-id"
            | "data-fly-style-id"
            | "data-fly-component-id"
            | "data-fly-index"
            | "data-fly-parent-id"
    )
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
    safe_url::head_url(value, allow_data_image).map(ToString::to_string)
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

/// Decides whether a `srcset` attribute value may be emitted (audit §3.1).
///
/// Mirrors the static publish policy's candidate structure — a non-empty URL
/// followed by at most one descriptor, `data:` URLs refused outright — and
/// checks every candidate URL with the same resource policy that governs
/// `src`. The value is all-or-nothing: one bad candidate drops the whole
/// attribute instead of silently changing which image the browser picks.
fn srcset_allowed(value: &str, policy: &RenderPolicy) -> bool {
    safe_url::srcset_allowed(value, &policy.url_policy())
}

/// Properties that can execute script regardless of their value.
const DENIED_STYLE_PROPERTIES: &[&str] =
    &["behavior", "-moz-binding", "-ms-behavior", "expression"];

/// Check the renderer's CSS property-name admission rule. Hosts may impose
/// additional length or publication restrictions, but must not admit names
/// the renderer would silently drop.
pub fn css_property_allowed(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with("--")
        && name.chars().all(|character| character.is_ascii_alphabetic() || character == '-')
        && !DENIED_STYLE_PROPERTIES.contains(&name.to_ascii_lowercase().as_str())
}

/// Return byte ranges of quoted resource strings inside CSS image functions.
///
/// image-set(), image(), and src() can carry resource URLs as strings. The scanner also
/// finds these functions when nested inside other functions such as cross-fade(). Callers
/// validate each returned string with their own resource URL policy. Strings outside these
/// functions are ignored. Escaped strings and malformed function/string boundaries fail closed.
/// Return all custom-property references in CSS var() functions.
///
/// Strings are ignored, nested fallbacks are scanned, and malformed or escaped var()
/// expressions fail closed. The caller decides which custom properties are in scope.
fn css_var_reference_ranges(value: &str) -> Option<Vec<std::ops::Range<usize>>> {
    if value.contains('\\') {
        return None;
    }
    let lowered = value.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while cursor < value.len() {
        match value.as_bytes()[cursor] {
            b'\'' | b'"' => {
                cursor = skip_css_string(value, cursor)?;
                continue;
            }
            _ => {}
        }
        if value.as_bytes().get(cursor..cursor + 2) == Some(&b"/*"[..]) {
            cursor = skip_css_comment(value, cursor)?;
            continue;
        }

        if lowered[cursor..].starts_with("var(") {
            let preceded_by_identifier = value[..cursor]
                .chars()
                .next_back()
                .is_some_and(css_identifier_character);
            if !preceded_by_identifier {
                let open = cursor + 3;
                let close = css_function_close(value, open)?;
                let arguments = &value[open + 1..close];
                let first_argument_end = arguments.find(',').unwrap_or(arguments.len());
                let first_argument = &arguments[..first_argument_end];
                let name = first_argument.trim();
                if !valid_css_custom_property_name(name) {
                    return None;
                }
                let leading_whitespace = first_argument.len() - first_argument.trim_start().len();
                let name_start = open + 1 + leading_whitespace;
                ranges.push(name_start..name_start + name.len());
                // Continue through fallbacks to find nested var() references.
                cursor = open + 1;
                continue;
            }
        }

        cursor += value[cursor..].chars().next()?.len_utf8();
    }
    Some(ranges)
}

/// Return all custom-property references in CSS var() functions.
///
/// Strings and comments are ignored, nested fallbacks are scanned, and malformed or escaped
/// var() expressions fail closed. The caller decides which custom properties are in scope.
pub fn css_var_references(value: &str) -> Option<Vec<String>> {
    let ranges = css_var_reference_ranges(value)?;
    Some(
        ranges
            .into_iter()
            .map(|range| value[range].to_string())
            .collect(),
    )
}

/// Rewrite only the first custom-property argument of actual CSS var() functions.
pub(crate) fn rewrite_css_var_references(
    value: &str,
    old_name: &str,
    new_name: &str,
) -> (String, usize) {
    let Some(ranges) = css_var_reference_ranges(value) else {
        return (value.to_string(), 0);
    };
    let target = format!("--{old_name}");
    let replacement = format!("--{new_name}");
    let matches = ranges
        .into_iter()
        .filter(|range| &value[range.start..range.end] == target.as_str())
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return (value.to_string(), 0);
    }
    let mut rewritten = value.to_string();
    for range in matches.iter().rev() {
        rewritten.replace_range(range.start..range.end, &replacement);
    }
    (rewritten, matches.len())
}

/// Whether a CSS declaration can load or reference an external resource.
///
/// Custom-property substitution is deliberately disabled for these properties: the
/// variable can be overridden by the embedding document's cascade after Fly validates it.
pub fn css_property_may_load_resource(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "background"
            | "background-image"
            | "border-image"
            | "border-image-source"
            | "clip-path"
            | "content"
            | "cursor"
            | "filter"
            | "fill"
            | "list-style"
            | "list-style-image"
            | "mask"
            | "mask-image"
            | "mask-border"
            | "mask-border-source"
            | "marker"
            | "marker-end"
            | "marker-mid"
            | "marker-start"
            | "offset-path"
            | "shape-outside"
            | "src"
            | "stroke"
            | "-webkit-box-reflect"
            | "-webkit-background-image"
            | "-webkit-clip-path"
            | "-webkit-filter"
            | "-webkit-border-image"
            | "-webkit-border-image-source"
            | "-webkit-mask"
            | "-webkit-mask-image"
            | "-webkit-mask-box-image"
            | "-webkit-mask-box-image-source"
    )
}

fn valid_css_custom_property_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("--") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || !character.is_ascii()
        })
}

/// Return the original byte ranges of quoted resource strings in CSS image functions.
pub fn css_resource_string_ranges(value: &str) -> Option<Vec<std::ops::Range<usize>>> {
    Some(
        scan_css_resource_references(value)?
            .into_iter()
            .filter(|reference| reference.quoted_image)
            .map(|reference| reference.url)
            .collect(),
    )
}

struct CssResourceReference {
    url: std::ops::Range<usize>,
    mask: std::ops::Range<usize>,
    quoted_image: bool,
}

/// Scan the original text once, recording disjoint ranges in source order.
/// Each stack entry describes whether direct strings inside that group are
/// image resources. URL tokens are consumed atomically, never rescanned as CSS.
fn scan_css_resource_references(value: &str) -> Option<Vec<CssResourceReference>> {
    if value.contains('\\') || value.chars().any(char::is_control) {
        return None;
    }
    let mut references = Vec::new();
    let mut groups = Vec::new();
    let mut cursor = 0;
    while cursor < value.len() {
        if value.as_bytes().get(cursor..cursor + 2) == Some(&b"/*"[..]) {
            cursor = skip_css_comment(value, cursor)?;
            continue;
        }
        match value.as_bytes()[cursor] {
            b'\'' | b'"' => {
                let after = skip_css_string(value, cursor)?;
                if groups.last() == Some(&true) {
                    let range = cursor + 1..after - 1;
                    references.push(CssResourceReference {
                        url: range.clone(),
                        mask: range,
                        quoted_image: true,
                    });
                }
                cursor = after;
            }
            b'(' => {
                let start = value[..cursor]
                    .char_indices()
                    .rev()
                    .find(|(_, character)| !css_identifier_character(*character))
                    .map_or(0, |(index, character)| index + character.len_utf8());
                let name = &value[start..cursor];
                if name.eq_ignore_ascii_case("url") {
                    let close = css_function_close(value, cursor)?;
                    let argument = &value[cursor + 1..close];
                    let raw = argument.trim();
                    let raw_start = cursor + 1 + argument.len() - argument.trim_start().len();
                    let url = if matches!(raw.as_bytes().first(), Some(b'\'' | b'"')) {
                        let after = skip_css_string(raw, 0)?;
                        if after != raw.len() {
                            return None;
                        }
                        raw_start + 1..raw_start + after - 1
                    } else {
                        if raw.contains(['\'', '"', '(', ')']) {
                            return None;
                        }
                        raw_start..raw_start + raw.len()
                    };
                    references.push(CssResourceReference {
                        url,
                        mask: start..close + 1,
                        quoted_image: false,
                    });
                    cursor = close + 1;
                    continue;
                }
                groups.push(
                    ["image-set", "-webkit-image-set", "image", "src"]
                        .iter()
                        .any(|function| name.eq_ignore_ascii_case(function)),
                );
                cursor += 1;
            }
            b')' => {
                groups.pop()?;
                cursor += 1;
            }
            _ => cursor += value[cursor..].chars().next()?.len_utf8(),
        }
    }
    groups.is_empty().then_some(references)
}

fn css_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_') || !character.is_ascii()
}

fn skip_css_string(value: &str, start: usize) -> Option<usize> {
    let quote = *value.as_bytes().get(start)?;
    if !matches!(quote, b'\'' | b'"') { return None; }
    let mut cursor = start + 1;
    while cursor < value.len() {
        match value.as_bytes()[cursor] {
            b'\\' => return None,
            character if character == quote => return Some(cursor + 1),
            _ => cursor += 1,
        }
    }
    None
}

fn skip_css_comment(value: &str, start: usize) -> Option<usize> {
    if value.as_bytes().get(start..start + 2) != Some(&b"/*"[..]) {
        return None;
    }
    let end = value.get(start + 2..)?.find("*/")?;
    Some(start + 2 + end + 2)
}

fn css_function_close(value: &str, open: usize) -> Option<usize> {
    if value.as_bytes().get(open) != Some(&b'(') { return None; }
    let mut depth = 1usize;
    let mut cursor = open + 1;
    while cursor < value.len() {
        if value.as_bytes().get(cursor..cursor + 2) == Some(&b"/*"[..]) {
            cursor = skip_css_comment(value, cursor)?;
            continue;
        }
        match value.as_bytes()[cursor] {
            b'\'' | b'"' => cursor = skip_css_string(value, cursor)?,
            b'\\' => return None,
            b'(' => { depth = depth.checked_add(1)?; cursor += 1; }
            b')' => {
                depth -= 1;
                if depth == 0 { return Some(cursor); }
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    None
}

/// Decide whether a single CSS declaration may be emitted.
///
/// CSS resource URLs may be expressed through url() or through quoted strings in image(),
/// src(), image-set(), and -webkit-image-set(). Every form is checked against the same
/// RenderPolicy resource rules, including when these functions are nested inside image functions.
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
/// the shared `strip_css_resource_references` used by the static publish policy.
fn safe_style(name: &str, value: &Value, policy: &RenderPolicy) -> Option<String> {
    safe_style_with_tokens(name, value, policy, &BTreeSet::new())
}

fn safe_style_with_tokens(
    name: &str,
    value: &Value,
    policy: &RenderPolicy,
    theme_tokens: &BTreeSet<String>,
) -> Option<String> {
    if !css_property_allowed(name) {
        return None;
    }
    let value = scalar_string(value)?;
    let references = css_var_references(&value)?;
    if (css_property_may_load_resource(name) && !references.is_empty())
        || references
            .iter()
            .any(|reference| !theme_tokens.contains(reference))
    {
        return None;
    }
    if value.chars().any(char::is_control) {
        return None;
    }

    // Check every original resource before masking it for structural checks.
    let url_policy = policy.url_policy();
    let without_urls = strip_css_resource_references(&value, |url| {
        safe_url::url_allowed(url, UrlAttributeKind::Resource, &url_policy)
    })?;

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

/// Validate CSS resource references using the caller's URL policy, then mask only
/// the checked tokens for subsequent structural checks. Quoted image sources are
/// checked in the original input without rescanning any masked text. A string
/// containing the literal characters "url(" is not itself a CSS url() function.
/// This deliberately conservative grammar refuses escapes and malformed tokens.
pub fn strip_css_resource_references(
    value: &str,
    mut allow_url: impl FnMut(&str) -> bool,
) -> Option<String> {
    let references = scan_css_resource_references(value)?;
    let mut output = String::with_capacity(value.len());
    let mut copied = 0;
    for reference in references {
        if reference.mask.start < copied
            || !css_resource_url_allowed(&value[reference.url], &mut allow_url)
        {
            return None;
        }
        output.push_str(&value[copied..reference.mask.start]);
        output.push_str("url-ok");
        copied = reference.mask.end;
    }
    output.push_str(&value[copied..]);
    Some(output)
}

fn css_resource_url_allowed(url: &str, allow_url: &mut impl FnMut(&str) -> bool) -> bool {
    !url.is_empty()
        && !url.contains(['{', '}', '<', '>'])
        && !url.contains("/*")
        && !url.contains("*/")
        && allow_url(url)
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
///
/// Public because the renderer is not the only writer of such a selector: a framework adapter
/// that styles one component by its id (the selection outline of the authoring canvas) writes
/// the id into a `<style>` element too, and uses this function at that sink.
pub fn escape_css_attribute(value: &str) -> String {
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
    fn action_target_does_not_duplicate_another_elements_html_or_form_id() {
        for other in [
            json!({"id": "other", "type": "section", "attributes": {"id": "panel"}}),
            json!({"id": "form", "type": "form", "flyForm": {"id": "panel"}}),
        ] {
            let document = GrapesJsCodec::decode_value(json!({
                "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "trigger", "type": "button", "flyAction": {"kind": "toggle_visibility", "target_id": "panel"}},
                    {"id": "panel", "type": "section"}, other
                ]}}]
            })).unwrap();
            // Form IDs materialize to HTML ids before public rendering.
            let materialized = crate::materialize_component_actions(&document, &json!({}));
            let page = render_page(
                &materialized.document,
                &PageSelection::First,
                &RenderPolicy::default(),
            )
            .unwrap();
            assert_eq!(
                page.html.matches(" id=\"panel\"").count(),
                1,
                "{}",
                page.html
            );
            assert!(
                page.html.contains("data-fly-target-id=\"panel\""),
                "{}",
                page.html
            );
        }
    }

    #[test]
    fn action_target_hook_survives_an_authored_id_without_style_hooks_or_instrumentation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {
                "id": "root", "type": "wrapper", "components": [
                    {"id": "trigger", "type": "button",
                     "flyAction": {"kind": "toggle_visibility", "target_id": "panel"}},
                    {"id": "panel", "type": "section",
                     "attributes": {"id": "authored-id", "data-fly-target-id": "forged"}},
                    {"id": "plain", "type": "section",
                     "attributes": {"data-fly-target-id": "panel", "data-fly-style-id": "spoofed", "data-fly-component-id": "spoofed"}}
                ]
            }}]
        }))
        .expect("document");
        let page = render_page(
            &document,
            &PageSelection::First,
            &RenderPolicy {
                instrument_components: false,
                emit_style_hooks: false,
                ..RenderPolicy::default()
            },
        )
        .expect("rendered");
        assert!(page.html.contains("id=\"authored-id\""), "{}", page.html);
        assert!(
            page.html.contains("data-fly-target-id=\"panel\""),
            "{}",
            page.html
        );
        assert_eq!(page.html.matches("data-fly-target-id").count(), 1);
        assert!(!page.html.contains("forged"));
        assert!(!page.html.contains("data-fly-style-id"));
        assert!(!page.html.contains("data-fly-component-id"));
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
    fn renderer_drops_undeclared_css_variable_references_and_nested_fallbacks() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyTheme": { "tokens": { "brand": "#0b5fff" } },
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "style": {
                        "color": "var(--brand)",
                        "background-image": "var(--host-url)",
                        "mask-image": "var(--brand, var(--external-url))",
                        "border-image-source": "var(--brand)",
                        "-webkit-box-reflect": "below 0 var(--brand)"
                    },
                    "components": []
                }
            }]
        })).expect("document");
        let page = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("rendered");

        assert!(page.css.contains("color:var(--brand)"), "css: {}", page.css);
        assert!(!page.css.contains("var(--host-url)"), "css: {}", page.css);
        assert!(!page.css.contains("var(--external-url)"), "css: {}", page.css);
        assert!(!page.css.contains("border-image-source:var(--brand)"), "css: {}", page.css);
        assert!(!page.css.contains("-webkit-box-reflect:"), "css: {}", page.css);
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
    fn css_string_resource_sources_follow_the_render_resource_policy() {
        let policy = RenderPolicy::default();
        for value in [
            "image-set(\"/hero.png\" 1x, \"https://cdn.example/hero@2x.png\" 2x)",
            "-webkit-image-set('/hero.png' 1x)",
            "image-set(\"data:image/png;base64,iVBORw0KGgo=\" 1x)",
            "image(\"https://cdn.example/hero.png\", red)",
            "src(\"https://cdn.example/hero.png\")",
            "cross-fade(image('/fade-start.png'), image('/fade-end.png'))",
            "image(\"data:image/png;base64,iVBORw0KGgo=\", red)",
        ] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy).is_some(),
                "rejected safe image-set {value}"
            );
        }
        for value in [
            "image-set('javascript:alert(1)' 1x)",
            "image-set('//attacker.example/hero.png' 1x)",
            "image-set('data:image/svg+xml;base64,PHN2Zz4=' 1x)",
            "image('javascript:alert(1)', red)",
            "src('//attacker.example/hero.png')",
        ] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy).is_none(),
                "accepted unsafe image-set {value}"
            );
        }
        let no_https = RenderPolicy {
            allow_https: false,
            ..RenderPolicy::default()
        };
        for value in [
            "image-set('https://cdn.example/hero.png' 1x)",
            "image('https://cdn.example/hero.png', red)",
            "src('https://cdn.example/hero.png')",
        ] {
            assert!(safe_style(
                "background-image",
                &Value::String(value.to_string()),
                &no_https
            ).is_none(), "accepted HTTPS with allow_https=false: {value}");
        }
        let no_data = RenderPolicy {
            allow_data_images: false,
            ..RenderPolicy::default()
        };
        assert!(safe_style(
            "background-image",
            &Value::String("image('data:image/png;base64,iVBORw0KGgo=', red)".to_string()),
            &no_data
        ).is_none());
    }

    #[test]
    fn css_resource_scanner_validates_original_urls_and_token_boundaries() {
        let policy = RenderPolicy::default();
        let oversized = format!("image-set('/{}' 1x)", "url(/long-segment)".repeat(130));
        assert!(safe_style("background-image", &Value::String(oversized), &policy).is_none());
        for value in ["url(/a(b))", "url(/a'broken')", "url('/a.png' extra)"] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy).is_none(),
                "{value}"
            );
        }
        for value in ["url('/hero(1).png')", "image-set('/url(/hero.png)' 1x)"] {
            assert!(
                safe_style("background-image", &Value::String(value.to_string()), &policy).is_some(),
                "{value}"
            );
        }
        let mut seen = Vec::new();
        let scanned = strip_css_resource_references(
            "image-set('/url(/hero.png)' 1x, url('/hero(2).png') 2x)",
            |url| { seen.push(url.to_string()); true },
        ).expect("valid CSS");
        assert_eq!(seen, ["/url(/hero.png)", "/hero(2).png"]);
        assert_eq!(scanned, "image-set('url-ok' 1x, url-ok 2x)");
    }

    #[test]
    fn nested_css_resource_ranges_remain_in_source_order() {
        let css = "image-set(image('/é.png') 1x, '/second-long.png' 2x)";
        let ranges = css_resource_string_ranges(css).expect("resource ranges");
        assert_eq!(
            ranges.iter().map(|range| &css[range.clone()]).collect::<Vec<_>>(),
            ["/é.png", "/second-long.png"]
        );
        let mut checked = Vec::new();
        let masked = strip_css_resource_references(css, |url| {
            checked.push(url.to_string());
            true
        }).expect("nested resources");
        assert_eq!(checked, ["/é.png", "/second-long.png"]);
        assert_eq!(masked, "image-set(image('url-ok') 1x, 'url-ok' 2x)");
        assert!(safe_style(
            "background-image",
            &Value::String(css.to_string()),
            &RenderPolicy::default(),
        ).is_some());
    }

    #[test]
    fn resource_scanner_uses_original_tokens_and_ignores_comments() {
        let value = "image-set(/* image('/ignored.png') */ image('/first.png') 1x, url('/second.png') 2x, '/third.png' 3x)";
        let mut checked = Vec::new();
        let masked = strip_css_resource_references(value, |url| {
            checked.push(url.to_string());
            true
        }).expect("resource tokens");
        assert_eq!(checked, ["/first.png", "/second.png", "/third.png"]);
        assert_eq!(
            masked,
            "image-set(/* image('/ignored.png') */ image('url-ok') 1x, url-ok 2x, 'url-ok' 3x)"
        );
        assert!(safe_style(
            "background-image",
            &Value::String(value.to_string()),
            &RenderPolicy::default(),
        ).is_none());
        for malformed in ["image-set('/a.png' 1x))", "cross-fade(image('/a.png')", "url(/a(b))"] {
            assert!(strip_css_resource_references(malformed, |_| true).is_none(), "{malformed}");
        }
    }

    #[test]
    fn css_resource_string_scanner_ignores_literals_and_finds_nested_functions() {
        let css = r#"content: "image-set('not-a-resource')"; background-image: IMAGE-SET("/hero.png" 1x, url("/hero@2x.png") 2x, type("image/avif")); -webkit-image-set('/webkit.png' 1x); background: image("https://cdn.example/hero.png", red); background: cross-fade(image("/fade.png"), src("/source.png")); font: src("/font.woff2" format("woff2"))"#;
        let ranges = css_resource_string_ranges(css).expect("valid CSS string boundaries");
        let values = ranges.iter().map(|range| &css[range.clone()]).collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                "/hero.png",
                "/webkit.png",
                "https://cdn.example/hero.png",
                "/fade.png",
                "/source.png",
                "/font.woff2"
            ]
        );
        assert!(css_resource_string_ranges(r#"content: "image-set('/literal.png')""#)
            .expect("literal CSS string").is_empty());
        assert!(css_resource_string_ranges(r#"image-set("/unterminated.png" 1x"#).is_none());
        assert!(css_resource_string_ranges(r#"image("/escaped\".png", red)"#).is_none());
    }

    #[test]
    fn css_var_reference_scanner_finds_nested_fallbacks_and_ignores_strings() {
        let css = r#"content: "var(--not-a-token)"; color: VAR(--brand); width: calc(var(--space) * 2); background: var(--image, var(--fallback))"#;
        assert_eq!(
            css_var_references(css).expect("valid var() expressions"),
            vec![
                "--brand".to_string(),
                "--space".to_string(),
                "--image".to_string(),
                "--fallback".to_string()
            ]
        );
    }

    #[test]
    fn css_var_reference_scanner_rejects_malformed_or_escaped_references() {
        for value in [
            "var()",
            "var(--bad name)",
            "var(--unclosed",
            r#"var(--token, "escaped\value")"#,
        ] {
            assert!(css_var_references(value).is_none(), "accepted {value}");
        }
    }

    #[test]
    fn css_var_reference_scanner_ignores_strings_and_comments_and_scans_nested_fallbacks() {
        let value = r#"content: "var(--literal)"; color: var(--brand, VAR( --fallback )); /* var(--comment) */"#;
        assert_eq!(
            css_var_references(value),
            Some(vec!["--brand".to_string(), "--fallback".to_string()])
        );
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
    fn inline_style_mode_preserves_theme_tokens_and_embed_component_styles() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyTheme": { "tokens": { "brand": "#0b5fff" } },
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "style": { "color": "var(--brand)" },
                    "components": [{
                        "id": "embed",
                        "type": "wrapper",
                        "style": { "border-color": "var(--brand)" },
                        "flyEmbed": {
                            "provider": "youtube",
                            "url": "https://www.youtube.com/embed/demo",
                            "title": "Video"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(
            &document,
            &PageSelection::First,
            &RenderPolicy {
                emit_style_hooks: false,
                ..RenderPolicy::default()
            },
        )
        .expect("render page");

        assert!(rendered.css.is_empty());
        assert!(
            rendered.html.contains(r#"style="--brand:#0b5fff;color:var(--brand)""#),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains(r#"style="border-color:var(--brand)""#),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains("<iframe"), "{}", rendered.html);
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
    fn composed_document_keeps_its_envelope() {
        let html = compose_document_html(&PageHead::default(), "a{b:c}", "<p>x</p>");
        assert_eq!(
            html,
            concat!(
                "<!doctype html><html><head><meta charset=\"utf-8\">",
                "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
                "<style>a{b:c}</style></head><body><p>x</p></body></html>",
            )
        );
    }

    // The head is the only part between the envelope opening and its stylesheet. Every place
    // of it that takes authored text is given the literal the lookup searches for.
    #[test]
    fn style_nonce_goes_on_the_envelope_stylesheet_only() {
        let head = PageHead {
            title: Some("<style>".to_string()),
            description: Some("<style>".to_string()),
            canonical_url: Some("https://example.test/<style>".to_string()),
            structured_data: vec![json!({ "name": "<style>" })],
            ..PageHead::default()
        };
        let body = "<style>p{color:red}</style><p>x</p>";
        let html = compose_document_html(&head, "a{b:c}", body);

        let stamped = document_html_with_style_nonce(&html, "0f3a9c").expect("envelope");

        assert_eq!(stamped.matches("nonce=").count(), 1, "{stamped}");
        assert!(
            stamped.contains("<style nonce=\"0f3a9c\">a{b:c}</style></head>"),
            "{stamped}"
        );
        assert!(
            stamped.contains(&format!("<body>{body}</body>")),
            "{stamped}"
        );
        assert_eq!(stamped.replace(" nonce=\"0f3a9c\"", ""), html);
    }

    #[test]
    fn style_nonce_value_is_written_as_an_escaped_attribute() {
        let html = compose_document_html(&PageHead::default(), "", "");
        let stamped = document_html_with_style_nonce(&html, "\"><script>").expect("envelope");
        assert!(
            stamped.contains("<style nonce=\"&quot;&gt;&lt;script&gt;\"></style>"),
            "{stamped}"
        );
        assert!(!stamped.contains("<script>"), "{stamped}");
    }

    #[test]
    fn style_nonce_is_refused_for_markup_that_is_not_the_envelope() {
        for markup in [
            String::new(),
            "<html><head><style>a{b:c}</style></head><body></body></html>".to_string(),
            format!(" {}", compose_document_html(&PageHead::default(), "", "")),
            format!("{DOCUMENT_ENVELOPE_OPEN}</head><body></body></html>"),
        ] {
            assert_eq!(
                document_html_with_style_nonce(&markup, "0f3a9c"),
                None,
                "{markup}"
            );
        }
    }

    #[test]
    fn the_stylesheet_is_read_back_from_the_document_it_travels_in() {
        let css = "[data-fly-style-id=\"hero\"]{margin-top:12px}";
        let html = compose_document_html(&PageHead::default(), css, "<p>x</p>");
        assert_eq!(document_html_stylesheet(&html), Some(css));
        // A policy hashes the text of the element, so the value is the one the browser sees.
        assert_eq!(
            document_html_stylesheet(&compose_document_html(&PageHead::default(), "", "")),
            Some("")
        );
    }

    #[test]
    fn the_stylesheet_is_the_escaped_text_and_not_the_page_model_css() {
        // The envelope rewrites `<` inside CSS, so hashing the page model's CSS would name a hash
        // the browser never computes for the element. Reading the document back is the only way
        // to get the text the browser hashes.
        let css = "a::after{content:'<'}";
        let html = compose_document_html(&PageHead::default(), css, "");
        let read_back = document_html_stylesheet(&html).expect("envelope");
        assert_ne!(read_back, css);
        assert!(read_back.contains("\\00003c "), "{read_back}");
        assert!(
            html.contains(&format!("<style>{read_back}</style>")),
            "{html}"
        );
    }

    #[test]
    fn the_stylesheet_is_absent_for_markup_that_is_not_the_envelope() {
        for markup in [
            String::new(),
            "<html><head><style>a{b:c}</style></head><body></body></html>".to_string(),
            format!("{DOCUMENT_ENVELOPE_OPEN}</head><body></body></html>"),
            format!("{DOCUMENT_ENVELOPE_OPEN}<style>a{{b:c}}</style></head>"),
            format!(
                "{DOCUMENT_ENVELOPE_OPEN}</HEAD ><body><style>a{{b:c}}</style></head><body></body></html>"
            ),
        ] {
            assert_eq!(document_html_stylesheet(&markup), None, "{markup}");
            assert_eq!(
                document_html_with_style_nonce(&markup, "nonce"),
                None,
                "{markup}"
            );
        }
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

    #[test]
    fn renders_srcset_when_every_candidate_url_is_allowed() {
        // `srcset` used to be dropped unconditionally while `sizes` was
        // emitted — validated adaptive images never reached the artifact
        // (audit §3.1).
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "image",
                        "attributes": {
                            "src": "/img/hero.webp",
                            "srcset": "/img/hero.webp 800w, /img/hero@2x.webp 1600w",
                            "sizes": "(max-width: 800px) 100vw, 800px"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("rendered");
        assert!(
            rendered
                .html
                .contains("srcset=\"/img/hero.webp 800w, /img/hero@2x.webp 1600w\""),
            "html: {}",
            rendered.html
        );
        assert!(
            rendered
                .html
                .contains("sizes=\"(max-width: 800px) 100vw, 800px\""),
            "html: {}",
            rendered.html
        );
    }

    #[test]
    fn non_string_urls_are_not_emitted_as_bare_attributes_or_numeric_paths() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"id": "home", "component": {"id": "root", "type": "wrapper", "components": [
                {"id": "image", "type": "image", "attributes": {"src": 42, "srcset": true, "alt": "Image"}},
                {"id": "link", "type": "link", "attributes": {"href": true}, "content": "Link"}
            ]}}]
        })).expect("document");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default()).expect("render");
        assert!(!rendered.html.contains(" src"), "{}", rendered.html);
        assert!(!rendered.html.contains(" href"), "{}", rendered.html);
        assert!(rendered.html.contains("alt=\"Image\""));
        let diagnostics = crate::validate_component_public_urls(&document);
        for name in ["src", "srcset", "href"] {
            assert!(diagnostics.iter().any(|d| d.code == "runtime_public_url_invalid" && d.severity == crate::ValidationSeverity::Error && d.path.ends_with(&format!(".{name}"))));
        }
    }

    #[test]
    fn drops_srcset_when_a_candidate_is_disallowed_or_malformed() {
        for srcset in [
            "javascript:alert(1) 1x, /img/hero.webp 2x",
            "/img/hero.webp 1x, data:image/png;base64,AAAA 2x",
            "/img/hero.webp 1x trailing-token",
            "/a.webp 1x, /b.webp 1.0x",
            "/a.webp 0x",
            "/a.webp 800w, /b.webp 2x",
            "DATA:image/png;base64,AAAA 2x",
            "/a.webp nonsense",
            "",
        ] {
            let document = GrapesJsCodec::decode_value(json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "hero",
                            "type": "image",
                            "attributes": { "src": "/img/hero.webp", "srcset": srcset }
                        }]
                    }
                }]
            }))
            .expect("document");
            let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
                .expect("rendered");
            assert!(
                !rendered.html.contains("srcset="),
                "html: {}",
                rendered.html
            );
        }
    }


    #[test]
    fn renderer_uses_the_host_embed_registry_and_rejects_unknown_provider_by_default() {
        let mut embed_node = json!({ "id": "player", "type": "wrapper" });
        embed_node
            .as_object_mut()
            .expect("component object")
            .insert(
                crate::FLY_EMBED_FIELD.to_string(),
                json!({
                    "provider": "custom",
                    "url": "https://player.example.test/frame/123",
                    "title": "Player"
                }),
            );
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [embed_node]
                }
            }]
        }))
        .expect("document");

        let default_render =
            render_page(&document, &PageSelection::First, &RenderPolicy::default())
                .expect("default render");
        assert!(!default_render.html.contains("<iframe"));

        let mut embeds = crate::EmbedProviderRegistry::with_builtins();
        embeds.register(crate::EmbedProvider {
            id: "custom",
            label: "Custom",
            hosts: &["player.example.test"],
            path_prefixes: &["/frame/"],
        });
        let custom_render = render_page_with_embed_registry(
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &embeds,
        )
        .expect("custom render");
        assert!(custom_render.html.contains("<iframe"));
        assert!(custom_render.html.contains("https://player.example.test/frame/123"));
    }

    #[test]
    fn render_policy_filters_embeds_and_fallback_links() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "registered",
                            "type": "wrapper",
                            "flyEmbed": {
                                "provider": "youtube",
                                "url": "https://www.youtube.com/embed/demo",
                                "title": "Registered video"
                            }
                        },
                        {
                            "id": "unknown",
                            "type": "wrapper",
                            "flyEmbed": {
                                "provider": "unknown",
                                "url": "https://example.test/video",
                                "title": "Unknown video"
                            }
                        },
                        {
                            "id": "rich",
                            "type": "wrapper",
                            "flyRichText": {
                                "capability": "richtext",
                                "payload": {
                                    "type": "doc",
                                    "content": [{
                                        "type": "paragraph",
                                        "content": [
                                            {
                                                "type": "text",
                                                "text": "External link",
                                                "marks": [{
                                                    "type": "link",
                                                    "attrs": {"href": "https://example.test/link"}
                                                }]
                                            },
                                            {
                                                "type": "text",
                                                "text": "Local link",
                                                "marks": [{
                                                    "type": "link",
                                                    "attrs": {"href": "/local"}
                                                }]
                                            }
                                        ]
                                    }]
                                }
                            }
                        }
                    ]
                }
            }]
        })).expect("document");

        let default = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("default render");
        assert!(default.html.contains("<iframe"));
        assert!(default.html.contains("href=\"https://example.test/video\""));
        assert!(default.html.contains("href=\"https://example.test/link\""));
        assert!(default.html.contains("href=\"/local\""));

        let restricted = render_page(
            &document,
            &PageSelection::First,
            &RenderPolicy {
                allow_https: false,
                ..RenderPolicy::default()
            },
        ).expect("restricted render");
        assert!(!restricted.html.contains("<iframe"));
        assert!(!restricted.html.contains("https://"));
        assert!(restricted.html.contains("Registered video"));
        assert!(restricted.html.contains("Unknown video"));
        assert!(restricted.html.contains("External link"));
        assert!(restricted.html.contains("href=\"/local\""));
    }

    #[test]
    fn render_policy_filters_links_before_trusted_rich_text_renderer() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "rich",
                        "type": "wrapper",
                        "flyRichText": {
                            "capability": "richtext",
                            "payload": {
                                "type": "doc",
                                "content": [{
                                    "type": "paragraph",
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": "External link",
                                            "marks": [{
                                                "type": "link",
                                                "attrs": {"href": "https://example.test/link"}
                                            }]
                                        },
                                        {
                                            "type": "text",
                                            "text": "Local link",
                                            "marks": [{
                                                "type": "link",
                                                "attrs": {"href": "/local"}
                                            }]
                                        }
                                    ]
                                }]
                            }
                        }
                    }]
                }
            }]
        })).expect("document");
        let trusted_renderer = |payload: &crate::RichTextPayload| {
            Some(crate::rich_text_fallback_html(payload))
        };
        let policy = RenderPolicy {
            allow_https: false,
            ..RenderPolicy::default()
        };
        let rendered = render_page_with_rich_text(
            &document,
            &PageSelection::First,
            &policy,
            Some(&trusted_renderer),
        ).expect("render");

        assert!(!rendered.html.contains("https://"));
        assert!(rendered.html.contains("External link"));
        assert!(rendered.html.contains("Local link"));
        assert!(!rendered.html.contains("href=\"https://"));
        assert!(rendered.html.contains("href=\"/local\""));
    }

    #[test]
    fn renderer_rejects_programmatically_built_trees_beyond_depth_limit() {
        let limits = crate::ValidationLimits::default();
        let mut component = json!({ "id": "leaf", "type": "text" });
        for index in 0..=limits.maximum_depth {
            component = json!({
                "id": format!("wrapper-{index}"),
                "type": "wrapper",
                "components": [component]
            });
        }
        let project: crate::GrapesProject = serde_json::from_value(json!({
            "pages": [{ "id": "home", "component": component }]
        }))
        .expect("typed project");
        let document = crate::ProjectDocument::new(project);
        assert!(matches!(
            render_page(&document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumDepthExceeded { maximum }) if maximum == limits.maximum_depth
        ));
    }

    #[test]
    fn renderer_rejects_programmatically_built_trees_beyond_node_limit() {
        let limits = crate::ValidationLimits::default();
        let children = (0..limits.maximum_nodes)
            .map(|index| json!({ "id": format!("child-{index}"), "type": "text" }))
            .collect::<Vec<_>>();
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": children
                }
            }]
        }))
        .expect("document");
        assert!(matches!(
            render_page(&document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumNodesExceeded { maximum }) if maximum == limits.maximum_nodes
        ));
    }




    #[test]
    fn renderer_rejects_deep_raw_project_extensions_before_symbol_resolution() {
        let mut nested = serde_json::Value::Null;
        for _ in 0..=crate::MAXIMUM_DECODE_DEPTH {
            nested = serde_json::Value::Array(vec![nested]);
        }
        let mut project: crate::GrapesProject = serde_json::from_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("project");
        project
            .extensions
            .insert(crate::FLY_SYMBOLS_FIELD.to_string(), nested);
        let document = crate::ProjectDocument::new(project);
        assert!(matches!(
            render_page(&document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumDepthExceeded { maximum })
                if maximum == crate::MAXIMUM_DECODE_DEPTH
        ));
    }




    #[test]
    fn renderer_counts_opaque_component_children_toward_node_limit() {
        let limits = crate::ValidationLimits::default();
        let children = (0..limits.maximum_nodes)
            .map(|index| json!({ "id": format!("opaque-{index}"), "type": 42 }))
            .collect::<Vec<_>>();
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": { "components": children }
                }
            }]
        }))
        .expect("document");
        assert!(matches!(
            render_page(&document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumNodesExceeded { maximum }) if maximum == limits.maximum_nodes
        ));
    }

    #[test]
    fn renderer_counts_children_of_opaque_component_nodes_toward_node_limit() {
        let limits = crate::ValidationLimits::default();
        let children = (0..limits.maximum_nodes)
            .map(|index| json!({ "id": format!("opaque-{index}"), "type": "text" }))
            .collect::<Vec<_>>();
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "opaque-root",
                    "type": 42,
                    "components": children
                }
            }]
        }))
        .expect("document");
        assert!(matches!(
            render_page(&document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumNodesExceeded { maximum }) if maximum == limits.maximum_nodes
        ));
    }




    #[test]
    fn renderer_rejects_excessive_project_collections_before_traversal() {
        let page_items = (0..=crate::DEFAULT_MAXIMUM_PAGES)
            .map(|index| json!({ "id": format!("page-{index}") }))
            .collect::<Vec<_>>();
        let pages = GrapesJsCodec::decode_value(json!({ "pages": page_items }))
            .expect("pages document");
        assert!(matches!(
            render_page(&pages, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumPagesExceeded { maximum })
                if maximum == crate::DEFAULT_MAXIMUM_PAGES
        ));

        let assets = (0..=crate::DEFAULT_MAXIMUM_ASSETS)
            .map(|index| json!({ "id": format!("asset-{index}") }))
            .collect::<Vec<_>>();
        let assets_document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }],
            "assets": assets
        }))
        .expect("assets document");
        assert!(matches!(
            render_page(&assets_document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumAssetsExceeded { maximum })
                if maximum == crate::DEFAULT_MAXIMUM_ASSETS
        ));

        let styles = (0..=crate::DEFAULT_MAXIMUM_STYLE_RULES)
            .map(|index| json!({ "selectors": [format!(".rule-{index}")], "style": {} }))
            .collect::<Vec<_>>();
        let styles_document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }],
            "styles": styles
        }))
        .expect("styles document");
        assert!(matches!(
            render_page(&styles_document, &PageSelection::First, &RenderPolicy::default()),
            Err(FlyError::MaximumStyleRulesExceeded { maximum })
                if maximum == crate::DEFAULT_MAXIMUM_STYLE_RULES
        ));
    }


}
