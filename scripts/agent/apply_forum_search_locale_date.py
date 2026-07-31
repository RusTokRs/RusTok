from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    write(path, content.replace(old, new, 1))


# Forum-owned projection timestamps.
replace_once(
    "crates/rustok-forum/src/search_projection.rs",
    '                "solution_reply_id": topic.solution_reply_id,\n                "route": format!("/modules/forum?topic={}", topic.id)\n',
    '                "solution_reply_id": topic.solution_reply_id,\n                "published_at": created_at.to_rfc3339(),\n                "route": format!("/modules/forum?topic={}", topic.id)\n',
)
replace_once(
    "crates/rustok-forum/src/search_projection.rs",
    '                "is_solution": is_solution,\n                "route": route\n',
    '                "is_solution": is_solution,\n                "published_at": created_at.to_rfc3339(),\n                "route": route\n',
)

# Search-owned exact locale and date-window predicate.
write(
    "crates/rustok-search/src/forum_document_filters.rs",
    r'''use chrono::{DateTime, Utc};
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

    pub fn matches(&self, item: &SearchResultItem) -> bool {
        if !self.matches_exact_locale(item) {
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
            && self.matches_published_window(item)
    }

    fn matches_exact_locale(&self, item: &SearchResultItem) -> bool {
        let Some(expected) = self.exact_locale.as_deref() else {
            return true;
        };
        if item.source_module != "forum" {
            return false;
        }

        item.locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
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

    fn matches_published_window(&self, item: &SearchResultItem) -> bool {
        if self.published_from.is_none() && self.published_to.is_none() {
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
            .is_none_or(|from| published_at >= *from)
            && self
                .published_to
                .as_ref()
                .is_none_or(|to| published_at <= *to)
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
                payload["published_at"] = serde_json::json!("2026-07-15T12:00:00Z");
            }
            "forum_reply" => {
                payload["topic_tags"] = tags
                    .map(|values| serde_json::json!(values))
                    .unwrap_or(serde_json::Value::Null);
                payload["is_solution"] = solved
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null);
                payload["published_at"] = serde_json::json!("2026-07-15T12:00:00Z");
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
    fn exact_locale_filter_preserves_forum_categories_and_rejects_mismatch() {
        let filters = ForumStorefrontDocumentFilters {
            exact_locale: Some("en".to_string()),
            ..ForumStorefrontDocumentFilters::default()
        };
        let category = item("forum_category", None, None, None);
        let mut mismatch = item("forum_topic", None, None, None);
        mismatch.locale = Some("de".to_string());
        let mut missing = item("forum_topic", None, None, None);
        missing.locale = None;
        let mut product = item("forum_topic", None, None, None);
        product.source_module = "product".to_string();
        product.entity_type = "product".to_string();

        assert!(filters.matches(&category));
        assert!(filters.matches(&item("forum_topic", None, None, None)));
        assert!(!filters.matches(&mismatch));
        assert!(!filters.matches(&missing));
        assert!(!filters.matches(&product));
        assert!(filters.is_empty());
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
            published_from: Some(timestamp("2026-07-15T12:00:00Z")),
            published_to: Some(timestamp("2026-07-15T12:00:00Z")),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut before = item("forum_topic", None, None, None);
        before.payload["published_at"] = serde_json::json!("2026-07-15T11:59:59Z");
        let mut after = item("forum_reply", None, None, Some(false));
        after.payload["published_at"] = serde_json::json!("2026-07-15T12:00:01Z");

        assert!(filters.matches(&item("forum_topic", None, None, None)));
        assert!(filters.matches(&item("forum_reply", None, None, Some(false))));
        assert!(!filters.matches(&before));
        assert!(!filters.matches(&after));
        assert!(!filters.matches(&item("forum_category", None, None, None)));
    }

    #[test]
    fn malformed_tag_solved_or_date_projection_fails_closed() {
        let tag_filter = ForumStorefrontDocumentFilters {
            tags: vec!["Rust".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };
        let solved_filter = ForumStorefrontDocumentFilters {
            solved: Some(true),
            ..ForumStorefrontDocumentFilters::default()
        };
        let date_filter = ForumStorefrontDocumentFilters {
            published_from: Some(timestamp("2026-07-01T00:00:00Z")),
            ..ForumStorefrontDocumentFilters::default()
        };
        let mut malformed_tags = item("forum_topic", None, Some(vec!["Rust"]), None);
        malformed_tags.payload["tags"] = serde_json::json!(["Rust", 7]);
        let mut malformed_solution = item("forum_topic", None, None, Some(true));
        malformed_solution.payload["solution_reply_id"] = serde_json::json!(true);
        let mut malformed_date = item("forum_reply", None, None, Some(false));
        malformed_date.payload["published_at"] = serde_json::json!("not-a-date");

        assert!(!tag_filter.matches(&malformed_tags));
        assert!(!solved_filter.matches(&malformed_solution));
        assert!(!date_filter.matches(&malformed_date));
    }

    #[test]
    fn active_filters_intersect_and_exclude_non_forum_items() {
        let expected = Uuid::new_v4();
        let filters = ForumStorefrontDocumentFilters {
            exact_locale: Some("en".to_string()),
            author_ids: vec![expected],
            tags: vec!["Rust".to_string()],
            solved: Some(true),
            published_from: Some(timestamp("2026-07-01T00:00:00Z")),
            published_to: Some(timestamp("2026-07-31T23:59:59Z")),
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
''',
)

