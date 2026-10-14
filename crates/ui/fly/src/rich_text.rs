//! Rich-text payloads carried by components.
//!
//! A component may carry a `flyRichText` extension holding a [`RichTextPayload`]: an opaque
//! ProseMirror/Tiptap-shaped document owned by the platform's rich-text capability
//! (`rustok-content::richtext` in RusToK). Fly never reimplements that capability. It stores
//! and round-trips the payload, structurally validates it (shape, depth, size), extracts
//! plain text, and renders it through the host-supplied [`RichTextHtmlRenderer`] port. When
//! no renderer is bound — a bare standalone `fly` consumer — the renderer falls back to a
//! conservative semantic projection built here: block structure and the basic inline marks,
//! with every text node escaped and every link passed through the same URL policy as the
//! rest of the renderer. The fallback is a reading view, never an editing surface.

use crate::RichTextPayload;
use crate::render::{push_escaped_attribute, push_escaped_html};
use crate::safe_url::{UrlAttributeKind, UrlPolicy, url_allowed};
use serde_json::Value;

pub const FLY_RICH_TEXT_FIELD: &str = "flyRichText";

/// Hard bounds for a stored payload. They mirror the spirit of `ValidationLimits`: a payload
/// is author input and is validated before anything walks it recursively.
pub const MAXIMUM_RICH_TEXT_DEPTH: usize = 32;
pub const MAXIMUM_RICH_TEXT_NODES: usize = 5_000;
pub const MAXIMUM_RICH_TEXT_BYTES: usize = 512 * 1024;

/// The capability identifier Fly records for platform rich-text payloads. The owning
/// capability (profile selection, normalization, sanitization) stays with the host.
pub const RICH_TEXT_CAPABILITY: &str = "richtext";

/// Renders a validated rich-text payload to **safe HTML**.
///
/// Implementations own profile validation and sanitization (in RusToK:
/// `rustok-content::richtext::render_html`). The returned string is trusted by the Fly
/// renderer exactly to the extent the host trusts its own rich-text capability; returning
/// `None` declines the payload and Fly uses its escaped fallback projection instead.
pub trait RichTextHtmlRenderer {
    fn render_rich_text(&self, payload: &RichTextPayload) -> Option<String>;
}

impl<F> RichTextHtmlRenderer for F
where
    F: Fn(&RichTextPayload) -> Option<String>,
{
    fn render_rich_text(&self, payload: &RichTextPayload) -> Option<String> {
        self(payload)
    }
}

/// Extract and structurally validate the rich-text payload carried by a raw extension value.
///
/// Returns `Err` with an author-actionable reason for anything that is not a bounded
/// ProseMirror/Tiptap root document. Profiles (which node/mark types exist) are the host
/// capability's business; this gate only guarantees the shape is safe to walk.
pub fn decode_rich_text_payload(raw: &Value) -> Result<RichTextPayload, String> {
    let payload: RichTextPayload = serde_json::from_value(raw.clone())
        .map_err(|error| format!("rich text payload cannot be decoded: {error}"))?;
    validate_rich_text_payload(&payload)?;
    Ok(payload)
}

/// Structural validation of an already-decoded payload (see [`decode_rich_text_payload`]).
pub fn validate_rich_text_payload(payload: &RichTextPayload) -> Result<(), String> {
    let document = &payload.payload;
    let serialized_bytes = serde_json::to_vec(document).map_or(usize::MAX, |bytes| bytes.len());
    if serialized_bytes > MAXIMUM_RICH_TEXT_BYTES {
        return Err(format!(
            "rich text payload exceeds the {MAXIMUM_RICH_TEXT_BYTES} byte limit"
        ));
    }
    let root = document
        .as_object()
        .ok_or_else(|| "rich text payload must be a document object".to_string())?;
    if root.get("type").and_then(Value::as_str) != Some("doc") {
        return Err("rich text payload root must have type `doc`".to_string());
    }
    let mut nodes = 0_usize;
    validate_node(document, 0, &mut nodes)?;
    Ok(())
}

fn validate_node(node: &Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
    if depth > MAXIMUM_RICH_TEXT_DEPTH {
        return Err(format!(
            "rich text payload exceeds the maximum depth of {MAXIMUM_RICH_TEXT_DEPTH}"
        ));
    }
    *nodes += 1;
    if *nodes > MAXIMUM_RICH_TEXT_NODES {
        return Err(format!(
            "rich text payload exceeds the maximum of {MAXIMUM_RICH_TEXT_NODES} nodes"
        ));
    }
    let Some(object) = node.as_object() else {
        return Err("rich text nodes must be objects".to_string());
    };
    if object.get("type").and_then(Value::as_str).is_none() {
        return Err("rich text node is missing its `type`".to_string());
    }
    if let Some(text) = object.get("text") {
        if !text.is_string() {
            return Err("rich text `text` must be a string".to_string());
        }
    }
    if let Some(content) = object.get("content") {
        let children = content
            .as_array()
            .ok_or_else(|| "rich text `content` must be an array".to_string())?;
        for child in children {
            validate_node(child, depth + 1, nodes)?;
        }
    }
    Ok(())
}

/// Concatenated text of the payload, block-separated by newlines. Used for previews, search
/// projections, and accessibility names — anywhere the text matters and markup must not.
pub fn rich_text_plain_text(payload: &RichTextPayload) -> String {
    let mut blocks = Vec::new();
    if let Some(content) = payload.payload.get("content").and_then(Value::as_array) {
        for node in content {
            let mut text = String::new();
            collect_text(node, &mut text);
            let text = text.trim().to_string();
            if !text.is_empty() {
                blocks.push(text);
            }
        }
    }
    blocks.join("\n")
}

