use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::SearchResultItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumStorefrontDocumentFilters {
    pub exact_locale: Option<String>,
    pub author_ids: Vec<Uuid>,
    pub tags: Vec<String>,
    pub solved: Option<bool>,
    pub published_from: Option<DateTime<Utc>>,
    pub published_to: Option<DateTime<Utc>>,
}

impl ForumStorefrontDocumentFilters {
    pub fn is_empty(&self) -> bool {
        self.author_ids.is_empty()
            && self.tags.is_empty()
            && self.solved.is_none()
            && self.published_from.is_none()
            && self.published_to.is_none()
    }

    pub fn has_date_window(&self) -> bool {
        self.published_from.is_some() || self.published_to.is_some()
    }

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        if let Some(exact_locale) = self.exact_locale.as_deref()
            && (item.source_module != "forum"
                || !item
                    .locale
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some_and(|value| value.eq_ignore_ascii_case(exact_locale)))
        {
            return false;
        }

        if self.is_empty() {
            return true;
        }
        if item.source_module != "forum"
            || !matches!(item.entity_type.as_str(), "forum_topic" | "forum_reply")
        {
            return false;
        }

        self.matches_author(item)
            && self.matches_tags(item)
            && self.matches_solved(item)
            && self.matches_published_at(item)
    }

    fn matches_author(&self, item: &SearchResultItem) -> bool {
        if self.author_ids.is_empty() {
            return true;
        }

        item.payload
            .get("author")
            .and_then(|author| author.get("user_id"))
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some_and(|author_id| self.author_ids.contains(&author_id))
    }

    fn matches_tags(&self, item: &SearchResultItem) -> bool {
        if self.tags.is_empty() {
            return true;
        }

        let payload_key = match item.entity_type.as_str() {
            "forum_topic" => "tags",
            "forum_reply" => "topic_tags",
            _ => return false,
        };
        let Some(projected_tags) = item
            .payload
            .get(payload_key)
            .and_then(serde_json::Value::as_array)
        else {
            return false;
        };
        let Some(projected_tags) = projected_tags
            .iter()
            .map(serde_json::Value::as_str)
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };

        self.tags
            .iter()
            .all(|expected| projected_tags.contains(&expected.as_str()))
    }

    fn matches_solved(&self, item: &SearchResultItem) -> bool {
        let Some(expected) = self.solved else {
            return true;
        };

        match item.entity_type.as_str() {
            "forum_topic" => {
                let projected = match item.payload.get("solution_reply_id") {
                    Some(serde_json::Value::Null) => Some(false),
                    Some(serde_json::Value::String(value)) => {
                        Uuid::parse_str(value).ok().map(|_| true)
                    }
                    _ => None,
                };
                projected == Some(expected)
            }
            "forum_reply" => {
                item.payload
                    .get("is_solution")
                    .and_then(serde_json::Value::as_bool)
                    == Some(expected)
            }
            _ => false,
        }
    }

    fn matches_published_at(&self, item: &SearchResultItem) -> bool {
        if !self.has_date_window() {
            return true;
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
            .is_none_or(|from| &published_at >= from)
            && self
                .published_to
                .as_ref()
                .is_none_or(|to| &published_at <= to)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::ForumStorefrontDocumentFilters;
    use crate::SearchResultItem;

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn item(
        entity_type: &str,
        author_id: Option<Uuid>,
        tags: Option<Vec<&str>>,
        solved: Option<bool>,
    ) -> SearchResultItem {
        let mut payload = serde_json::json!({
            "author": author_id.map(|user_id| serde_json::json!({ "user_id": user_id }))
        });
        match entity_type {
            "forum_topic" => {
                payload["tags"] = tags
                    .map(|values| serde_json::json!(values))
                    .unwrap_or(serde_json::Value::Null);
                payload["solution_reply_id"] = match solved {
                    Some(true) => serde_json::json!(Uuid::new_v4()),
                    Some(false) => serde_json::Value::Null,
                    None => serde_json::Value::Null,
                };
            }
            "forum_reply" => {
                payload["topic_tags"] = tags
                    .map(|values| serde_json::json!(values))
                    .unwrap_or(serde_json::Value::Null);
                payload["is_solution"] = solved
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null);
            }
            _ => {}
        }
        SearchResultItem {
            id: Uuid::new_v4(),
            entity_type: entity_type.to_string(),
            source_module: "forum".to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: Some("en".to_string()),
            payload,
        }
    }

    #[test]
    fn empty_filter_preserves_all_items() {
        let filters = ForumStorefrontDocumentFilters::default();
        assert!(filters.matches(&item("forum_category", None, None, None)));
        assert!(filters.matches(&item("forum_topic", None, None, None)));
    }

    #[test]
    fn exact_locale_preserves_categories_and_fails_closed_on_missing_locale() {
        let filters = ForumStorefrontDocumentFilters {
            exact_locale: Some("en".to_string()),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut category = item("forum_category", None, None, None);
        category.locale = Some("EN".to_string());
        let mut foreign = item("forum_topic", None, None, None);
        foreign.locale = Some("de".to_string());
        let mut missing = item("forum_topic", None, None, None);
        missing.locale = None;

        assert!(filters.matches(&category));
        assert!(!filters.matches(&foreign));
        assert!(!filters.matches(&missing));
    }

    #[test]
    fn author_filter_matches_exact_public_topic_or_reply_author() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            author_ids: vec![expected],
            ..ForumStorefrontDocumentFilters::default()
        };

        assert!(filters.matches(&item("forum_topic", Some(expected), None, None)));
        assert!(filters.matches(&item("forum_reply", Some(expected), None, None)));
        assert!(!filters.matches(&item("forum_topic", Some(Uuid::new_v4()), None, None)));
        assert!(!filters.matches(&item("forum_category", None, None, None)));
        assert!(!filters.matches(&item("forum_reply", None, None, None)));
    }

    #[test]
    fn non_forum_items_never_match_active_author_filter() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            author_ids: vec![expected],
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut product = item("forum_topic", Some(expected), None, None);
        product.entity_type = "product".to_string();
        product.source_module = "product".to_string();

        assert!(!filters.matches(&product));
    }

    #[test]
    fn tag_filter_requires_every_exact_topic_tag() {
        let filters = ForumStorefrontDocumentFilters {
            tags: vec!["Rust".to_string(), "Search".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };

        assert!(filters.matches(&item(
            "forum_topic",
            None,
            Some(vec!["Search", "Rust", "Forum"]),
            None
        )));
        assert!(filters.matches(&item(
            "forum_reply",
            None,
            Some(vec!["Rust", "Search"]),
            Some(false)
        )));
        assert!(!filters.matches(&item(
            "forum_topic",
            None,
            Some(vec!["rust", "Search"]),
            None
        )));
        assert!(!filters.matches(&item("forum_reply", None, Some(vec!["Rust"]), Some(false))));
        assert!(!filters.matches(&item("forum_reply", None, None, Some(false))));
    }

    #[test]
    fn solved_filter_uses_topic_solution_and_exact_reply_marker() {
        let solved = ForumStorefrontDocumentFilters {
            solved: Some(true),
            ..ForumStorefrontDocumentFilters::default()
        };
        let unsolved = ForumStorefrontDocumentFilters {
            solved: Some(false),
            ..ForumStorefrontDocumentFilters::default()
        };

        assert!(solved.matches(&item("forum_topic", None, None, Some(true))));
        assert!(solved.matches(&item("forum_reply", None, None, Some(true))));
        assert!(!solved.matches(&item("forum_topic", None, None, Some(false))));
        assert!(!solved.matches(&item("forum_reply", None, None, Some(false))));
        assert!(unsolved.matches(&item("forum_topic", None, None, Some(false))));
        assert!(unsolved.matches(&item("forum_reply", None, None, Some(false))));
        assert!(!unsolved.matches(&item("forum_reply", None, None, None)));
    }

    #[test]
    fn published_window_is_inclusive_and_excludes_categories() {
        let filters = ForumStorefrontDocumentFilters {
            exact_locale: Some("en".to_string()),
            published_from: Some(timestamp("2026-07-15T12:00:00Z")),
            published_to: Some(timestamp("2026-07-15T12:00:00Z")),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut matching = item("forum_topic", None, None, None);
        matching.payload["published_at"] = serde_json::json!("2026-07-15T12:00:00Z");
        let mut after = item("forum_reply", None, None, None);
        after.payload["published_at"] = serde_json::json!("2026-07-15T12:00:01Z");
        let category = item("forum_category", None, None, None);

        assert!(filters.matches(&matching));
        assert!(!filters.matches(&after));
        assert!(!filters.matches(&category));
    }

    #[test]
    fn malformed_or_missing_published_projection_fails_closed() {
        let filters = ForumStorefrontDocumentFilters {
            published_from: Some(timestamp("2026-07-01T00:00:00Z")),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut malformed = item("forum_topic", None, None, None);
        malformed.payload["published_at"] = serde_json::json!("not-a-date");
        let missing = item("forum_reply", None, None, None);

        assert!(!filters.matches(&malformed));
        assert!(!filters.matches(&missing));
    }

    #[test]
    fn malformed_tag_or_solved_projection_fails_closed() {
        let tag_filter = ForumStorefrontDocumentFilters {
            tags: vec!["Rust".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };
        let solved_filter = ForumStorefrontDocumentFilters {
            solved: Some(true),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut malformed_tags = item("forum_topic", None, Some(vec!["Rust"]), None);
        malformed_tags.payload["tags"] = serde_json::json!(["Rust", 7]);
        let mut malformed_solution = item("forum_topic", None, None, Some(true));
        malformed_solution.payload["solution_reply_id"] = serde_json::json!(true);

        assert!(!tag_filter.matches(&malformed_tags));
        assert!(!solved_filter.matches(&malformed_solution));
    }

    #[test]
    fn active_filters_intersect_and_exclude_non_forum_items() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            author_ids: vec![expected],
            tags: vec!["Rust".to_string()],
            solved: Some(true),
            ..ForumStorefrontDocumentFilters::default()
        };
        let matching = item(
            "forum_reply",
            Some(expected),
            Some(vec!["Rust"]),
            Some(true),
        );
        let mut product = matching.clone();
        product.entity_type = "product".to_string();
        product.source_module = "product".to_string();

        assert!(filters.matches(&matching));
        assert!(!filters.matches(&product));
    }
}