# Execution request, exact locale query and date normalization.
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "use std::time::Instant;\n\nuse rustok_api",
    "use std::time::Instant;\n\nuse chrono::{DateTime, Utc};\nuse rustok_api",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "    pub tags: Vec<String>,\n    pub solved: Option<bool>,\n    pub attribute_filters:",
    "    pub tags: Vec<String>,\n    pub solved: Option<bool>,\n    pub published_from: Option<String>,\n    pub published_to: Option<String>,\n    pub attribute_filters:",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "        locale: input.locale,\n        channel_id:",
    "        locale: Some(effective_locale.clone()),\n        channel_id:",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "    let fallback_locale = normalize_required_locale(&request.fallback_locale)?;\n    let requested_channel_id",
    "    let fallback_locale = normalize_required_locale(&request.fallback_locale)?;\n    let exact_locale = locale\n        .clone()\n        .unwrap_or_else(|| fallback_locale.clone());\n    let published_from =\n        normalize_optional_rfc3339(\"published_from\", request.published_from.as_deref())?;\n    let published_to =\n        normalize_optional_rfc3339(\"published_to\", request.published_to.as_deref())?;\n    if published_from\n        .as_ref()\n        .zip(published_to.as_ref())\n        .is_some_and(|(from, to)| from > to)\n    {\n        return validation(\"published_from must not be after published_to\");\n    }\n    let requested_channel_id",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "        document_filters: ForumStorefrontDocumentFilters {\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,\n            tags: normalize_tag_values(\"tags\", request.tags)?,\n            solved: request.solved,\n        },",
    "        document_filters: ForumStorefrontDocumentFilters {\n            exact_locale: Some(exact_locale),\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,\n            tags: normalize_tag_values(\"tags\", request.tags)?,\n            solved: request.solved,\n            published_from,\n            published_to,\n        },",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "fn normalize_filter_values(\n",
    "fn normalize_optional_rfc3339(\n    field: &str,\n    value: Option<&str>,\n) -> Result<Option<DateTime<Utc>>, ForumStorefrontSearchExecutionError> {\n    value\n        .map(str::trim)\n        .filter(|value| !value.is_empty())\n        .map(|value| {\n            DateTime::parse_from_rfc3339(value)\n                .map(|timestamp| timestamp.with_timezone(&Utc))\n                .map_err(|_| {\n                    ForumStorefrontSearchExecutionError::Validation(format!(\n                        \"{field} must be RFC3339\"\n                    ))\n                })\n        })\n        .transpose()\n}\n\nfn normalize_filter_values(\n",
)

# GraphQL owner arguments.
replace_once(
    "crates/rustok-search/src/graphql/forum_storefront.rs",
    "    /// exact topic/reply result eligibility and optional exact author, tag and\n    /// solved-state scope. The input must explicitly select only the `forum` source\n",
    "    /// exact topic/reply result eligibility and optional exact author, tag,\n    /// solved-state and inclusive published-date scope. The input must explicitly\n    /// select only the `forum` source\n",
)
replace_once(
    "crates/rustok-search/src/graphql/forum_storefront.rs",
    "        tags: Option<Vec<String>>,\n        solved: Option<bool>,\n    )",
    "        tags: Option<Vec<String>>,\n        solved: Option<bool>,\n        published_from: Option<String>,\n        published_to: Option<String>,\n    )",
)
replace_once(
    "crates/rustok-search/src/graphql/forum_storefront.rs",
    "            tags: tags.unwrap_or_default(),\n            solved,\n            attribute_filters:",
    "            tags: tags.unwrap_or_default(),\n            solved,\n            published_from,\n            published_to,\n            attribute_filters:",
)

