use fly::{ComponentNode, ComponentObject, ProjectDocument};
use fly_leptos::FLY_IFRAME_PROTOCOL_V1;
use serde_json::Value;

const CANVAS_SCRIPT: &str = include_str!("canvas_runtime.js");

pub fn render_canvas_srcdoc(document: &ProjectDocument, instance_id: &str) -> String {
    let mut canvas = String::new();
    match document
        .project
        .pages
        .iter()
        .find_map(|page| page.component.as_ref())
    {
        Some(root) => render_node(root, None, 0, &mut canvas),
        None => canvas.push_str("<div class=\"fly-empty\">No editable root component</div>"),
    }

    let protocol = serde_json::to_string(FLY_IFRAME_PROTOCOL_V1)
        .unwrap_or_else(|_| "\"fly_iframe_v1\"".to_string());
    let instance = serde_json::to_string(instance_id)
        .unwrap_or_else(|_| "\"fly-canvas\"".to_string());
    let script = CANVAS_SCRIPT
        .replace("__FLY_PROTOCOL__", &protocol)
        .replace("__FLY_INSTANCE__", &instance);

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: https: http:; media-src data: https: http:; font-src data: https: http:;\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><style>{}</style></head><body><main id=\"fly-canvas-root\">{}</main><script>{}</script></body></html>",
        canvas_styles(), canvas, script
    )
}

fn render_node(node: &ComponentNode, parent_id: Option<&str>, index: usize, output: &mut String) {
    match node {
        ComponentNode::Object(component) => render_component(component, parent_id, index, output),
        ComponentNode::Opaque(value) => render_opaque(value, output),
    }
}

fn render_component(
    component: &ComponentObject,
    parent_id: Option<&str>,
    index: usize,
    output: &mut String,
) {
    let component_id = component.id.as_deref().unwrap_or("fly-component");
    let tag = safe_tag(component);
    let void_tag = matches!(tag, "img" | "input" | "hr" | "br");

    output.push('<');
    output.push_str(tag);
    write_attribute(output, "data-fly-component-id", component_id);
    write_attribute(output, "data-fly-index", &index.to_string());
    if let Some(parent_id) = parent_id {
        write_attribute(output, "data-fly-parent-id", parent_id);
    }

    for (name, value) in &component.attributes {
        let Some(value) = attribute_value(value) else {
            continue;
        };
        if !safe_attribute_name(name)
            || matches!(name.as_str(), "style" | "srcdoc")
            || (matches!(name.as_str(), "href" | "src" | "poster" | "action")
                && !safe_url(&value))
        {
            continue;
        }
        write_attribute(output, name, &value);
    }

    if let Some(style) = component.style.as_ref().and_then(Value::as_object) {
        let style = style
            .iter()
            .filter_map(|(name, value)| safe_style(name, value))
            .collect::<Vec<_>>()
            .join(";");
        if !style.is_empty() {
            write_attribute(output, "style", &style);
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
        render_node(child, Some(component_id), child_index, output);
    }
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
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
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':'))
}

fn attribute_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn safe_url(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.starts_with('#')
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("data:image/")
        || normalized.starts_with("data:video/")
}

fn safe_style(name: &str, value: &Value) -> Option<String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '-')
    {
        return None;
    }
    let value = attribute_value(value)?;
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("expression(")
        || normalized.contains("javascript:")
        || normalized.contains("url(")
        || value.contains('<')
        || value.contains('>')
    {
        return None;
    }
    Some(format!("{name}:{value}"))
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
    fn renderer_supports_forms_and_media_primitives() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "form", "type": "form", "components": [
                            { "id": "input", "type": "input", "attributes": { "type": "email" } },
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
        assert!(html.contains("<video"));
    }
}
