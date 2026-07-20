#[cfg(any(target_arch = "wasm32", test))]
use fly::render_page;
use fly::{
    render_page_with_runtime_context, PageSelection, ProjectDocument, RenderPolicy, RenderedPage,
};
use fly_leptos::FLY_IFRAME_PROTOCOL;
use serde_json::Value;

const CANVAS_SCRIPT: &str = include_str!("canvas_runtime.js");
const CANVAS_MAX_GEOMETRY_COMPONENTS: usize = 4096;

#[cfg(any(target_arch = "wasm32", test))]
pub fn render_canvas_srcdoc(document: &ProjectDocument, instance_id: &str) -> String {
    let rendered = render_page(document, &PageSelection::First, &canvas_render_policy());
    render_srcdoc(rendered, instance_id, 0, 0)
}

pub fn render_canvas_srcdoc_with_context(
    document: &ProjectDocument,
    instance_id: &str,
    context: &Value,
) -> String {
    match render_page_with_runtime_context(
        document,
        &PageSelection::First,
        &canvas_render_policy(),
        context,
    ) {
        Ok(result) => render_srcdoc(
            Ok(result.page),
            instance_id,
            result.diagnostics.len(),
            result.repeated_nodes,
        ),
        Err(error) => render_srcdoc(Err(error), instance_id, 0, 0),
    }
}

fn canvas_render_policy() -> RenderPolicy {
    RenderPolicy {
        instrument_components: true,
        emit_style_hooks: true,
        ..RenderPolicy::default()
    }
}

fn render_srcdoc(
    rendered: fly::FlyResult<RenderedPage>,
    instance_id: &str,
    runtime_diagnostics: usize,
    repeated_nodes: usize,
) -> String {
    let (head, canvas, project_styles) = match rendered {
        Ok(rendered) => (rendered.head.render_html(), rendered.html, rendered.css),
        Err(error) => (
            String::new(),
            format!(
                "<div class=\"fly-empty\">{}</div>",
                escape_html(&error.to_string())
            ),
            String::new(),
        ),
    };

    let protocol =
        serde_json::to_string(FLY_IFRAME_PROTOCOL).unwrap_or_else(|_| "\"fly_iframe\"".to_string());
    let instance =
        serde_json::to_string(instance_id).unwrap_or_else(|_| "\"fly-canvas\"".to_string());
    let geometry_limit = CANVAS_MAX_GEOMETRY_COMPONENTS.to_string();
    let script = CANVAS_SCRIPT
        .replace("__FLY_PROTOCOL__", &protocol)
        .replace("__FLY_INSTANCE__", &instance)
        .replace("__FLY_MAX_GEOMETRY_COMPONENTS__", &geometry_limit);

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: https: http:; media-src data: https: http:; font-src data: https: http:;\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{head}<style>{}{}</style></head><body data-runtime-diagnostics=\"{runtime_diagnostics}\" data-repeated-nodes=\"{repeated_nodes}\"><main id=\"fly-canvas-root\">{canvas}</main><script>{script}</script></body></html>",
        canvas_styles(), project_styles,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn canvas_styles() -> &'static str {
    r#"html{box-sizing:border-box;background:#fff;color:#111827;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*,*:before,*:after{box-sizing:inherit}body{margin:0;min-height:100vh}#fly-canvas-root{min-height:100vh}[data-fly-component-id]{position:relative;min-height:20px;outline:1px solid transparent;outline-offset:2px}[data-fly-component-id]:hover{outline-color:rgba(59,130,246,.45)}[data-fly-selected]{outline:2px solid #2563eb!important}img,video{max-width:100%}input,textarea,select,button{font:inherit}.fly-empty{display:grid;min-height:320px;place-items:center;border:1px dashed #94a3b8;border-radius:12px;color:#64748b}"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::GrapesJsCodec;
    use serde_json::json;

    #[test]
    fn renderer_instruments_components_and_escapes_script_content() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "section",
                        "components": ["<script>alert(1)</script>Hello"]
                    }]
                }
            }]
        }))
        .expect("decode");
        let html = render_canvas_srcdoc(&document, "canvas-home");
        assert!(html.contains("data-fly-component-id=\"hero\""));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("geometry_snapshot"));
        assert!(html.contains("setTimeout(announce, 100)"));
        assert!(html.contains("const configuredGeometryLimit = 4096"));
        assert!(html.contains("geometry_components"));
        assert!(!html.contains("__FLY_MAX_GEOMETRY_COMPONENTS__"));
        assert!(html.contains("canvas-home"));
    }

    #[test]
    fn runtime_renderer_materializes_conditions_and_repeaters() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "banner",
                        "type": "text",
                        "content": "Banner"
                    }, {
                        "id": "row",
                        "type": "text",
                        "content": "{{item.name}}"
                    }]
                }
            }],
            "flyRuntimeConditions": [{
                "id": "show-banner",
                "component_id": "banner",
                "path": "showBanner",
                "operator": "truthy"
            }],
            "flyRuntimeRepeaters": [{
                "id": "rows",
                "component_id": "row",
                "path": "items"
            }]
        }))
        .expect("decode");
        let html = render_canvas_srcdoc_with_context(
            &document,
            "canvas-home",
            &json!({
                "showBanner": false,
                "items": [{ "name": "One" }, { "name": "Two" }]
            }),
        );
        assert!(!html.contains("Banner"));
        assert!(html.contains("One"));
        assert!(html.contains("Two"));
        assert!(html.contains("data-repeated-nodes=\"2\""));
    }

    #[test]
    fn renderer_rejects_event_attributes_and_javascript_urls() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "link",
                    "attributes": {
                        "onclick": "alert(1)",
                        "href": "javascript:alert(1)"
                    }
                }
            }]
        }))
        .expect("decode");
        let html = render_canvas_srcdoc(&document, "canvas-home");
        assert!(!html.contains("onclick="));
        assert!(!html.contains("href=\"javascript:"));
    }

    #[test]
    fn renderer_applies_component_media_rules_from_shared_renderer() {
        let document = GrapesJsCodec::decode_value(json!({
            "styles": [{
                "selectors": [{ "name": "hero", "type": 2 }],
                "style": { "padding": "24px" },
                "atRuleType": "media",
                "mediaText": "(max-width: 767px)",
                "flyComponentId": "hero"
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                }
            }]
        }))
        .expect("decode");
        let html = render_canvas_srcdoc(&document, "canvas-home");
        assert!(html.contains("@media (max-width: 767px)"));
        assert!(html.contains("[data-fly-style-id=\"hero\"]{padding:24px}"));
    }
}