# Additive GraphQL date-window transport.
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
    'const FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY: &str = "query ForumStorefrontSearchByFilters($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";\n',
    'const FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY: &str = "query ForumStorefrontSearchByFilters($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";\nconst FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY: &str = "query ForumStorefrontSearchByDateWindow($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean, $publishedFrom: String, $publishedTo: String) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved, publishedFrom: $publishedFrom, publishedTo: $publishedTo) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";\n',
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
    "struct FilterSearchPreviewVariables {\n    input: SearchPreviewInput,\n    #[serde(rename = \"authorIds\")]\n    author_ids: Option<Vec<String>>,\n    tags: Option<Vec<String>>,\n    solved: Option<bool>,\n}\n",
    "struct FilterSearchPreviewVariables {\n    input: SearchPreviewInput,\n    #[serde(rename = \"authorIds\")]\n    author_ids: Option<Vec<String>>,\n    tags: Option<Vec<String>>,\n    solved: Option<bool>,\n}\n\n#[derive(Debug, Serialize)]\nstruct DateWindowSearchPreviewVariables {\n    input: SearchPreviewInput,\n    #[serde(rename = \"authorIds\")]\n    author_ids: Option<Vec<String>>,\n    tags: Option<Vec<String>>,\n    solved: Option<bool>,\n    #[serde(rename = \"publishedFrom\")]\n    published_from: Option<String>,\n    #[serde(rename = \"publishedTo\")]\n    published_to: Option<String>,\n}\n",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
    "fn search_preview_input(\n",
    "pub async fn fetch_search_with_date_window(\n    query: String,\n    locale: Option<String>,\n    preset_key: Option<String>,\n    filters: SearchPreviewFilters,\n    author_ids: Vec<String>,\n    tags: Vec<String>,\n    solved: Option<bool>,\n    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, ApiError> {\n    let input = search_preview_input(query, locale, preset_key, filters);\n    let response: ForumStorefrontSearchResponse = execute_graphql(\n        &graphql_url(),\n        GraphqlRequest::new(\n            FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY,\n            Some(DateWindowSearchPreviewVariables {\n                input,\n                author_ids: (!author_ids.is_empty()).then_some(author_ids),\n                tags: (!tags.is_empty()).then_some(tags),\n                solved,\n                published_from,\n                published_to,\n            }),\n        ),\n        None,\n        configured_tenant_slug(),\n        None,\n    )\n    .await\n    .map_err(|error| ApiError::Graphql(error.to_string()))?;\n\n    Ok(response.forum_storefront_search)\n}\n\nfn search_preview_input(\n",
)