fn collect_text(node: &Value, output: &mut String) {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        output.push_str(text);
    }
    if let Some(content) = node.get("content").and_then(Value::as_array) {
        for child in content {
            collect_text(child, output);
        }
    }
}

/// Escaped, structure-preserving fallback projection (see the module docs). Only the block
/// types and marks listed here are projected; anything else degrades to its text content,
/// which is the safe direction for unknown node types.
pub fn rich_text_fallback_html(payload: &RichTextPayload) -> String {
    let mut html = String::new();
    if let Some(content) = payload.payload.get("content").and_then(Value::as_array) {
        for node in content {
            render_block(node, &mut html);
        }
    }
    html
}

fn render_block(node: &Value, html: &mut String) {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or_default();
    match kind {
        "paragraph" => wrap(html, "p", node),
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 6);
            wrap(html, &format!("h{level}"), node)
        }
        "blockquote" => wrap(html, "blockquote", node),
        "bulletList" => wrap(html, "ul", node),
        "orderedList" => wrap(html, "ol", node),
        "listItem" => wrap(html, "li", node),
        "codeBlock" => {
            html.push_str("<pre><code>");
            let mut text = String::new();
            collect_text(node, &mut text);
            push_escaped_html(html, &text);
            html.push_str("</code></pre>");
        }
        "horizontalRule" => html.push_str("<hr>"),
        _ => render_inline_children(node, html),
    }
}

fn wrap(html: &mut String, tag: &str, node: &Value) {
    html.push('<');
    html.push_str(tag);
    html.push('>');
    // Block containers may hold nested blocks (list items, blockquotes) or inline content.
    if let Some(content) = node.get("content").and_then(Value::as_array) {
        for child in content {
            let child_kind = child
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(
                child_kind,
                "paragraph"
                    | "heading"
                    | "blockquote"
                    | "bulletList"
                    | "orderedList"
                    | "listItem"
                    | "codeBlock"
            ) {
                render_block(child, html);
            } else {
                render_inline(child, html);
            }
        }
    }
    html.push_str("</");
    html.push_str(tag);
    html.push('>');
}

fn render_inline_children(node: &Value, html: &mut String) {
    if let Some(content) = node.get("content").and_then(Value::as_array) {
        for child in content {
            render_inline(child, html);
        }
    }
}

fn render_inline(node: &Value, html: &mut String) {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        let mut open = String::new();
        let mut close = String::new();
        if let Some(marks) = node.get("marks").and_then(Value::as_array) {
            for mark in marks {
                match mark.get("type").and_then(Value::as_str) {
                    Some("bold") => {
                        open.push_str("<strong>");
                        close.insert_str(0, "</strong>");
                    }
                    Some("italic") => {
                        open.push_str("<em>");
                        close.insert_str(0, "</em>");
                    }
                    Some("code") => {
                        open.push_str("<code>");
                        close.insert_str(0, "</code>");
                    }
                    Some("strike") => {
                        open.push_str("<s>");
                        close.insert_str(0, "</s>");
                    }
                    Some("link") => {
                        let href = mark
                            .get("attrs")
                            .and_then(|attrs| attrs.get("href"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if url_allowed(href, UrlAttributeKind::Navigation, &UrlPolicy::permissive())
                        {
                            open.push_str("<a href=\"");
                            push_escaped_attribute(&mut open, href);
                            open.push_str("\">");
                            close.insert_str(0, "</a>");
                        }
                    }
                    _ => {}
                }
            }
        }
        html.push_str(&open);
        push_escaped_html(html, text);
        html.push_str(&close);
        return;
    }
    match node.get("type").and_then(Value::as_str) {
        Some("hardBreak") => html.push_str("<br>"),
        _ => render_inline_children(node, html),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(document: Value) -> RichTextPayload {
        RichTextPayload::opaque(RICH_TEXT_CAPABILITY, document)
    }

    #[test]
    fn fallback_projection_escapes_text_and_filters_links() {
        let payload = payload(json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "5 < 10 & " },
                    {
                        "type": "text",
                        "text": "linked",
                        "marks": [{ "type": "link", "attrs": { "href": "javascript:alert(1)" } }]
                    },
                    {
                        "type": "text",
                        "text": "bold",
                        "marks": [{ "type": "bold" }]
                    }
                ]
            }]
        }));
        let html = rich_text_fallback_html(&payload);
        assert_eq!(
            html, "<p>5 &lt; 10 &amp; linked<strong>bold</strong></p>",
            "unsafe links degrade to text, text stays escaped: {html}"
        );
        assert_eq!(rich_text_plain_text(&payload), "5 < 10 & linkedbold");
    }

    #[test]
    fn structural_validation_rejects_non_documents_and_bombs() {
        assert!(
            decode_rich_text_payload(&json!({
                "capability": "richtext",
                "payload": { "type": "paragraph", "content": [] }
            }))
            .is_err()
        );
        assert!(
            decode_rich_text_payload(&json!({
                "capability": "richtext",
                "payload": { "type": "doc", "content": [{ "content": [] }] }
            }))
            .is_err()
        );
        let valid = payload(json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Ok" }] }]
        }));
        assert!(validate_rich_text_payload(&valid).is_ok());
    }
}
