mod localized_route;

pub use localized_route::*;

use fly::{PageHead, RenderPolicy, RenderedPage, RuntimeRenderResult};
use leptos::prelude::*;
use rustok_page_builder::render::{
    PageBuilderRenderRequest, PageBuilderRenderResponse, PageBuilderRuntimeRenderRequest,
    PageBuilderRuntimeRenderResponse, render_page_builder_project, render_page_builder_runtime,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Page selection contract used by the public storefront renderer.
pub use fly::PageSelection;

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
    mut policy: RenderPolicy,
) -> fly::FlyResult<StorefrontPageOutput> {
    policy.instrument_components = false;
    let PageBuilderRenderResponse { page } =
        render_page_builder_project(PageBuilderRenderRequest {
            project_data,
            selection,
            policy,
        })?;
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
        None => render_storefront_page(project_data, selection, policy)
            .map(|output| (output.page, 0, 0)),
    };

    match rendered {
        Ok((page, diagnostic_count, repeated_nodes)) => {
            let css = page.css;
            let html = page.html;
            let page_id = page.page_id.unwrap_or_default();
            view! {
                <section
                    class=class
                    data-rustok-page-builder-storefront="true"
                    data-page-id=page_id
                    data-runtime-diagnostics=diagnostic_count
                    data-repeated-nodes=repeated_nodes
                >
                    <style data-fly-project-styles="true">{css}</style>
                    <div data-fly-page-body="true" inner_html=html></div>
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