# Additive native date-window endpoint; legacy signatures remain unchanged.
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "#[server(prefix = \"/api/fn\", endpoint = \"search/forum-storefront-search\")]\n",
    "pub async fn fetch_search_with_date_window(\n    query: String,\n    locale: Option<String>,\n    preset_key: Option<String>,\n    filters: SearchPreviewFilters,\n    author_ids: Vec<String>,\n    tags: Vec<String>,\n    solved: Option<bool>,\n    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, ApiError> {\n    forum_storefront_search_by_date_window_native(\n        query,\n        locale,\n        preset_key,\n        filters,\n        author_ids,\n        tags,\n        solved,\n        published_from,\n        published_to,\n    )\n    .await\n    .map_err(ApiError::from)\n}\n\n#[server(prefix = \"/api/fn\", endpoint = \"search/forum-storefront-search\")]\n",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "        Vec::new(),\n        None,\n    )\n    .await\n}\n\n#[server(\n    prefix = \"/api/fn\",\n    endpoint = \"search/forum-storefront-search-by-authors\"\n)]",
    "        Vec::new(),\n        None,\n        None,\n        None,\n    )\n    .await\n}\n\n#[server(\n    prefix = \"/api/fn\",\n    endpoint = \"search/forum-storefront-search-by-authors\"\n)]",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "        author_ids,\n        Vec::new(),\n        None,\n    )\n    .await\n}\n\n#[server(\n    prefix = \"/api/fn\",\n    endpoint = \"search/forum-storefront-search-by-filters\"\n)]",
    "        author_ids,\n        Vec::new(),\n        None,\n        None,\n        None,\n    )\n    .await\n}\n\n#[server(\n    prefix = \"/api/fn\",\n    endpoint = \"search/forum-storefront-search-by-filters\"\n)]",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "    execute_forum_storefront_search_native(\n        query, locale, preset_key, filters, author_ids, tags, solved,\n    )\n    .await\n}\n\nasync fn execute_forum_storefront_search_native(\n",
    "    execute_forum_storefront_search_native(\n        query,\n        locale,\n        preset_key,\n        filters,\n        author_ids,\n        tags,\n        solved,\n        None,\n        None,\n    )\n    .await\n}\n\n#[server(\n    prefix = \"/api/fn\",\n    endpoint = \"search/forum-storefront-search-by-date-window\"\n)]\nasync fn forum_storefront_search_by_date_window_native(\n    query: String,\n    locale: Option<String>,\n    preset_key: Option<String>,\n    filters: SearchPreviewFilters,\n    author_ids: Vec<String>,\n    tags: Vec<String>,\n    solved: Option<bool>,\n    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, ServerFnError> {\n    execute_forum_storefront_search_native(\n        query,\n        locale,\n        preset_key,\n        filters,\n        author_ids,\n        tags,\n        solved,\n        published_from,\n        published_to,\n    )\n    .await\n}\n\nasync fn execute_forum_storefront_search_native(\n",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "    tags: Vec<String>,\n    solved: Option<bool>,\n) -> Result<SearchPreviewPayload, ServerFnError>",
    "    tags: Vec<String>,\n    solved: Option<bool>,\n    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, ServerFnError>",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "            tags,\n            solved,\n            attribute_filters:",
    "            tags,\n            solved,\n            published_from,\n            published_to,\n            attribute_filters:",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "        let _ = (query, locale, preset_key, filters, author_ids, tags, solved);",
    "        let _ = (\n            query,\n            locale,\n            preset_key,\n            filters,\n            author_ids,\n            tags,\n            solved,\n            published_from,\n            published_to,\n        );",
)

# Storefront transport facade.
replace_once(
    "crates/rustok-search/storefront/src/transport/mod.rs",
    "fn is_explicit_forum_category_scope(filters: &SearchPreviewFilters) -> bool {\n",
    "pub async fn fetch_forum_search_with_date_window(\n    query: String,\n    locale: Option<String>,\n    preset_key: Option<String>,\n    filters: SearchPreviewFilters,\n    author_ids: Vec<String>,\n    tags: Vec<String>,\n    solved: Option<bool>,\n    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, SearchTransportError> {\n    let native_query = query.clone();\n    let native_locale = locale.clone();\n    let native_preset_key = preset_key.clone();\n    let native_filters = filters.clone();\n    let native_author_ids = author_ids.clone();\n    let native_tags = tags.clone();\n    let native_published_from = published_from.clone();\n    let native_published_to = published_to.clone();\n\n    execute_selected_transport(\n        \"search\",\n        selected_transport_path(),\n        move || {\n            forum_native_server_adapter::fetch_search_with_date_window(\n                native_query,\n                native_locale,\n                native_preset_key,\n                native_filters,\n                native_author_ids,\n                native_tags,\n                solved,\n                native_published_from,\n                native_published_to,\n            )\n        },\n        move || {\n            forum_graphql_adapter::fetch_search_with_date_window(\n                query,\n                locale,\n                preset_key,\n                filters,\n                author_ids,\n                tags,\n                solved,\n                published_from,\n                published_to,\n            )\n        },\n    )\n    .await\n}\n\nfn is_explicit_forum_category_scope(filters: &SearchPreviewFilters) -> bool {\n",
)

