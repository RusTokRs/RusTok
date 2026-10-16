mod localized_route;

pub use localized_route::*;

#[cfg(feature = "inline-edit")]
mod inline_edit;
#[cfg(feature = "inline-edit")]
pub use inline_edit::*;

use fly::{
    GrapesJsCodec, PageHead, RenderPolicy, RenderedPage, RichTextHtmlRenderer, RuntimeRenderResult,
};
use leptos::prelude::*;
use rustok_page_builder::render::{
    PageBuilderRuntimeRenderRequest, PageBuilderRuntimeRenderResponse, render_page_builder_runtime,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Page selection contract used by the public storefront renderer.
pub use fly::PageSelection;
pub use fly_browser;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorefrontPageOutput {
    pub page: RenderedPage,
}

impl StorefrontPageOutput {
    pub fn head(&self) -> &PageHead {
        &self.page.head
    }

    pub fn document_html(&self) -> String {
        self.page.document_html()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorefrontRuntimePageOutput {
    pub result: RuntimeRenderResult,
}

impl StorefrontRuntimePageOutput {
    pub fn page(&self) -> &RenderedPage {
        &self.result.page
    }

    pub fn head(&self) -> &PageHead {
        &self.result.page.head
    }

    pub fn document_html(&self) -> String {
        self.result.document_html()
    }
}

pub fn render_storefront_page(
    project_data: Value,
    selection: PageSelection,
    policy: RenderPolicy,
) -> fly::FlyResult<StorefrontPageOutput> {
    render_storefront_page_with_rich_text(project_data, selection, policy, None)
}

/// Render a storefront page with the host rich-text renderer bound, so `flyRichText`
/// components render through the platform rich-text capability instead of Fly's fallback
/// projection. Without a renderer this behaves exactly like [`render_storefront_page`].
/// Shared handle to a host rich-text renderer, suitable for Leptos props and SSR tasks.
pub type SharedRichTextRenderer = std::sync::Arc<dyn RichTextHtmlRenderer + Send + Sync>;

pub fn render_storefront_page_with_rich_text(
    project_data: Value,
    selection: PageSelection,
    mut policy: RenderPolicy,
    rich_text: Option<&dyn RichTextHtmlRenderer>,
) -> fly::FlyResult<StorefrontPageOutput> {
    policy.instrument_components = false;
    let document = GrapesJsCodec::decode_value(project_data)?;
    let linked = fly::materialize_internal_page_links(&document, &Value::Null);
    let action = fly::materialize_component_actions_with_providers(
        &linked.document,
        &Value::Null,
        Some(&rustok_page_builder::HostFormProviderEndpoints),
    );
    let page = fly::render_page_with_rich_text(&action.document, &selection, &policy, rich_text)?;
    Ok(StorefrontPageOutput { page })
}

pub fn render_storefront_page_with_context(
    project_data: Value,
    selection: PageSelection,
    mut policy: RenderPolicy,
    context: Value,
) -> fly::FlyResult<StorefrontRuntimePageOutput> {
    policy.instrument_components = false;
    let PageBuilderRuntimeRenderResponse { result } =
        render_page_builder_runtime(PageBuilderRuntimeRenderRequest {
            project_data,
            selection,
            policy,
            context,
        })?;
    Ok(StorefrontRuntimePageOutput { result })
}

/// Renders sanitized Fly HTML and scoped project CSS into a read-only storefront surface.
///
/// `inner_html` is safe here because the string is produced exclusively by Fly's sanitizer,
/// which strips raw tags from content, rejects event attributes and filters URLs/CSS values.
#[component]
pub fn PageBuilderStorefront(
    project_data: Value,
    #[prop(optional)] selection: Option<PageSelection>,
    #[prop(optional)] policy: Option<RenderPolicy>,
    #[prop(optional)] context: Option<Value>,
    #[prop(optional)] class: Option<String>,
    #[prop(optional)] rich_text_renderer: Option<SharedRichTextRenderer>,
) -> impl IntoView {
    let selection = selection.unwrap_or(PageSelection::First);
    let policy = policy.unwrap_or_default();
    let class = class.unwrap_or_else(|| "rustok-page-builder-storefront".to_string());

    let rendered = match context {
        Some(context) => {
            render_storefront_page_with_context(project_data, selection, policy, context).map(
                |output| {
                    let diagnostic_count = output.result.diagnostics.len();
                    let repeated_nodes = output.result.repeated_nodes;
                    (output.result.page, diagnostic_count, repeated_nodes)
                },
            )
        }
        None => render_storefront_page_with_rich_text(
            project_data,
            selection,
            policy,
            rich_text_renderer
                .as_deref()
                .map(|r| r as &dyn RichTextHtmlRenderer),
        )
        .map(|output| (output.page, 0, 0)),
    };

    match rendered {
        Ok((page, diagnostic_count, repeated_nodes)) => {
            let scoped_css = scope_landing_css(&page.css, ".rustok-page-builder-storefront");
            let html = page.html;
            let page_id = page.page_id.unwrap_or_default();
            let has_actions = html.contains("data-fly-action");
            view! {
                <section
                    class=class
                    style="isolation: isolate; contain: style layout;"
                    data-rustok-page-builder-storefront="true"
                    data-page-id=page_id
                    data-runtime-diagnostics=diagnostic_count
                    data-repeated-nodes=repeated_nodes
                >
                    <style data-fly-project-styles="true">{scoped_css}</style>
                    <div data-fly-page-body="true" inner_html=html></div>
                    {has_actions.then(|| view! {
                        <script data-fly-actions="true" inner_html=fly_browser::FLY_ACTIONS_JS></script>
                    })}
                    <script data-fly-storefront-bridge="true" inner_html=FLY_STOREFRONT_BRIDGE_JS></script>
                </section>
            }
            .into_any()
        }
        Err(error) => view! {
            <section
                class=class
                data-rustok-page-builder-storefront="true"
                data-render-error="true"
                role="alert"
            >
                <p>{error.to_string()}</p>
            </section>
        }
        .into_any(),
    }
}

/// Standalone action bridge script that bridges `fly:action` CustomEvents (e.g. `add_to_cart`,
/// `buy_now`, `navigate`) to the host storefront environment and intercepts internal in-page hash links.
pub const FLY_STOREFRONT_BRIDGE_JS: &str = r##"
(function () {
  "use strict";
  if (window.__rustok_fly_bridge_installed) return;
  window.__rustok_fly_bridge_installed = true;

  window.addEventListener("fly:action", function (e) {
    var detail = e.detail || {};
    var event = detail.event || detail.action || "";
    var data = detail.payload || detail.input || {};

    if (event === "add_to_cart" || event === "cart:add" || event === "buy_now") {
      var variantId = data.variantId || data.productId || data.id;
      var quantity = Number(data.quantity) || 1;
      if (variantId) {
        window.dispatchEvent(new CustomEvent("rustok:cart:add", {
          bubbles: true,
          detail: { variantId: String(variantId), quantity: quantity, openDrawer: true }
        }));
      }
    } else if (event === "navigate") {
      var href = data.href || data.url;
      if (href && typeof href === "string") {
        window.location.assign(href);
      }
    }
  });

  document.addEventListener("click", function (e) {
    var target = e.target;
    if (!target || typeof target.closest !== "function") return;
    var anchor = target.closest("a");
    if (!anchor) return;
    if (anchor.target === "_blank" || anchor.hasAttribute("download") || anchor.hasAttribute("data-fly-action")) return;
    var href = anchor.getAttribute("href");
    if (!href) return;
    if (href.startsWith("#")) {
      var el = document.querySelector(href);
      if (el) {
        e.preventDefault();
        el.scrollIntoView({ behavior: "smooth" });
      }
    }
  });
})();
"##;

/// Scopes landing CSS using modern CSS `@scope (<scope_selector>)` and normalizes
/// root-level selectors (`body`, `html`) to `:scope`, ensuring styles from user-generated
/// or GrapesJS landings cannot leak into the host application's shell, header, or footer.
pub fn scope_landing_css(raw_css: &str, scope_selector: &str) -> String {
    let trimmed = raw_css.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("@scope") {
        return trimmed.to_string();
    }

    let remapped = normalize_landing_root_selectors(trimmed);
    format!(
        "@scope ({scope_selector}) {{\n  :scope {{\n    isolation: isolate;\n  }}\n{remapped}\n}}"
    )
}

fn normalize_landing_root_selectors(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let mut in_rule_block = false;
    let mut in_quote: Option<char> = None;

    let chars: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        if let Some(quote) = in_quote {
            result.push(ch);
            if ch == quote && (i == 0 || chars[i - 1] != '\\') {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            result.push(ch);
            i += 1;
            continue;
        }

        if ch == '{' {
            in_rule_block = true;
            result.push(ch);
            i += 1;
            continue;
        } else if ch == '}' {
            in_rule_block = false;
            result.push(ch);
            i += 1;
            continue;
        }

        if !in_rule_block {
            let remaining = &css[i..];
            if remaining.starts_with("body") && is_identifier_boundary(&chars, i, 4) {
                result.push_str(":scope");
                i += 4;
                continue;
            } else if remaining.starts_with("html") && is_identifier_boundary(&chars, i, 4) {
                result.push_str(":scope");
                i += 4;
                continue;
            }
        }

        result.push(ch);
        i += 1;
    }

    result
}

fn is_identifier_boundary(chars: &[char], start: usize, len: usize) -> bool {
    let prev_is_ident = if start > 0 {
        let prev = chars[start - 1];
        prev.is_alphanumeric() || prev == '-' || prev == '_' || prev == '.' || prev == '#'
    } else {
        false
    };
    if prev_is_ident {
        return false;
    }

    let end = start + len;
    if end < chars.len() {
        let next = chars[end];
        let next_is_ident = next.is_alphanumeric() || next == '-' || next == '_';
        if next_is_ident {
            return false;
        }
    }

    true
}

/// Renders a pre-compiled, verified static landing artifact (SSR HTML and scoped CSS)
/// into a read-only storefront surface without an iframe.
///
/// If `html` contains interactive Fly actions (`data-fly-action`), the lightweight
/// `fly-actions.js` runtime script is inlined to handle toggles, smooth scroll,
/// animations, and `fly:action` custom events.
#[component]
pub fn StaticLandingStorefront(
    html: String,
    css: String,
    #[prop(optional)] page_id: Option<String>,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| "rustok-page-builder-storefront".to_string());
    let page_id = page_id.unwrap_or_default();
    let has_actions = html.contains("data-fly-action");
    let scoped_css = scope_landing_css(&css, ".rustok-page-builder-storefront");
    view! {
        <section
            class=class
            style="isolation: isolate; contain: style layout;"
            data-rustok-page-builder-storefront="true"
            data-page-id=page_id
            data-page-builder-body="artifact"
        >
            <style data-fly-project-styles="true">{scoped_css}</style>
            <div data-fly-page-body="true" inner_html=html></div>
            {has_actions.then(|| view! {
                <script data-fly-actions="true" inner_html=fly_browser::FLY_ACTIONS_JS></script>
            })}
            <script data-fly-storefront-bridge="true" inner_html=FLY_STOREFRONT_BRIDGE_JS></script>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn static_landing_storefront_component_constructs_view() {
        let _view = view! {
            <StaticLandingStorefront
                html="<div data-fly-action=\"test\">Landing</div>".to_string()
                css=".landing { color: blue; }".to_string()
                page_id="p1".to_string()
            />
        };
    }

    #[test]
    fn scope_landing_css_wraps_and_remaps_root_selectors() {
        let raw = "body { margin: 0; background: #fff; }\nhtml, body { height: 100%; }\n.hero { color: red; }\ntbody { display: table-row-group; }\n.some-body { color: blue; }";
        let scoped = scope_landing_css(raw, ".rustok-page-builder-storefront");

        assert!(scoped.starts_with("@scope (.rustok-page-builder-storefront) {"));
        assert!(scoped.contains(":scope {\n    isolation: isolate;"));
        assert!(scoped.contains(":scope { margin: 0; background: #fff; }"));
        assert!(scoped.contains(":scope, :scope { height: 100%; }"));
        assert!(scoped.contains(".hero { color: red; }"));
        // Ensure tbody and .some-body are NOT accidentally remapped
        assert!(scoped.contains("tbody { display: table-row-group; }"));
        assert!(scoped.contains(".some-body { color: blue; }"));
    }

    #[test]
    fn scope_landing_css_handles_empty_and_preserves_already_scoped() {
        assert_eq!(scope_landing_css("", ".scope"), "");
        assert_eq!(scope_landing_css("   ", ".scope"), "");

        let already_scoped = "@scope (.test) { :scope { color: green; } }";
        assert_eq!(scope_landing_css(already_scoped, ".other"), already_scoped);
    }

    #[test]
    fn storefront_helper_forces_editor_instrumentation_off() {
        let output = render_storefront_page(
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "heading",
                            "type": "heading",
                            "content": "Hello"
                        }]
                    }
                }]
            }),
            PageSelection::Id("home".to_string()),
            RenderPolicy {
                instrument_components: true,
                ..RenderPolicy::default()
            },
        )
        .expect("storefront output");
        assert!(!output.page.html.contains("data-fly-component-id"));
        assert!(output.page.html.contains("data-fly-style-id"));
    }

    #[test]
    fn runtime_storefront_materializes_repeaters() {
        let output = render_storefront_page_with_context(
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "item",
                            "type": "text",
                            "content": "{{entry.name}}"
                        }]
                    }
                }],
                "flyRuntimeRepeaters": [{
                    "id": "items",
                    "component_id": "item",
                    "path": "entries",
                    "item_alias": "entry"
                }]
            }),
            PageSelection::First,
            RenderPolicy::default(),
            json!({ "entries": [{ "name": "One" }, { "name": "Two" }] }),
        )
        .expect("runtime storefront output");
        assert_eq!(output.result.repeated_nodes, 2);
        assert!(output.result.page.html.contains("One"));
        assert!(output.result.page.html.contains("Two"));
    }

    #[test]
    fn storefront_helper_exposes_head_and_full_document() {
        let output = render_storefront_page(
            json!({
                "pages": [{
                    "id": "home",
                    "flyPageMeta": { "title": "Home" },
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
            PageSelection::First,
            RenderPolicy::default(),
        )
        .expect("storefront output");
        assert_eq!(output.head().title.as_deref(), Some("Home"));
        assert!(output.document_html().contains("<title>Home</title>"));
    }

    #[test]
    fn storefront_helper_renders_action_runtime_attributes() {
        let output = render_storefront_page(
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "btn",
                            "type": "button",
                            "content": "Toggle",
                            "flyAction": {
                                "kind": "toggle_visibility",
                                "target_id": "panel"
                            }
                        }, {
                            "id": "panel",
                            "type": "section",
                            "content": "Panel"
                        }]
                    }
                }]
            }),
            PageSelection::First,
            RenderPolicy::default(),
        )
        .expect("storefront output");
        assert!(output.page.html.contains("data-fly-action"));
        assert!(
            output
                .page
                .html
                .contains("data-fly-action-kind=\"toggle_visibility\"")
        );
        assert!(!fly_browser::FLY_ACTIONS_JS.is_empty());
    }
}
