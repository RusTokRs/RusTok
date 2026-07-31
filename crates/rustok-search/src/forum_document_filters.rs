use uuid::Uuid;

use crate::SearchResultItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumStorefrontDocumentFilters {
    pub author_ids: Vec<Uuid>,
    pub tags: Vec<String>,
    pub solved: Option<bool>,
}

impl ForumStorefrontDocumentFilters {
    pub fn is_empty(&self) -> bool {
        self.author_ids.is_empty() && self.tags.is_empty() && self.solved.is_none()
    }

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        if self.is_empty() {
            return true;
        }
        if item.source_module != "forum"
            || !matches!(item.entity_type.as_str(), "forum_topic" | "forum_reply")
        {
            return false;
        }

        self.matches_author(item) && self.matches_tags(item) && self.matches_solved(item)
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
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::ForumStorefrontDocumentFilters;
    use crate::SearchResultItem;

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
