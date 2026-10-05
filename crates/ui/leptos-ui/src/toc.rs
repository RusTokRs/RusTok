/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::*;

/// Representation of a single heading in the table of contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TocItem {
    pub id: String,
    pub text: String,
    pub level: u8,
}

impl TocItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>, level: u8) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            level,
        }
    }
}

/// Strips HTML tags from an inner string to yield clean heading text.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    // Decode common HTML entities
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

/// Generates a clean URL slug from heading text.
fn slugify(text: &str, index: usize) -> String {
    let mut slug = String::new();
    let mut last_dash = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }

    if slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        format!("section-{}", index + 1)
    } else {
        slug
    }
}

/// Parses an HTML fragment and extracts all `<h2>` and `<h3>` headings.
///
/// If a heading already has an `id="..."` attribute, that id is preserved.
/// Otherwise, a deterministic slug is derived from the heading text.
pub fn extract_headings_from_html(html: &str) -> Vec<TocItem> {
    let mut items = Vec::new();
    let lower_html = html.to_lowercase();
    let mut search_pos = 0;

    while search_pos < html.len() {
        // Find next <h2 or <h3
        let next_h2 = lower_html[search_pos..].find("<h2");
        let next_h3 = lower_html[search_pos..].find("<h3");

        let (tag_offset, level) = match (next_h2, next_h3) {
            (Some(o2), Some(o3)) => {
                if o2 <= o3 {
                    (o2, 2)
                } else {
                    (o3, 3)
                }
            }
            (Some(o2), None) => (o2, 2),
            (None, Some(o3)) => (o3, 3),
            (None, None) => break,
        };

        let tag_start = search_pos + tag_offset;
        // Verify it's a tag boundary: char after <h2/<h3 must be whitespace or '>'
        let tag_prefix_len = 3; // "<h2" or "<h3"
        let after_tag_idx = tag_start + tag_prefix_len;
        if after_tag_idx >= html.len() {
            break;
        }
        let after_char = html.as_bytes()[after_tag_idx];
        if after_char != b' ' && after_char != b'>' && after_char != b'\t' && after_char != b'\n' && after_char != b'\r' {
            search_pos = tag_start + tag_prefix_len;
            continue;
        }

        // Find end of opening tag '>'
        let Some(open_tag_end_offset) = html[tag_start..].find('>') else {
            break;
        };
        let open_tag_end = tag_start + open_tag_end_offset;
        let open_tag = &html[tag_start..open_tag_end];

        // Extract id attribute from opening tag if present: id="..." or id='...'
        let mut id = None;
        let lower_open_tag = open_tag.to_lowercase();
        if let Some(id_idx) = lower_open_tag.find("id=") {
            let after_id = &open_tag[id_idx + 3..].trim_start();
            if let Some(first_char) = after_id.chars().next() {
                if first_char == '"' || first_char == '\'' {
                    let rest = &after_id[first_char.len_utf8()..];
                    if let Some(close_quote) = rest.find(first_char) {
                        let parsed_id = rest[..close_quote].trim();
                        if !parsed_id.is_empty() {
                            id = Some(parsed_id.to_string());
                        }
                    }
                } else {
                    // Unquoted id attribute
                    let end_token = after_id
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(after_id.len());
                    let parsed_id = after_id[..end_token].trim();
                    if !parsed_id.is_empty() {
                        id = Some(parsed_id.to_string());
                    }
                }
            }
        }

        // Find closing tag `</h2` or `</h3`
        let close_tag = if level == 2 { "</h2" } else { "</h3" };
        let Some(close_tag_offset) = lower_html[open_tag_end..].find(close_tag) else {
            search_pos = open_tag_end + 1;
            continue;
        };
        let close_tag_start = open_tag_end + close_tag_offset;
        let inner_html = &html[open_tag_end + 1..close_tag_start];
        let text = strip_tags(inner_html);

        if !text.is_empty() {
            let item_index = items.len();
            let effective_id = match id {
                Some(existing) => existing,
                None => {
                    let mut base = slugify(&text, item_index);
                    if items.iter().any(|it: &TocItem| it.id == base) {
                        base = format!("{base}-{}", item_index + 1);
                    }
                    base
                }
            };

            items.push(TocItem {
                id: effective_id,
                text,
                level,
            });
        }

        search_pos = close_tag_start + 4; // advance past closing tag
    }

    items
}

