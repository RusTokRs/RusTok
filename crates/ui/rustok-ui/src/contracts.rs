/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Headless contracts, state models, and utilities for UI component lifecycles.
//!
//! These types carry no DOM or framework dependency, so a Leptos page, a
//! Dioxus desktop shell, and a server-side renderer can share the same option
//! lists, tab definitions, and dialog/tab state machines.

use serde::{Deserialize, Serialize};

/// Headless option definition for select controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    /// Value submitted when the option is selected.
    pub value: String,
    /// Human-readable label rendered inside the option.
    pub label: String,
    /// Whether the option is rendered but not selectable.
    pub disabled: bool,
}

impl SelectOption {
    /// Creates a selectable option.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Normalizes the maximum value for a progress indicator.
///
/// A finite, positive maximum is preserved. Invalid values fall back to `100`
/// so the component can keep its ARIA range and visual percentage well-defined.
#[must_use]
pub fn normalize_progress_max(max: f64) -> f64 {
    if max.is_finite() && max > 0.0 {
        max
    } else {
        100.0
    }
}

/// Normalizes a progress value against an explicit maximum.
///
/// Finite values are clamped to `0..=max`. Non-finite values, including `NaN`
/// and infinities, become `0` so the rendered width and accessible value remain
/// valid.
#[must_use]
pub fn normalize_progress_value_for_max(value: f64, max: f64) -> f64 {
    let max = normalize_progress_max(max);
    if !value.is_finite() || value <= 0.0 {
        0.0
    } else if value >= max {
        max
    } else {
        value
    }
}

/// Converts a normalized progress value to the percentage used by its visual bar.
#[must_use]
pub fn progress_value_percentage(value: f64, max: f64) -> f64 {
    let max = normalize_progress_max(max);
    normalize_progress_value_for_max(value, max) / max * 100.0
}

/// Normalizes a progress percentage for both its visual bar and ARIA value.
///
/// Finite values are clamped to the inclusive range `0..=100`. Non-finite
/// values, including `NaN` and infinities, become `0` so the rendered width
/// and accessible value always remain valid percentages.
#[must_use]
pub fn normalize_progress_value(value: f64) -> f64 {
    normalize_progress_value_for_max(value, 100.0)
}

/// Headless tab definition for tab bars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabItem {
    /// Stable identifier used for selection and `aria-controls` wiring.
    pub id: String,
    /// Human-readable label rendered inside the tab trigger.
    pub label: String,
    /// Whether the tab is rendered but not selectable.
    pub disabled: bool,
    /// Optional counter/status rendered next to the label.
    pub badge: Option<String>,
}

impl TabItem {
    /// Creates a selectable tab without a badge.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            badge: None,
        }
    }

    /// Attaches a badge (for example an unread counter) to the tab.
    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Marks the tab as disabled.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Headless state model for modal dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DialogState {
    /// Whether the dialog is currently visible.
    pub is_open: bool,
}

impl DialogState {
    /// Creates a dialog state with an explicit initial visibility.
    pub fn new(is_open: bool) -> Self {
        Self { is_open }
    }

    /// Opens the dialog.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Closes the dialog.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Inverts the visibility of the dialog.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}

/// Headless state model for tab selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabsState {
    /// Identifier of the currently selected tab.
    pub active_tab: String,
}

impl TabsState {
    /// Creates a tab state with `initial_tab` selected.
    pub fn new(initial_tab: impl Into<String>) -> Self {
        Self {
            active_tab: initial_tab.into(),
        }
    }

    /// Selects the tab with the given identifier.
    pub fn select(&mut self, tab_id: impl Into<String>) {
        self.active_tab = tab_id.into();
    }

    /// Returns `true` when `tab_id` is the selected tab.
    pub fn is_active(&self, tab_id: &str) -> bool {
        self.active_tab == tab_id
    }
}

/// Extracts up to two uppercase initial letters from a user name or fallback string.
///
/// The initial of a word is the first alphanumeric character of that word,
/// uppercased. Characters whose uppercase form expands into several characters
/// (for example `ß`) contribute the first character of that expansion, so the
/// result never exceeds two characters.
///
/// # Examples
/// ```rust
/// use rustok_ui::extract_initials;
///
/// assert_eq!(extract_initials("John Doe"), "JD");
/// assert_eq!(extract_initials("Alice"), "A");
/// assert_eq!(extract_initials("   Bob   Smith  "), "BS");
/// assert_eq!(extract_initials(""), "?");
/// assert_eq!(extract_initials("***"), "?");
/// ```
pub fn extract_initials(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()))
        .filter_map(|c| c.to_uppercase().next())
        .take(2)
        .collect();

    if initials.is_empty() {
        // Surface an explicit placeholder instead of an empty avatar label.
        "?".to_string()
    } else {
        initials
    }
}

/// Headless item representation for a Table of Contents entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TocItem {
    /// HTML anchor identifier (without `#`).
    pub id: String,
    /// Clean display text of the heading.
    pub text: String,
    /// Heading level (e.g. 2 for `<h2>`, 3 for `<h3>`, 4 for `<h4>`).
    pub level: u8,
}

impl TocItem {
    /// Creates a new Table of Contents item.
    pub fn new(id: impl Into<String>, text: impl Into<String>, level: u8) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            level,
        }
    }
}

/// Strips HTML tags from an inner string to yield clean heading text.
fn strip_html_tags(html: &str) -> String {
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
#[must_use]
pub fn slugify_heading(text: &str, index: usize) -> String {
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
#[must_use]
pub fn extract_headings_from_html(html: &str) -> Vec<TocItem> {
    let mut items = Vec::new();
    let lower_html = html.to_lowercase();
    let mut search_pos = 0;

    while search_pos < html.len() {
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
        let tag_prefix_len = 3;
        let after_tag_idx = tag_start + tag_prefix_len;
        if after_tag_idx >= html.len() {
            break;
        }
        let after_char = html.as_bytes()[after_tag_idx];
        if after_char != b' ' && after_char != b'>' && after_char != b'\t' && after_char != b'\n' && after_char != b'\r' {
            search_pos = tag_start + tag_prefix_len;
            continue;
        }

        let Some(open_tag_end_offset) = html[tag_start..].find('>') else {
            break;
        };
        let open_tag_end = tag_start + open_tag_end_offset;
        let open_tag = &html[tag_start..open_tag_end];

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

        let close_tag = if level == 2 { "</h2" } else { "</h3" };
        let Some(close_tag_offset) = lower_html[open_tag_end..].find(close_tag) else {
            search_pos = open_tag_end + 1;
            continue;
        };
        let close_tag_start = open_tag_end + close_tag_offset;
        let inner_html = &html[open_tag_end + 1..close_tag_start];
        let text = strip_html_tags(inner_html);

        if !text.is_empty() {
            let item_index = items.len();
            let effective_id = match id {
                Some(existing) => existing,
                None => {
                    let mut base = slugify_heading(&text, item_index);
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

        search_pos = close_tag_start + 4;
    }

    items
}