# Owner note.
write(
    "crates/rustok-forum/docs/forum-23b2f3-search-locale-date-filter.md",
    r'''# FORUM-23B2F3 exact Forum Search locale and date filters

## Status

`source_complete_execution_pending`

This slice locks the explicit Forum-only storefront Search result locale to the
normalized requested locale or tenant fallback and adds an inclusive published
RFC3339 date window. Runtime, PostgreSQL and reindex evidence remain
maintainer-owned and are not claimed here.

## Exact locale boundary

The existing `SearchPreviewInput.locale` remains the only locale input. Search
normalizes it through the existing bounded locale validator; when it is absent,
the tenant fallback locale becomes the exact query locale.

The PostgreSQL FTS and typo paths therefore always receive a non-empty exact
locale for Forum-only execution. Search repeats the same exact locale assertion on
every raw result before Forum owner eligibility. A missing or mismatched result
locale fails closed. Locale-only execution still admits Forum category, topic and
reply documents and does not disable query-rule pins.

No multi-locale candidate union is introduced. Exact current owner eligibility is
still evaluated using the same single effective locale as category scope and the
Search document query.

## Owner date projection

Forum remains the owner of topic and reply creation time. Search evaluates only
Forum-projected values:

```text
forum_topic.payload.published_at
forum_reply.payload.published_at
```

Both values are derived from the owner `created_at` timestamp and serialized as
UTC RFC3339. Existing Search table timestamp columns do not become a Forum policy
source.

Legacy topic or reply documents without `published_at` fail closed while a date
window is active. A targeted or full Forum Search reindex is required before those
documents can match date-scoped queries. Searches without date bounds preserve
their previous topic/reply behavior.

## Input contract

The Forum GraphQL field accepts optional `publishedFrom` and `publishedTo` string
arguments. The additive native date-window endpoint accepts equivalent
`published_from` and `published_to` values.

Each non-empty value must be RFC3339 and is normalized to UTC. Bounds are
inclusive:

```text
published_at >= published_from
published_at <= published_to
```

Either bound may be omitted. When both are present, `published_from` must not be
after `published_to`. Missing or malformed projected timestamps fail closed.
Categories do not match an active date window.

The arguments remain separate from neutral `SearchPreviewInput`, `SearchQuery`
and `SearchPreviewFilters` contracts.

## Evaluation order

The existing Forum-only execution owner first resolves its stable raw Search
snapshot in the exact effective locale. The raw total must remain at or below 100
before date narrowing. A broad query cannot bypass the owner-call bound merely
because a later date filter would reduce the result set.

After the raw snapshot is complete and stable:

1. Search verifies the exact result locale.
2. Search intersects active author, tag, solved and date predicates.
3. Categories are retained for locale-only execution but excluded by any active
   author, tag, solved or date predicate.
4. Forum performs exact current topic and approved-reply eligibility for retained
   candidates.
5. Visible totals, facets, offset and limit are computed from the filtered and
   authorized intersection while preserving raw ranking order.

Query-rule pins remain enabled for locale-only execution and disabled whenever an
actual Forum document-narrowing filter is active.

## Transport compatibility

The existing GraphQL operations remain unchanged:

```text
ForumStorefrontSearch
ForumStorefrontSearchByAuthors
ForumStorefrontSearchByFilters
```

Date-window calls use the additive operation:

```text
ForumStorefrontSearchByDateWindow
```

The existing native endpoints also remain unchanged:

```text
search/forum-storefront-search
search/forum-storefront-search-by-authors
search/forum-storefront-search-by-filters
```

Date-window calls use the additive endpoint:

```text
search/forum-storefront-search-by-date-window
```

All operations delegate to the same Search execution owner.

## Compatibility and degraded mode

No database migration, public shared input/DTO field, neutral `SearchQuery` field,
dependency or `Cargo.lock` change is introduced. Topic and approved-reply payloads
gain `published_at`; legacy rows require reindex for date matches and fail closed
until repaired. Existing unfiltered, author-only and B2F2 filter wire operations
retain their previous shape for rolling deployments.

Missing category-scope or result-eligibility owner composition continues to fail
closed exactly as before. This slice does **not** add storefront UI controls,
kind/channel/group/attachment filters, durable non-Forum projection ordering,
deletion or ACL cleanup, or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-locale-date-filter.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should cover requested and fallback locales, a mismatched locale,
inclusive lower/upper bounds, one-sided windows, invalid/reversed input, categories,
legacy rows without `published_at`, malformed projected timestamps, raw candidate
bounds, owner eligibility, totals, facets and pagination.
''',
)

