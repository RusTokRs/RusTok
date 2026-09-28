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

use serde::{Deserialize, Serialize};

/// Headless option definition for select controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Headless tab definition for tab bars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabItem {
    pub id: String,
    pub label: String,
    pub disabled: bool,
    pub badge: Option<String>,
}

impl TabItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            badge: None,
        }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// Headless state model for modal dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DialogState {
    pub is_open: bool,
}

impl DialogState {
    pub fn new(is_open: bool) -> Self {
        Self { is_open }
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}

/// Headless state model for tab selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabsState {
    pub active_tab: String,
}

impl TabsState {
    pub fn new(initial_tab: impl Into<String>) -> Self {
        Self {
            active_tab: initial_tab.into(),
        }
    }

    pub fn select(&mut self, tab_id: impl Into<String>) {
        self.active_tab = tab_id.into();
    }

    pub fn is_active(&self, tab_id: &str) -> bool {
        self.active_tab == tab_id
    }
}

/// Extracts up to two uppercase initial letters from a user name or fallback string.
///
/// # Examples
/// ```rust
/// use rustok_ui::contracts::extract_initials;
/// assert_eq!(extract_initials("John Doe"), "JD");
/// assert_eq!(extract_initials("Alice"), "A");
/// assert_eq!(extract_initials(""), "?");
/// ```
pub fn extract_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "?".to_string();
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() {
        return "?".to_string();
    }

    if words.len() == 1 {
        words[0].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string())
    } else {
        let first = words[0].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        let second = words[1].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        format!("{first}{second}")
    }
}
