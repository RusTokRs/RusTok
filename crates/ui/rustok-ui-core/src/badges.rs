/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize};

/// Visual semantic tones for UI status badges and indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiBadgeTone {
    Success,
    Warning,
    Danger,
    Info,
    Neutral,
    Muted,
}

impl UiBadgeTone {
    /// Returns the standard class combination for this badge tone.
    pub fn container_class(self) -> &'static str {
        ui_badge_container_class(self)
    }
}

/// Returns a finite, build-time-visible Tailwind utility class combination
/// representing the given [`UiBadgeTone`].
pub fn ui_badge_container_class(tone: UiBadgeTone) -> &'static str {
    match tone {
        UiBadgeTone::Success => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-800"
        }
        UiBadgeTone::Warning => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-800"
        }
        UiBadgeTone::Danger => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-950/40 dark:text-rose-300 dark:border-rose-800"
        }
        UiBadgeTone::Info => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-sky-50 text-sky-700 border-sky-200 dark:bg-sky-950/40 dark:text-sky-300 dark:border-sky-800"
        }
        UiBadgeTone::Neutral => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-slate-50 text-slate-700 border-slate-200 dark:bg-slate-900 dark:text-slate-300 dark:border-slate-800"
        }
        UiBadgeTone::Muted => {
            "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold bg-muted text-muted-foreground border-border"
        }
    }
}

/// Resolves a domain status string into a canonical [`UiBadgeTone`].
///
/// Handles common domain statuses across RusToK modules in a case-insensitive,
/// whitespace-tolerant manner without requiring module-local duplicate matches.
pub fn status_badge_tone(status: &str) -> UiBadgeTone {
    let normalized = status.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "active" | "published" | "paid" | "completed" | "delivered" | "success"
        | "approved" | "available" | "healthy" | "in_stock" => UiBadgeTone::Success,

        "draft" | "pending" | "processing" | "low_stock" | "warning" | "review"
        | "in_progress" => UiBadgeTone::Warning,

        "archived" | "inactive" | "disabled" | "neutral" => UiBadgeTone::Neutral,

        "failed" | "error" | "rejected" | "cancelled" | "canceled" | "danger"
        | "out_of_stock" | "spam" => UiBadgeTone::Danger,

        "info" | "shipped" | "refunded" | "replaying" => UiBadgeTone::Info,

        _ => UiBadgeTone::Muted,
    }
}

/// Resolves a domain status string directly to a canonical Tailwind class string.
pub fn status_badge_class(status: &str) -> &'static str {
    ui_badge_container_class(status_badge_tone(status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_badge_tone_mapping() {
        assert_eq!(status_badge_tone("ACTIVE"), UiBadgeTone::Success);
        assert_eq!(status_badge_tone("published "), UiBadgeTone::Success);
        assert_eq!(status_badge_tone("Paid"), UiBadgeTone::Success);

        assert_eq!(status_badge_tone("draft"), UiBadgeTone::Warning);
        assert_eq!(status_badge_tone("PENDING"), UiBadgeTone::Warning);

        assert_eq!(status_badge_tone("ARCHIVED"), UiBadgeTone::Neutral);
        assert_eq!(status_badge_tone("inactive"), UiBadgeTone::Neutral);

        assert_eq!(status_badge_tone("FAILED"), UiBadgeTone::Danger);
        assert_eq!(status_badge_tone("cancelled"), UiBadgeTone::Danger);

        assert_eq!(status_badge_tone("unknown_custom_value"), UiBadgeTone::Muted);
    }

    #[test]
    fn test_ui_badge_container_class() {
        let success_class = status_badge_class("ACTIVE");
        assert!(success_class.contains("emerald"));
        assert!(success_class.contains("rounded-full"));

        let warning_class = status_badge_class("DRAFT");
        assert!(warning_class.contains("amber"));

        let danger_class = status_badge_class("FAILED");
        assert!(danger_class.contains("rose"));

        let neutral_class = status_badge_class("ARCHIVED");
        assert!(neutral_class.contains("slate"));
    }
}
