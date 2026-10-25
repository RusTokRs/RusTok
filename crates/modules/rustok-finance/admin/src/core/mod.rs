/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models, search filters, and state machines for Operational Finance & Cash Desks admin UI.
//!
//! INVARIANT (FFA): This module MUST NOT import any UI framework types.
//! All validation, search filtering, and state transition logic must be testable with pure #[test].

use serde::{Deserialize, Serialize};

/// Table row representation for Operational Finance & Cash Desks admin DataGrid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceAdminTableRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub can_edit: bool,
    pub can_delete: bool,
}

impl FinanceAdminTableRow {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        status: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            created_at: created_at.into(),
            can_edit: true,
            can_delete: true,
        }
    }
}

/// Pure in-memory search filter for Operational Finance & Cash Desks admin records.
pub fn filter_rows(
    rows: &[FinanceAdminTableRow],
    query: Option<&str>,
) -> Vec<FinanceAdminTableRow> {
    match query {
        Some(q) if !q.trim().is_empty() => {
            let normalized = q.trim().to_lowercase();
            rows.iter()
                .filter(|r| {
                    r.title.to_lowercase().contains(&normalized)
                        || r.status.to_lowercase().contains(&normalized)
                })
                .cloned()
                .collect()
        }
        _ => rows.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_rows_matches_title_and_status() {
        let rows = vec![
            FinanceAdminTableRow::new("1", "Production Workspace", "active", "2026-01-01"),
            FinanceAdminTableRow::new("2", "Archived Channel", "archived", "2026-01-02"),
        ];

        let filtered = filter_rows(&rows, Some("production"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "1");

        let status_filtered = filter_rows(&rows, Some("archived"));
        assert_eq!(status_filtered.len(), 1);
        assert_eq!(status_filtered[0].id, "2");

        let all = filter_rows(&rows, None);
        assert_eq!(all.len(), 2);
    }
}