# Static guardrail; it is source-only and not executed by this workflow.
write(
    "scripts/verify/verify-forum-search-locale-date-filter.mjs",
    r'''#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  contract: "crates/rustok-forum/contracts/forum-search-locale-date-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f3-search-locale-date-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  filter: "crates/rustok-search/src/forum_document_filters.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  graphqlTypes: "crates/rustok-search/src/graphql/types.rs",
  storefrontModel: "crates/rustok-search/storefront/src/model.rs",
  graphqlAdapter: "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
  nativeAdapter: "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  transportFacade: "crates/rustok-search/storefront/src/transport/mod.rs",
  engine: "crates/rustok-search/src/engine.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}
function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}
function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}
function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const contract = parseJson(paths.contract);
const note = read(paths.note);
const projection = read(paths.projection);
const filter = read(paths.filter);
const execution = read(paths.execution);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlTypes = read(paths.graphqlTypes);
const storefrontModel = read(paths.storefrontModel);
const graphqlAdapter = read(paths.graphqlAdapter);
const nativeAdapter = read(paths.nativeAdapter);
const transportFacade = read(paths.transportFacade);
const engine = read(paths.engine);

requireAll(projection, [
  '"published_at": created_at.to_rfc3339()',
  '"topic_tags": topic_tags',
  '"is_solution": is_solution',
], paths.projection);
if (projection.split('"published_at": created_at.to_rfc3339()').length - 1 !== 2) {
  failures.push(`${paths.projection}: topic and reply published_at projections are required`);
}

requireAll(filter, [
  "pub exact_locale: Option<String>",
  "pub published_from: Option<DateTime<Utc>>",
  "pub published_to: Option<DateTime<Utc>>",
  "matches_exact_locale",
  "matches_published_window",
  "DateTime::parse_from_rfc3339",
  "published_at >= *from",
  "published_at <= *to",
  "exact_locale_filter_preserves_forum_categories_and_rejects_mismatch",
  "published_window_is_inclusive_and_excludes_categories",
  "malformed_tag_solved_or_date_projection_fails_closed",
], paths.filter);
rejectAll(filter, ["rustok_forum", "forum_topic::", "forum_reply::"], `${paths.filter} owner-neutral boundary`);

requireAll(execution, [
  "pub published_from: Option<String>",
  "pub published_to: Option<String>",
  "locale: Some(effective_locale.clone())",
  "exact_locale: Some(exact_locale)",
  'normalize_optional_rfc3339("published_from"',
  'normalize_optional_rfc3339("published_to"',
  "published_from must not be after published_to",
  "all_items.retain(|item| document_filters.matches(item));",
  "if document_filters.is_empty()",
], paths.execution);
if (execution.indexOf("let raw_total =") > execution.indexOf("all_items.retain(|item| document_filters.matches(item));")) {
  failures.push(`${paths.execution}: raw candidate bound must precede date narrowing`);
}

requireAll(graphqlOwner, [
  "published_from: Option<String>",
  "published_to: Option<String>",
  "published_from,",
  "published_to,",
], paths.graphqlOwner);
requireAll(graphqlAdapter, [
  "ForumStorefrontSearchByDateWindow",
  "$publishedFrom: String",
  "$publishedTo: String",
  "DateWindowSearchPreviewVariables",
  "fetch_search_with_date_window",
], paths.graphqlAdapter);
requireAll(nativeAdapter, [
  "fetch_search_with_date_window",
  'endpoint = "search/forum-storefront-search-by-date-window"',
  "published_from: Option<String>",
  "published_to: Option<String>",
], paths.nativeAdapter);
requireAll(transportFacade, [
  "pub async fn fetch_forum_search_with_date_window",
  "forum_native_server_adapter::fetch_search_with_date_window",
  "forum_graphql_adapter::fetch_search_with_date_window",
], paths.transportFacade);

requireAll(graphqlAdapter, [
  "ForumStorefrontSearch($input: SearchPreviewInput!)",
  "ForumStorefrontSearchByAuthors",
  "ForumStorefrontSearchByFilters",
], `${paths.graphqlAdapter} legacy operations`);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search"',
  'endpoint = "search/forum-storefront-search-by-authors"',
  'endpoint = "search/forum-storefront-search-by-filters"',
], `${paths.nativeAdapter} legacy endpoints`);
rejectAll(graphqlTypes, ["published_from", "published_to", "publishedFrom", "publishedTo"], `${paths.graphqlTypes} neutral SearchPreviewInput`);
rejectAll(storefrontModel, ["published_from", "published_to", "publishedFrom", "publishedTo"], `${paths.storefrontModel} neutral shared filter DTO`);
rejectAll(engine, ["published_from", "published_to", "ForumStorefrontDocumentFilters"], `${paths.engine} neutral SearchQuery`);

requireAll(forumPlan, ["FORUM-23B2F3", "exact Forum locale and date filters", "verify-forum-search-locale-date-filter.mjs"], paths.forumPlan);
requireAll(searchPlan, ["FORUM-23B2F3", "source_complete_execution_pending", "exact locale and inclusive published date-window"], paths.searchPlan);
requireAll(note, ["# FORUM-23B2F3 exact Forum Search locale and date filters", "Locale-only execution still admits Forum category", "does **not** add storefront UI controls"], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2F3") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") failures.push(`${paths.contract}: unexpected status`);
  if (contract.input?.date_format !== "RFC3339") failures.push(`${paths.contract}: date format drift`);
  if (!contract.evaluation?.postgres_locale_predicate_is_exact) failures.push(`${paths.contract}: exact PostgreSQL locale invariant missing`);
  if (!contract.evaluation?.post_scan_locale_assertion_is_exact) failures.push(`${paths.contract}: post-scan locale invariant missing`);
  if (!contract.evaluation?.raw_candidate_limit_is_checked_before_date_narrowing) failures.push(`${paths.contract}: raw candidate ordering invariant missing`);
  if (contract.transport_compatibility?.existing_wire_signatures_changed !== false) failures.push(`${paths.contract}: legacy wire signatures changed`);
  if (contract.compatibility?.search_query_shape_changed !== false) failures.push(`${paths.contract}: neutral SearchQuery changed`);
}

if (failures.length > 0) {
  console.error("FORUM-23B2F3 Search locale/date verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("FORUM-23B2F3 Search locale/date source contract is consistent.");
''',
)

