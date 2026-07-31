use chrono::{DateTime, Utc};

use crate::SearchResultItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontLocaleDateFilters {
    pub exact_locale: String,
    pub published_from: Option<DateTime<Utc>>,
    pub published_to: Option<DateTime<Utc>>,
}

impl ForumStorefrontLocaleDateFilters {
    pub fn has_date_window(&self) -> bool {
        self.published_from.is_some() || self.published_to.is_some()
    }

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        if item.source_module != "forum"
            || !item
                .locale
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some_and(|value| value.eq_ignore_ascii_case(&self.exact_locale))
        {
            return false;
        }
        if !self.has_date_window() {
            return true;
        }
        if !matches!(item.entity_type.as_str(), "forum_topic" | "forum_reply") {
            return false;
        }
        let Some(published_at) = item
            .payload
            .get("published_at")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        else {
            return false;
        };

        self.published_from
            .as_ref()
            .is_none_or(|from| published_at >= from.clone())
            && self
                .published_to
                .as_ref()
                .is_none_or(|to| published_at <= to.clone())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::ForumStorefrontLocaleDateFilters;
    use crate::SearchResultItem;

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn item(
        entity_type: &str,
        locale: Option<&str>,
        published_at: serde_json::Value,
    ) -> SearchResultItem {
        SearchResultItem {
            id: Uuid::new_v4(),
            entity_type: entity_type.to_string(),
            source_module: "forum".to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: locale.map(str::to_string),
            payload: serde_json::json!({ "published_at": published_at }),
        }
    }

    #[test]
    fn exact_locale_preserves_categories_without_date_window() {
        let filters = ForumStorefrontLocaleDateFilters {
            exact_locale: "en".to_string(),
            published_from: None,
            published_to: None,
        };
        assert!(filters.matches(&item("forum_category", Some("EN"), serde_json::Value::Null)));
        assert!(!filters.matches(&item("forum_topic", Some("de"), serde_json::Value::Null)));
        assert!(!filters.matches(&item("forum_topic", None, serde_json::Value::Null)));
    }

    #[test]
    fn published_window_is_inclusive_and_excludes_categories() {
        let filters = ForumStorefrontLocaleDateFilters {
            exact_locale: "en".to_string(),
            published_from: Some(timestamp("2026-07-15T12:00:00Z")),
            published_to: Some(timestamp("2026-07-15T12:00:00Z")),
        };
        assert!(filters.matches(&item(
            "forum_topic",
            Some("en"),
            serde_json::json!("2026-07-15T12:00:00Z")
        )));
        assert!(!filters.matches(&item(
            "forum_reply",
            Some("en"),
            serde_json::json!("2026-07-15T12:00:01Z")
        )));
        assert!(!filters.matches(&item("forum_category", Some("en"), serde_json::Value::Null)));
    }

    #[test]
    fn malformed_or_missing_projection_fails_closed() {
        let filters = ForumStorefrontLocaleDateFilters {
            exact_locale: "en".to_string(),
            published_from: Some(timestamp("2026-07-01T00:00:00Z")),
            published_to: None,
        };
        assert!(!filters.matches(&item(
            "forum_topic",
            Some("en"),
            serde_json::json!("not-a-date")
        )));
        assert!(!filters.matches(&item("forum_reply", Some("en"), serde_json::Value::Null)));
    }
}
