use fly::{ComponentNode, ComponentObject, ProjectDocument};
use fly_leptos::FLY_IFRAME_PROTOCOL_V1;
use serde_json::Value;

const CANVAS_SCRIPT: &str = r#"
(() => {
  const protocol = __FLY_PROTOCOL__;
  const instanceId = __FLY_INSTANCE__;
  let sequence = 0;
  let scheduled = false;

  const send = (type, payload = {}) => {
    parent.postMessage(JSON.stringify({
      protocol,
      instance_id: instanceId,
      sequence: ++sequence,
      message: { type, ...payload },
    }), '*');
  };

  const reportViewport = () => {
    send('viewport_changed', {
      width: Math.max(0, Math.round(window.innerWidth)),
      height: Math.max(0, Math.round(window.innerHeight)),
      scroll_x: window.scrollX,
      scroll_y: window.scrollY,
      zoom: window.devicePixelRatio > 0 ? window.devicePixelRatio : 1,
    });
  };

  const measure = () => {
    scheduled = false;
    const components = Array.from(document.querySelectorAll('[data-fly-component-id]')).map((element) => {
      const rect = element.getBoundingClientRect();
      const parentElement = element.parentElement?.closest('[data-fly-component-id]');
      return {
        component_id: element.dataset.flyComponentId,
        parent_component_id: parentElement?.dataset.flyComponentId ?? null,
        index: Number.parseInt(element.dataset.flyIndex ?? '0', 10) || 0,
        rect: {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
        },
      };
    });
    send('geometry_snapshot', { components });
  };

  const scheduleMeasure = () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(measure);
  };

  const componentAt = (target) => target instanceof Element
    ? target.closest('[data-fly-component-id]')
    : null;

  document.addEventListener('click', (event) => {
    const component = componentAt(event.target);
    document.querySelectorAll('[data-fly-selected]').forEach((node) => node.removeAttribute('data-fly-selected'));
    if (component) component.setAttribute('data-fly-selected', 'true');
    send('focus_requested', {
      component_id: component?.dataset.flyComponentId ?? null,
    });
  });

  document.addEventListener('pointerover', (event) => {
    const component = componentAt(event.target);
    send('hover_requested', {
      component_id: component?.dataset.flyComponentId ?? null,
    });
  });

  document.addEventListener('pointerleave', () => {
    send('hover_requested', { component_id: null });
  });

  let pointerFrame = false;
  document.addEventListener('pointermove', (event) => {
    if (pointerFrame) return;
    pointerFrame = true;
    requestAnimationFrame(() => {
      pointerFrame = false;
      const kind = ['mouse', 'touch', 'pen'].includes(event.pointerType)
        ? event.pointerType
        : 'unknown';
      send('pointer_moved', {
        sample: {
          pointer_id: event.pointerId,
          kind,
          position: { x: event.clientX, y: event.clientY },
          buttons: event.buttons,
          primary: event.isPrimary,
        },
      });
    });
  }, { passive: true });

  const observer = new ResizeObserver(scheduleMeasure);
  observer.observe(document.documentElement);
  document.querySelectorAll('[data-fly-component-id]').forEach((node) => observer.observe(node));

  window.addEventListener('resize', () => {
    reportViewport();
    scheduleMeasure();
  }, { passive: true });
  window.addEventListener('scroll', () => {
    reportViewport();
    scheduleMeasure();
  }, { passive: true });

  reportViewport();
  scheduleMeasure();
  send('ready');
})();
"#;

pub fn render_canvas_srcdoc(document: &ProjectDocument, instance_id: &str) -> String {
    let mut canvas = String::new();
    if let Some(root) = document
        .project
        .pages
        .iter()
        .find_map(|page| page.component.as_ref())
    {
        render_node(root, None, 0, &mut canvas);
    } else {
        canvas.push_str("<div class=\"fly-empty\">No editable root component</div>");
    }

    let protocol = serde_json::to_string(FLY_IFRAME_PROTOCOL_V1)
        .unwrap_or_else(|_| "\"fly_iframe_v1\"".to_string());
    let instance = serde_json::to_string(instance_id).unwrap_or_else(|_| "\"fly-canvas\"".to_string());
    let script = CANVAS_SCRIPT
        .replace("__FLY_PROTOCOL__", &protocol)
        .replace("__FLY_INSTANCE__", &instance);

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: https: http:; font-src data: https: http:;\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><style>{}</style></head><body><main id=\"fly-canvas-root\">{}</main><script>{}</script></body></html>",
        canvas_styles(),
        canvas,
        script
    )
}

fn render_node(node: &ComponentNode, parent_id: Option<&str>, index: usize, output: &mut String) {
    match node {
        ComponentNode::Opaque(value) => render_opaque(value, output),
        ComponentNode::Object(component) => render_component(component, parent_id, index, output),
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
    let is_void = matches!(tag, "img" | "input" | "hr" | "br");

    output.push('<');
    output.push_str(tag);
    output.push_str(" data-fly-component-id=\"");
    output.push_str(&escape_attribute(component_id));
    output.push_str("\" data-fly-index=\"");
    output.push_str(&index.to_string());
    output.push('"');
    if let Some(parent_id) = parent_id {
        output.push_str(" data-fly-parent-id=\"");
        output.push_str(&escape_attribute(parent_id));
        output.push('"');
    }

    for (name, value) in &component.attributes {
        if !safe_attribute_name(name) || matches!(name.as_str(), "style" | "srcdoc") {
            continue;
        }
        let Some(value) = attribute_value(value) else {
            continue;
        };
        if matches!(name.as_str(), "href" | "src" | "action") && !safe_url(&value) {
            continue;
        }
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        output.push_str(&escape_attribute(&value));
        output.push('"');
    }

    if let Some(style) = component.style.as_ref().and_then(Value::as_object) {
        let style = style
            .iter()
            .filter_map(|(name, value)| safe_style(name, value))
            .collect::<Vec<_>>()
            .join(";");
        if !style.is_empty() {
            output.push_str(" style=\"");
            output.push_str(&escape_attribute(&style));
            output.push('"');
        }
    }

    output.push('>');
    if is_void {
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
            "wrapper" => "div",
            "section" => "section",
            "heading" => "h2",
            "text" => "p",
            "link" => "a",
            "image" => "img",
            "button" => "button",
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
        "p" => "p",
        "span" => "span",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "a" => "a",
        "button" => "button",
        "img" => "img",
        "ul" => "ul",
        "ol" => "ol",
        "li" => "li",
        "blockquote" => "blockquote",
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
        || value.contains(['<', '>'])
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
    escape_html(value).replace('"', "&quot;").replace('\'', "&#39;")
}

fn canvas_styles() -> &'static str {
    r#"html{box-sizing:border-box;background:#fff;color:#111827;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*,*:before,*:after{box-sizing:inherit}body{margin:0;min-height:100vh}#fly-canvas-root{min-height:100vh;padding:24px}[data-fly-component-id]{position:relative;min-height:20px;outline:1px solid transparent;outline-offset:2px}[data-fly-component-id]:hover{outline-color:rgba(59,130,246,.45)}[data-fly-selected]{outline:2px solid #2563eb!important}.fly-empty{display:grid;min-height:320px;place-items:center;border:1px dashed #94a3b8;border-radius:12px;color:#64748b}"#
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
        assert!(html.contains("canvas-home"));
    }
}