# Canonical Search plan.
search_plan = "crates/rustok-search/docs/implementation-plan.md"
replace_once(
    search_plan,
    "Runtime and reindex evidence remain pending.\n\nSearch settings have one owner boundary.",
    "Runtime and reindex evidence remain pending.\n\n`FORUM-23B2F3` locks Forum-only Search to one normalized exact locale: the\nrequested locale or tenant fallback is sent to PostgreSQL FTS/typo filtering,\ncategory scope, owner eligibility and a post-scan result assertion. It also adds\ninclusive RFC3339 `published_from` / `published_to` bounds over Forum-projected\n`payload.published_at` for topics and approved replies. The stable raw 100-row cap\nremains before date narrowing; date filters intersect with author/tag/solved\nbefore owner eligibility and visible totals/facets/pagination. Locale-only\nexecution retains categories and query-rule pins, while an active date window\nexcludes categories and suppresses pins. Legacy topic/reply documents without the\nnew timestamp fail closed until Forum Search reindex. Existing legacy, author-only\nand B2F2 filter operations plus neutral DTOs and mixed/Product/admin Search remain\nunchanged. Runtime and reindex evidence remain pending.\n\nSearch settings have one owner boundary.",
)
replace_once(
    search_plan,
    "- Exact Forum tag and solved contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-tag-solved-filter.json` and\n  `scripts/verify/verify-forum-search-tag-solved-filter.mjs`.\n",
    "- Exact Forum tag and solved contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-tag-solved-filter.json` and\n  `scripts/verify/verify-forum-search-tag-solved-filter.mjs`.\n- Exact Forum locale and date filter status:\n  `source_complete_execution_pending` under `FORUM-23B2F3`.\n- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.\n",
)
replace_once(
    search_plan,
    "- Exact Forum tag and solved filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F2`.\n",
    "- Exact Forum tag and solved filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F2`.\n- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n",
)
replace_once(
    search_plan,
    "21. Added exact bounded Forum tag and solved-state filters, parent-topic tag\n    projection for approved replies, additive GraphQL/native filter operations,\n    pre-eligibility intersection, post-authorization totals/facets/pagination, and\n    fail-closed legacy reply behavior under `FORUM-23B2F2`.\n\n## Next results\n\n1. **Complete remaining Forum storefront query filters.** Add locale, date, kind,\n   channel/group and attachment-presence filters",
    "21. Added exact bounded Forum tag and solved-state filters, parent-topic tag\n    projection for approved replies, additive GraphQL/native filter operations,\n    pre-eligibility intersection, post-authorization totals/facets/pagination, and\n    fail-closed legacy reply behavior under `FORUM-23B2F2`.\n22. Locked Forum-only Search to the exact requested or fallback locale and added\n    inclusive RFC3339 published date-window filtering, Forum-owned timestamp\n    projection, additive GraphQL/native date-window transports and fail-closed\n    legacy projection behavior under `FORUM-23B2F3`.\n\n## Next results\n\n1. **Complete remaining Forum storefront query filters.** Add kind,\n   channel/group and attachment-presence filters",
)

