/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use dioxus::prelude::*;
use rustok_ui::{
    TocItem, extract_headings_from_html, toc_header_classes, toc_item_classes, toc_list_classes,
    toc_nav_classes,
};

/// Universal Table of Contents component for Dioxus applications.
///
/// Renders a sticky navigation block for long-form content across blog posts,
/// forum threads, long product descriptions, and documentation.
#[component]
pub fn TableOfContents(
    #[props(default)] items: Vec<TocItem>,
    #[props(default)] html: Option<String>,
    #[props(default)] locale: Option<String>,
    #[props(default)] title: Option<String>,
    #[props(default = 2)] min_headings: usize,
    #[props(default)] sticky: Option<bool>,
    #[props(default)] class: Option<String>,
) -> Element {
    let resolved_items = if !items.is_empty() {
        items
    } else if let Some(ref content_html) = html {
        extract_headings_from_html(content_html)
    } else {
        Vec::new()
    };

    if resolved_items.len() < min_headings {
        return rsx! {};
    }

    let default_title = if locale.as_deref() == Some("ru") {
        "Содержание"
    } else {
        "Table of Contents"
    };
    let effective_title = title.as_deref().unwrap_or(default_title);

    let is_sticky = sticky.unwrap_or(true);
    let nav_class = toc_nav_classes(is_sticky, class.as_deref());
    let header_class = toc_header_classes();
    let list_class = toc_list_classes();

    rsx! {
        nav {
            role: "navigation",
            aria_label: "{effective_title}",
            class: "{nav_class}",
            div { class: "{header_class}",
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "16",
                    height: "16",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "h-4 w-4 text-primary",
                    line { x1: "21", x2: "3", y1: "6", y2: "6" }
                    line { x1: "15", x2: "3", y1: "12", y2: "12" }
                    line { x1: "17", x2: "3", y1: "18", y2: "18" }
                }
                span { "{effective_title}" }
            }
            ul {
                role: "list",
                class: "{list_class}",
                for item in resolved_items {
                    {
                        let link_href = format!("#{}", item.id);
                        let item_class = toc_item_classes(item.level, false);
                        rsx! {
                            li { key: "{item.id}",
                                a {
                                    href: "{link_href}",
                                    class: "{item_class}",
                                    "{item.text}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