/// Universal Table of Contents component for Leptos.
///
/// Renders a sticky navigation block for long-form content:
/// - Blog posts
/// - Forum threads
/// - Long product descriptions
/// - Documentation / Knowledge base
///
/// Can either receive pre-extracted `items: Vec<TocItem>`, or parse headings
/// directly from the provided `html: String` on both SSR and client.
#[component]
pub fn TableOfContents(
    #[prop(default = Vec::new())] items: Vec<TocItem>,
    #[prop(default = String::new())] html: String,
    #[prop(optional)] locale: Option<String>,
    #[prop(optional)] title: Option<String>,
    #[prop(default = 2)] min_headings: usize,
    #[prop(default = String::new())] class: String,
) -> impl IntoView {
    let resolved_items = if !items.is_empty() {
        items
    } else if !html.is_empty() {
        extract_headings_from_html(&html)
    } else {
        Vec::new()
    };

    if resolved_items.len() < min_headings {
        return ().into_any();
    }

    let default_title = if locale.as_deref() == Some("ru") {
        "Содержание".to_string()
    } else {
        "Table of Contents".to_string()
    };
    let effective_title = title.unwrap_or(default_title);

    let base_class = "sticky top-24 rounded-2xl border border-border bg-card p-5 shadow-sm space-y-3";
    let combined_class = if class.is_empty() {
        base_class.to_string()
    } else {
        format!("{base_class} {class}")
    };

    let aria_title = effective_title.clone();

    view! {
        <nav
            aria-label=aria_title
            class=combined_class
        >
            <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground border-b border-border pb-3">
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class="h-4 w-4 text-primary"
                >
                    <line x1="21" x2="3" y1="6" y2="6"></line>
                    <line x1="15" x2="3" y1="12" y2="12"></line>
                    <line x1="17" x2="3" y1="18" y2="18"></line>
                </svg>
                <span>{effective_title}</span>
            </div>
            <ul
                role="list"
                class="space-y-1.5 text-sm max-h-[calc(100vh-12rem)] overflow-y-auto pr-1"
            >
                {resolved_items
                    .into_iter()
                    .map(|item| {
                        let link_href = format!("#{}", item.id);
                        let padding_class = if item.level == 3 {
                            "pl-3"
                        } else if item.level == 4 {
                            "pl-6"
                        } else {
                            ""
                        };

                        view! {
                            <li class=padding_class>
                                <a
                                    href=link_href
                                    class="block py-1 text-xs transition-colors rounded-md px-2 text-muted-foreground hover:bg-muted hover:text-foreground"
                                >
                                    {item.text}
                                </a>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </nav>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_headings_with_existing_ids() {
        let html = r#"
            <div>
                <h2 id="intro">Introduction</h2>
                <p>Some text</p>
                <h3 id="details">Details &amp; Setup</h3>
                <p>More text</p>
                <h2>Conclusion</h2>
            </div>
        "#;

        let headings = extract_headings_from_html(html);
        assert_eq!(headings.len(), 3);

        assert_eq!(headings[0].id, "intro");
        assert_eq!(headings[0].text, "Introduction");
        assert_eq!(headings[0].level, 2);

        assert_eq!(headings[1].id, "details");
        assert_eq!(headings[1].text, "Details & Setup");
        assert_eq!(headings[1].level, 3);

        assert_eq!(headings[2].id, "conclusion");
        assert_eq!(headings[2].text, "Conclusion");
        assert_eq!(headings[2].level, 2);
    }

    #[test]
    fn test_extract_headings_strips_nested_tags() {
        let html = r#"
            <h2><span>Heading</span> with <code>Code</code></h2>
            <h3>Another <strong>Bold</strong> Heading</h3>
        "#;

        let headings = extract_headings_from_html(html);
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "Heading with Code");
        assert_eq!(headings[1].text, "Another Bold Heading");
    }

    #[test]
    fn test_extract_headings_duplicate_slug_disambiguation() {
        let html = r#"
            <h2>Summary</h2>
            <h2>Summary</h2>
        "#;

        let headings = extract_headings_from_html(html);
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].id, "summary");
        assert_eq!(headings[1].id, "summary-2");
    }
}
