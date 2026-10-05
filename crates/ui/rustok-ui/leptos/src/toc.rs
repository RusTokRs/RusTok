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
use rustok_ui::{
    TocItem, extract_headings_from_html, toc_header_classes, toc_item_classes,
    toc_list_classes, toc_nav_classes,
};

/// Universal Table of Contents component for Leptos applications.
///
/// Renders a sticky navigation block for long-form content across blog posts,
/// forum threads, long product descriptions, and documentation.
#[component]
pub fn TableOfContents(
    #[prop(default = Vec::new())] items: Vec<TocItem>,
    #[prop(default = String::new())] html: String,
    #[prop(optional)] locale: Option<String>,
    #[prop(optional)] title: Option<String>,
    #[prop(default = 2)] min_headings: usize,
    #[prop(default = true)] sticky: bool,
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

    let custom_cls = (!class.is_empty()).then_some(class.as_str());
    let nav_class = toc_nav_classes(sticky, custom_cls);
    let header_class = toc_header_classes();
    let list_class = toc_list_classes();
    let aria_title = effective_title.clone();

    view! {
        <nav
            role="navigation"
            aria-label=aria_title
            class=nav_class
        >
            <div class=header_class>
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
                class=list_class
            >
                {resolved_items
                    .into_iter()
                    .map(|item| {
                        let link_href = format!("#{}", item.id);
                        let item_class = toc_item_classes(item.level, false);

                        view! {
                            <li>
                                <a
                                    href=link_href
                                    class=item_class
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
