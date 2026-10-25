/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Request/response, command/query, and data transfer objects.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecuritySummary {
    pub slug: String,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_serde_roundtrip() {
        let summary = SecuritySummary {
            slug: "security".into(),
            title: "Security".into(),
        };
        let json = serde_json::to_string(&summary).expect("serialize summary");
        let decoded: SecuritySummary = serde_json::from_str(&json).expect("deserialize summary");
        assert_eq!(summary.slug, decoded.slug);
        assert_eq!(summary.title, decoded.title);
    }
}
