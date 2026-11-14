//! # rustok-page-builder-storefront
//!
//! Leptos storefront renderer and scoped CSS container for RusToK Page Builder documents.
//!
//! ## Responsibilities & Guarantees
//!
//! - **Strict FFA Isolation**: View-models and pure Rust presentation logic in `core` remain decoupled from Leptos signals.
//! - **Scoped CSS Containment**: Every surface wraps project CSS in a native `@scope` rule rooted at the
//!   landing body element it renders (`LANDING_SCOPE_SELECTOR`) and remaps root selectors
//!   (`body`, `html`, `:root`) to `:scope`.
//! - **Direct SSR**: Renders pre-compiled landing artifacts without an iframe. Mounted hydration and
//!   client navigation still require host-level verification.
//! - **Event & Navigation Bridge**: Validates Product buyer handoffs and requests host-owned Cart actions.

pub mod core;
mod css_scope;
mod i18n;
mod localized_route;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use css_scope::{LANDING_SCOPE_SELECTOR, scope_landing_css};
pub use localized_route::{
    LocalizedPageBuilderStorefront, StorefrontLocalizedRouteOutput,
    render_storefront_localized_request, render_storefront_localized_slug,
};
pub use ui::PageBuilderView;

#[cfg(feature = "inline-edit")]
mod inline_edit;
#[cfg(feature = "inline-edit")]
pub use inline_edit::{
    AuthenticatedInlineEditSession, InlineEditApplyResult, InlineEditAuthorizationPort,
    InlineEditError, InlineEditGrant, InlineEditRequest, PageBuilderAuthenticatedInlineStorefront,
};

use fly::{
    GrapesJsCodec, PageHead, RenderPolicy, RenderedPage, RichTextHtmlRenderer, RuntimeRenderResult,
};
use leptos::prelude::*;
use rustok_page_builder::render::{
    PageBuilderRuntimeRenderRequest, PageBuilderRuntimeRenderResponse, render_page_builder_runtime,
};
use rustok_ui_core::{UiCspNonce, UiRouteContext};
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

// Fixed host-owned selectors only. A nonce-bearing style element works with style-src-attr
// 'none', unlike a style attribute on the wrapper. Authored CSS is scoped separately below.
const LANDING_CONTAINMENT_CSS: &str = "section[data-rustok-page-builder-storefront],section[data-rustok-page-builder-inline-storefront]{isolation:isolate;contain:style layout}\n";

fn storefront_scoped_css(css: &str) -> String {
    format!(
        "{LANDING_CONTAINMENT_CSS}{}",
        scope_landing_css(css, LANDING_SCOPE_SELECTOR)
    )
}

fn storefront_csp_nonce() -> Option<String> {
    use_context::<UiCspNonce>().map(|nonce| nonce.as_str().to_string())
}

#[component]
fn LandingBridgeFeedback() -> impl IntoView {
    let route = use_context::<UiRouteContext>().unwrap_or_default();
    let messages = core::landing_bridge_messages(route.locale.as_deref());
    view! {
        <p data-fly-bridge-error="buyer_handoff_invalid" role="alert" hidden>{messages.buyer_handoff_error}</p>
        <p data-fly-bridge-error="cart_unavailable" role="alert" hidden>{messages.cart_unavailable}</p>
    }
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
            let scoped_css = storefront_scoped_css(&page.css);
            let html = page.html;
            let page_id = page.page_id.unwrap_or_default();
            let has_actions = html.contains("data-fly-action");
            let style_nonce = storefront_csp_nonce();
            let runtime_nonce = style_nonce.clone();
            let bridge_nonce = style_nonce.clone();
            view! {
                <section
                    class=class
                    data-rustok-page-builder-storefront="true"
                    data-page-id=page_id
                    data-runtime-diagnostics=diagnostic_count
                    data-repeated-nodes=repeated_nodes
                >
                    <style data-fly-project-styles="true" nonce=style_nonce>{scoped_css}</style>
                    <div data-fly-page-body="true" inner_html=html></div>
                    <LandingBridgeFeedback />
                    {has_actions.then(|| view! {
                        <script data-fly-actions="true" nonce=runtime_nonce inner_html=fly_browser::FLY_ACTIONS_JS></script>
                    })}
                    <script data-fly-storefront-bridge="true" nonce=bridge_nonce inner_html=FLY_STOREFRONT_BRIDGE_JS></script>
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

/// Browser bridge for validated Product handoffs, Cart control requests and landing-local anchors.
/// Cart requests require an acknowledging host consumer; this asset never performs a Cart mutation.
pub const FLY_STOREFRONT_BRIDGE_JS: &str = include_str!("../assets/fly-storefront-bridge.js");

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
    let scoped_css = storefront_scoped_css(&css);
    let style_nonce = storefront_csp_nonce();
    let runtime_nonce = style_nonce.clone();
    let bridge_nonce = style_nonce.clone();
    view! {
        <section
            class=class
            data-rustok-page-builder-storefront="true"
            data-page-id=page_id
            data-page-builder-body="artifact"
        >
            <style data-fly-project-styles="true" nonce=style_nonce>{scoped_css}</style>
            <div data-fly-page-body="true" inner_html=html></div>
            <LandingBridgeFeedback />
            {has_actions.then(|| view! {
                <script data-fly-actions="true" nonce=runtime_nonce inner_html=fly_browser::FLY_ACTIONS_JS></script>
            })}
            <script data-fly-storefront-bridge="true" nonce=bridge_nonce inner_html=FLY_STOREFRONT_BRIDGE_JS></script>
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