# Canonical Forum plan.
forum_plan = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    forum_plan,
    "FORUM-23B2F2 adds exact bounded Forum tag and solved filters before owner eligibility, visible totals, facets and pagination. Remaining locale, date, kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
    "FORUM-23B2F2 adds exact bounded Forum tag and solved filters; FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters before owner eligibility, visible totals, facets and pagination. Remaining kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
)
replace_once(
    forum_plan,
    "### Compatibility and degraded mode\n\nNo database migration",
    "### Delivered in `FORUM-23B2F3`\n\n- the requested locale or tenant fallback is normalized once and used as the exact\n  PostgreSQL FTS/typo locale, category-scope locale, owner-eligibility locale and\n  post-scan result assertion; missing or mismatched row locale fails closed;\n- locale-only Forum Search retains category, topic and reply results and does not\n  disable query-rule pins; no multi-locale union is introduced;\n- topics and approved replies project Forum-owned creation time as UTC RFC3339\n  `payload.published_at`; legacy rows without it fail closed for date windows until\n  reindexed;\n- optional inclusive `published_from` / `published_to` bounds accept RFC3339, may\n  be one-sided, reject reversed ranges, exclude categories and fail closed on\n  malformed projected timestamps;\n- date narrowing intersects author/tag/solved after the stable bounded raw snapshot\n  and before exact Forum owner eligibility, visible totals, facets, offset and limit;\n- existing legacy, author-only and B2F2 filter GraphQL/native wire contracts remain\n  unchanged; date windows use additive `ForumStorefrontSearchByDateWindow` and\n  `search/forum-storefront-search-by-date-window` transports;\n- `forum-search-locale-date-filter.json`,\n  `forum-23b2f3-search-locale-date-filter.md`, and\n  `verify-forum-search-locale-date-filter.mjs` lock the locale, projection, range,\n  ordering, compatibility and degraded-mode contract while recording execution and\n  reindex evidence as pending.\n\n### Compatibility and degraded mode\n\nNo database migration",
)
replace_once(
    forum_plan,
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2`. `FORUM-23B2F2` extends the\nForum reply projection payload with parent-topic `topic_tags`; legacy reply rows\nrequire reindex before positive tag matches and fail closed until repaired.",
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3`. `FORUM-23B2F2` extends\nthe Forum reply projection payload with parent-topic `topic_tags`; `FORUM-23B2F3`\nextends topic and reply payloads with Forum-owned `published_at`. Legacy rows\nrequire reindex before positive tag/date matches and fail closed until repaired.",
)
replace_once(
    forum_plan,
    "Legacy replies without `topic_tags` fail closed for tag queries until reindexed; existing\nlegacy and author-only GraphQL/native operations remain unchanged. Admin/global Search\nbehavior remains unchanged.\n\n### Remaining scope\n\n- add locale, date, kind, channel/group and attachment-presence query filters;",
    "Legacy replies without `topic_tags` fail closed for tag queries until reindexed. Exact\nrequested/fallback locale now scopes PostgreSQL and post-scan results; date windows use\nForum-projected timestamps and legacy topic/reply rows fail closed until reindexed. Existing\nlegacy, author-only and B2F2 filter GraphQL/native operations remain unchanged. Admin/global\nSearch behavior remains unchanged.\n\n### Remaining scope\n\n- add kind, channel/group and attachment-presence query filters;",
)
replace_once(
    forum_plan,
    "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\ncargo check -p rustok-search",
    "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\nnode scripts/verify/verify-forum-search-locale-date-filter.mjs\ncargo check -p rustok-search",
)
replace_once(
    forum_plan,
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2` source and contract records do not",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3` source and contract records do not",
)
# The second aggregate verification list has the same adjacent marker after the first replacement.
replace_once(
    forum_plan,
    "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\ncargo check -p rustok-search",
    "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\nnode scripts/verify/verify-forum-search-locale-date-filter.mjs\ncargo check -p rustok-search",
)
replace_once(
    forum_plan,
    "14. continue `FORUM-23` with locale, date, kind, channel/group and\n    attachment-presence filters before owner revision ordering and reconciliation;\n    execute B2D/F1/F2 evidence with `LINK-FORUM-03` only after ordering is stable;",
    "14. continue `FORUM-23` with kind, channel/group and attachment-presence\n    filters before owner revision ordering and reconciliation; execute B2D/F1/F2/F3\n    evidence with `LINK-FORUM-03` only after ordering is stable;",
)
