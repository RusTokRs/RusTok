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
