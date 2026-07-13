use fly::{render_page, PageSelection, ProjectDocument, RenderPolicy};
use fly_leptos::FLY_IFRAME_PROTOCOL_V1;

const CANVAS_SCRIPT: &str = include_str!("canvas_runtime.js");

pub fn render_canvas_srcdoc(document: &ProjectDocument, instance_id: &str) -> String {
    let rendered = render_page(
        document,
        &PageSelection::First,
        &RenderPolicy {
            instrument_components: true,
            emit_style_hooks: true,
            ..RenderPolicy::default()
        },
    );
    let (head, canvas, project_styles) = match rendered {
        Ok(rendered) => (
            rendered.head.render_html(),
            rendered.html,
            rendered.css,
        ),
        Err(error) => (
            String::new(),
            format!(
                "<div class=\"fly-empty\">{}</div>",
                escape_html(&error.to_string())
            ),
            String::new(),
        ),
    };

    let protocol = serde_json::to_string(FLY_IFRAME_PROTOCOL_V1)
        .unwrap_or_else(|_| "\"fly_iframe_v1\"".to_string());
    let instance = serde_json::to_string(instance_id)
        .unwrap_or_else(|_| "\"fly-canvas\"".to_string());
    let script = CANVAS_SCRIPT
        .replace("__FLY_PROTOCOL__", &protocol)
        .replace("__FLY_INSTANCE__", &instance);

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: https: http:; media-src data: https: http:; font-src data: https: http:;\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{head}<style>{}{}</style></head><body><main id=\"fly-canvas-root\">{canvas}</main><script>{script}</script></body></html>",
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
    use fly::GrapesJsV1Codec;
    use serde_json::json;

    #[test]
    fn renderer_instruments_components_and_escapes_script_content() {
        let document = GrapesJsV1Codec::decode_value(json!({
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
        assert!(html.contains("canvas-home"));
    }

    #[test]
    fn renderer_rejects_event_attributes_and_javascript_urls() {
        let document = GrapesJsV1Codec::decode_value(json!({
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
    fn renderer_supports_forms_media_and_boolean_attributes() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "form", "type": "form", "components": [
                            { "id": "input", "type": "input", "attributes": { "type": "email", "required": true } },
                            { "id": "submit", "type": "submit", "content": "Send" }
                        ] },
                        { "id": "video", "type": "video", "attributes": { "controls": true } }
                    ]
                }
            }]
        }))
        .expect("decode");
        let html = render_canvas_srcdoc(&document, "canvas-home");
        assert!(html.contains("<form"));
        assert!(html.contains("<input"));
        assert!(html.contains(" required"));
        assert!(html.contains("<video"));
        assert!(html.contains(" controls"));
    }

    #[test]
    fn renderer_applies_component_media_rules_from_shared_renderer() {
        let document = GrapesJsV1Codec::decode_value(json!({
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
