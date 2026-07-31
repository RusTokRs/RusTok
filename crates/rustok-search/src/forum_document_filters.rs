use uuid::Uuid;

use crate::SearchResultItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumStorefrontDocumentFilters {
    pub author_ids: Vec<Uuid>,
}

impl ForumStorefrontDocumentFilters {
    pub fn is_empty(&self) -> bool {
        self.author_ids.is_empty()
    }

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        if self.author_ids.is_empty() {
            return true;
        }
        if item.source_module != "forum"
            || !matches!(item.entity_type.as_str(), "forum_topic" | "forum_reply")
        {
            return false;
        }

        item.payload
            .get("author")
            .and_then(|author| author.get("user_id"))
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some_and(|author_id| self.author_ids.contains(&author_id))
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::ForumStorefrontDocumentFilters;
    use crate::SearchResultItem;

    fn item(entity_type: &str, author_id: Option<Uuid>) -> SearchResultItem {
        SearchResultItem {
            id: Uuid::new_v4(),
            entity_type: entity_type.to_string(),
            source_module: "forum".to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: Some("en".to_string()),
            payload: serde_json::json!({
                "author": author_id.map(|user_id| serde_json::json!({ "user_id": user_id }))
            }),
        }
    }

    #[test]
    fn empty_filter_preserves_all_items() {
        let filters = ForumStorefrontDocumentFilters::default();
        assert!(filters.matches(&item("forum_category", None)));
        assert!(filters.matches(&item("forum_topic", None)));
    }

    #[test]
    fn author_filter_matches_exact_public_topic_or_reply_author() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            author_ids: vec![expected],
        };

        assert!(filters.matches(&item("forum_topic", Some(expected))));
        assert!(filters.matches(&item("forum_reply", Some(expected))));
        assert!(!filters.matches(&item("forum_topic", Some(Uuid::new_v4()))));
        assert!(!filters.matches(&item("forum_category", None)));
        assert!(!filters.matches(&item("forum_reply", None)));
    }

    #[test]
    fn non_forum_items_never_match_active_author_filter() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            author_ids: vec![expected],
        };
        let mut product = item("product", Some(expected));
        product.source_module = "product".to_string();

        assert!(!filters.matches(&product));
    }
}
