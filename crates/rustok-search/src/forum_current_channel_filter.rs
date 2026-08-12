use crate::SearchResultItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ForumStorefrontCurrentChannelFilter {
    pub channel_slug: Option<String>,
}

impl ForumStorefrontCurrentChannelFilter {
    pub fn is_empty(&self) -> bool {
        self.channel_slug.is_none()
    }

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        let Some(expected_channel) = self.channel_slug.as_deref() else {
            return true;
        };
        if item.source_module != "forum"
            || !matches!(item.entity_type.as_str(), "forum_topic" | "forum_reply")
        {
            return false;
        }

        let payload_key = match item.entity_type.as_str() {
            "forum_topic" => "channel_slugs",
            "forum_reply" => "topic_channel_slugs",
            _ => return false,
        };
        let Some(projected_channels) = item
            .payload
            .get(payload_key)
            .and_then(serde_json::Value::as_array)
        else {
            return false;
        };
        let Some(projected_channels) = projected_channels
            .iter()
            .map(serde_json::Value::as_str)
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };

        projected_channels.contains(&expected_channel)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::ForumStorefrontCurrentChannelFilter;
    use crate::SearchResultItem;

    fn item(entity_type: &str, payload_key: &str, channels: serde_json::Value) -> SearchResultItem {
        SearchResultItem {
            id: Uuid::new_v4(),
            entity_type: entity_type.to_string(),
            source_module: "forum".to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: Some("en".to_string()),
            payload: serde_json::json!({ (payload_key): channels }),
        }
    }

    #[test]
    fn inactive_filter_preserves_categories_topics_and_replies() {
        let filter = ForumStorefrontCurrentChannelFilter::default();
        assert!(filter.matches(&item(
            "forum_category",
            "channel_slugs",
            serde_json::json!([])
        )));
        assert!(filter.matches(&item("forum_topic", "channel_slugs", serde_json::json!([]))));
        assert!(filter.matches(&item(
            "forum_reply",
            "topic_channel_slugs",
            serde_json::json!([])
        )));
    }

    #[test]
    fn exact_current_channel_matches_topics_and_parent_scoped_replies() {
        let filter = ForumStorefrontCurrentChannelFilter {
            channel_slug: Some("web".to_string()),
        };
        assert!(filter.matches(&item(
            "forum_topic",
            "channel_slugs",
            serde_json::json!(["mobile", "web"])
        )));
        assert!(filter.matches(&item(
            "forum_reply",
            "topic_channel_slugs",
            serde_json::json!(["web"])
        )));
        assert!(!filter.matches(&item(
            "forum_topic",
            "channel_slugs",
            serde_json::json!(["mobile"])
        )));
        assert!(!filter.matches(&item("forum_topic", "channel_slugs", serde_json::json!([]))));
        assert!(!filter.matches(&item(
            "forum_category",
            "channel_slugs",
            serde_json::json!(["web"])
        )));
    }

    #[test]
    fn missing_or_malformed_projection_fails_closed() {
        let filter = ForumStorefrontCurrentChannelFilter {
            channel_slug: Some("web".to_string()),
        };
        assert!(!filter.matches(&item("forum_reply", "other", serde_json::json!(["web"]))));
        assert!(!filter.matches(&item(
            "forum_reply",
            "topic_channel_slugs",
            serde_json::json!(["web", 7])
        )));
    }
}
