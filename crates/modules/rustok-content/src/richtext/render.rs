/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::{RichTextDocument, RichTextMark, RichTextNode, RichTextView};
use url::Url;

use super::{RichTextError, RichTextProfile, validate_and_normalize};

pub fn render_html(
    document: &RichTextDocument,
    profile: RichTextProfile,
) -> Result<String, RichTextError> {
    let document = validate_and_normalize(document.clone(), profile)?;
    Ok(render_normalized(&document, profile))
}

pub fn project(
    document: &RichTextDocument,
    profile: RichTextProfile,
) -> Result<RichTextView, RichTextError> {
    let document = validate_and_normalize(document.clone(), profile)?;
    let html = render_normalized(&document, profile);
    Ok(RichTextView { document, html })
}

fn render_normalized(document: &RichTextDocument, profile: RichTextProfile) -> String {
    let mut html = String::new();
    for (index, node) in document.content.iter().enumerate() {
        if index > 0 {
            html.push('\n');
        }
        render_node(node, profile, &mut html);
    }
    html
}

fn render_node(node: &RichTextNode, profile: RichTextProfile, html: &mut String) {
    match node.kind.as_str() {
        "paragraph" => render_container("p", "richtext-paragraph", node, profile, html),
        "heading" => {
            let level = node.attrs["level"]
                .as_u64()
                .expect("validated heading level");
            let tag = format!("h{level}");
            let class = format!("richtext-heading richtext-heading-{level}");
            render_container(&tag, &class, node, profile, html);
        }
        "bulletList" => render_container(
            "ul",
            "richtext-list richtext-list-bullet",
            node,
            profile,
            html,
        ),
        "orderedList" => {
            html.push_str("<ol class=\"richtext-list richtext-list-ordered\"");
            if let Some(start) = node.attrs.get("start").and_then(|value| value.as_u64()) {
                html.push_str(" start=\"");
                html.push_str(&start.to_string());
                html.push('"');
            }
            html.push('>');
            render_children(node, profile, html);
            html.push_str("</ol>");
        }
        "listItem" => render_container("li", "richtext-list-item", node, profile, html),
        "blockquote" => render_container("blockquote", "richtext-blockquote", node, profile, html),
        "codeBlock" => {
            html.push_str("<pre class=\"richtext-code-block\"><code>");
            for child in &node.content {
                escape_text(child.text.as_deref().expect("validated code text"), html);
            }
            html.push_str("</code></pre>");
        }
        "horizontalRule" => html.push_str("<hr class=\"richtext-horizontal-rule\">"),
        "hardBreak" => render_inline("<br>", &node.marks, profile, html),
        "text" => {
            let mut escaped = String::new();
            escape_text(
                node.text.as_deref().expect("validated text node"),
                &mut escaped,
            );
            render_inline(&escaped, &node.marks, profile, html);
        }
        _ => unreachable!("renderer receives only validated nodes"),
    }
}

fn render_container(
    tag: &str,
    class: &str,
    node: &RichTextNode,
    profile: RichTextProfile,
    html: &mut String,
) {
    html.push('<');
    html.push_str(tag);
    html.push_str(" class=\"");
    html.push_str(class);
    html.push('"');
    html.push('>');
    render_children(node, profile, html);
    html.push_str("</");
    html.push_str(tag);
    html.push('>');
}

fn render_children(node: &RichTextNode, profile: RichTextProfile, html: &mut String) {
    for child in &node.content {
        render_node(child, profile, html);
    }
}

fn render_inline(value: &str, marks: &[RichTextMark], profile: RichTextProfile, html: &mut String) {
    for mark in marks {
        match mark.kind.as_str() {
            "link" => {
                let href = mark.attrs["href"].as_str().expect("validated link href");
                html.push_str("<a href=\"");
                escape_attribute(href, html);
                html.push('"');
                if is_external_link(href) {
                    html.push_str(" rel=\"");
                    html.push_str(profile.external_link_rel());
                    html.push('"');
                }
                html.push('>');
            }
            "bold" => html.push_str("<strong>"),
            "italic" => html.push_str("<em>"),
            "strike" => html.push_str("<s>"),
            "code" => html.push_str("<code>"),
            _ => unreachable!("renderer receives only validated marks"),
        }
    }

    html.push_str(value);

    for mark in marks.iter().rev() {
        match mark.kind.as_str() {
            "link" => html.push_str("</a>"),
            "bold" => html.push_str("</strong>"),
            "italic" => html.push_str("</em>"),
            "strike" => html.push_str("</s>"),
            "code" => html.push_str("</code>"),
            _ => unreachable!("renderer receives only validated marks"),
        }
    }
}

fn is_external_link(href: &str) -> bool {
    Url::parse(href)
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn escape_text(value: &str, output: &mut String) {
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

fn escape_attribute(value: &str, output: &mut String) {
    escape_text(value, output);
}
