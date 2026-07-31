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


# Search-owned exact Forum document filter.
path = "crates/rustok-search/src/forum_document_filters.rs"
replace_once(
    path,
    "    pub exact_locale: Option<String>,\n    pub author_ids: Vec<Uuid>,",
    "    pub exact_locale: Option<String>,\n    pub kinds: Vec<String>,\n    pub author_ids: Vec<Uuid>,",
)
replace_once(
    path,
    "        self.author_ids.is_empty()\n            && self.tags.is_empty()",
    "        self.kinds.is_empty()\n            && self.author_ids.is_empty()\n            && self.tags.is_empty()",
)
replace_once(
    path,
    "        self.matches_author(item)\n            && self.matches_tags(item)",
    "        self.matches_kind(item)\n            && self.matches_author(item)\n            && self.matches_tags(item)",
)
replace_once(
    path,
    "    fn matches_author(&self, item: &SearchResultItem) -> bool {",
    '''    fn matches_kind(&self, item: &SearchResultItem) -> bool {
        if self.kinds.is_empty() {
            return true;
        }

        let kind = match item.entity_type.as_str() {
            "forum_topic" => "topic",
            "forum_reply" => "reply",
            _ => return false,
        };
        self.kinds.iter().any(|expected| expected == kind)
    }

    fn matches_author(&self, item: &SearchResultItem) -> bool {''',
)
replace_once(
    path,
    "    #[test]\n    fn author_filter_matches_exact_public_topic_or_reply_author() {",
    '''    #[test]
    fn kind_filter_selects_exact_topic_or_reply_documents() {
        let topics = ForumStorefrontDocumentFilters {
            kinds: vec!["topic".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };
        let replies = ForumStorefrontDocumentFilters {
            kinds: vec!["reply".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };
        let both = ForumStorefrontDocumentFilters {
            kinds: vec!["reply".to_string(), "topic".to_string()],
            ..ForumStorefrontDocumentFilters::default()
        };

        assert!(topics.matches(&item("forum_topic", None, None, None)));
        assert!(!topics.matches(&item("forum_reply", None, None, None)));
        assert!(!topics.matches(&item("forum_category", None, None, None)));
        assert!(replies.matches(&item("forum_reply", None, None, None)));
        assert!(!replies.matches(&item("forum_topic", None, None, None)));
        assert!(both.matches(&item("forum_topic", None, None, None)));
        assert!(both.matches(&item("forum_reply", None, None, None)));
    }

    #[test]
    fn author_filter_matches_exact_public_topic_or_reply_author() {''',
)
replace_once(
    path,
    "        let filters = ForumStorefrontDocumentFilters {\n            author_ids: vec![expected],\n            tags: vec![\"Rust\".to_string()],",
    "        let filters = ForumStorefrontDocumentFilters {\n            kinds: vec![\"reply\".to_string()],\n            author_ids: vec![expected],\n            tags: vec![\"Rust\".to_string()],",
)

# Unified execution owner and bounded input normalization.
path = "crates/rustok-search/src/forum_storefront_execution.rs"
replace_once(
    path,
    "    pub category_ids: Vec<String>,\n    pub author_ids: Vec<String>,",
    "    pub category_ids: Vec<String>,\n    pub kinds: Vec<String>,\n    pub author_ids: Vec<String>,",
)
replace_once(
    path,
    "            exact_locale: Some(exact_locale),\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,",
    "            exact_locale: Some(exact_locale),\n            kinds: normalize_forum_kinds(request.kinds)?,\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,",
)
replace_once(
    path,
    "fn normalize_tag_values(\n    field: &str,",
    '''fn normalize_forum_kinds(
    values: Vec<String>,
) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {
    if values.len() > 2 {
        return validation("kinds exceeds the maximum size of 2 values");
    }
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| {
            if !matches!(value.as_str(), "topic" | "reply") {
                return validation("kinds contains an unsupported value");
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_tag_values(
    field: &str,''',
)

# GraphQL owner remains the single execution owner.
path = "crates/rustok-search/src/graphql/forum_storefront.rs"
replace_once(
    path,
    "        solved: Option<bool>,\n        published_from: Option<String>,",
    "        solved: Option<bool>,\n        kinds: Option<Vec<String>>,\n        published_from: Option<String>,",
)
replace_once(
    path,
    "            category_ids: input.category_ids.unwrap_or_default(),\n            author_ids: author_ids.unwrap_or_default(),",
    "            category_ids: input.category_ids.unwrap_or_default(),\n            kinds: kinds.unwrap_or_default(),\n            author_ids: author_ids.unwrap_or_default(),",
)

# Additive GraphQL transport operation.
path = "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs"
replace_once(
    path,
    "const FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY: &str = \"query ForumStorefrontSearchByDateWindow($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean, $publishedFrom: String, $publishedTo: String) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved, publishedFrom: $publishedFrom, publishedTo: $publishedTo) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }\";",
    "const FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY: &str = \"query ForumStorefrontSearchByDateWindow($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean, $publishedFrom: String, $publishedTo: String) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved, publishedFrom: $publishedFrom, publishedTo: $publishedTo) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }\";\nconst FORUM_STOREFRONT_SEARCH_BY_KINDS_QUERY: &str = \"query ForumStorefrontSearchByKinds($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean, $publishedFrom: String, $publishedTo: String, $kinds: [String!]!) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved, publishedFrom: $publishedFrom, publishedTo: $publishedTo, kinds: $kinds) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }\";",
)
replace_once(
    path,
    "struct DateWindowSearchPreviewVariables {\n    input: SearchPreviewInput,",
    '''struct KindSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "authorIds")]
    author_ids: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    solved: Option<bool>,
    #[serde(rename = "publishedFrom")]
    published_from: Option<String>,
    #[serde(rename = "publishedTo")]
    published_to: Option<String>,
    kinds: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DateWindowSearchPreviewVariables {
    input: SearchPreviewInput,''',
)
replace_once(
    path,
    "fn search_preview_input(\n    query: String,",
    '''pub async fn fetch_search_with_kinds(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
    kinds: Vec<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            FORUM_STOREFRONT_SEARCH_BY_KINDS_QUERY,
            Some(KindSearchPreviewVariables {
                input,
                author_ids: (!author_ids.is_empty()).then_some(author_ids),
                tags: (!tags.is_empty()).then_some(tags),
                solved,
                published_from,
                published_to,
                kinds,
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(response.forum_storefront_search)
}

fn search_preview_input(
    query: String,''',
)

# Additive native endpoint; existing endpoint signatures stay unchanged.
path = "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs"
replace_once(
    path,
    "#[server(prefix = \"/api/fn\", endpoint = \"search/forum-storefront-search\")]",
    '''pub async fn fetch_search_with_kinds(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
    kinds: Vec<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    forum_storefront_search_by_kinds_native(
        query,
        locale,
        preset_key,
        filters,
        author_ids,
        tags,
        solved,
        published_from,
        published_to,
        kinds,
    )
    .await
    .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "search/forum-storefront-search")]''',
)
# Existing calls receive an empty kind scope.
content = read(path)
old_call_tail = "        None,\n        None,\n    )"
if content.count(old_call_tail) != 3:
    raise SystemExit(f"{path}: expected three legacy helper tails")
content = content.replace(old_call_tail, "        None,\n        None,\n        Vec::new(),\n    )")
write(path, content)
replace_once(
    path,
    "        published_from,\n        published_to,\n    )\n    .await\n}\n\nasync fn execute_forum_storefront_search_native(",
    '''        published_from,
        published_to,
        Vec::new(),
    )
    .await
}

#[server(
    prefix = "/api/fn",
    endpoint = "search/forum-storefront-search-by-kinds"
)]
async fn forum_storefront_search_by_kinds_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
    kinds: Vec<String>,
) -> Result<SearchPreviewPayload, ServerFnError> {
    execute_forum_storefront_search_native(
        query,
        locale,
        preset_key,
        filters,
        author_ids,
        tags,
        solved,
        published_from,
        published_to,
        kinds,
    )
    .await
}

async fn execute_forum_storefront_search_native(''',
)
replace_once(
    path,
    "    published_from: Option<String>,\n    published_to: Option<String>,\n) -> Result<SearchPreviewPayload, ServerFnError> {",
    "    published_from: Option<String>,\n    published_to: Option<String>,\n    kinds: Vec<String>,\n) -> Result<SearchPreviewPayload, ServerFnError> {",
)
replace_once(
    path,
    "            category_ids: filters.category_ids,\n            author_ids,",
    "            category_ids: filters.category_ids,\n            kinds,\n            author_ids,",
)
replace_once(
    path,
    "            published_from,\n            published_to,\n        );",
    "            published_from,\n            published_to,\n            kinds,\n        );",
)

# Storefront transport facade.
path = "crates/rustok-search/storefront/src/transport/mod.rs"
replace_once(
    path,
    "fn is_explicit_forum_category_scope(filters: &SearchPreviewFilters) -> bool {",
    '''pub async fn fetch_forum_search_by_kinds(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
    kinds: Vec<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();
    let native_tags = tags.clone();
    let native_published_from = published_from.clone();
    let native_published_to = published_to.clone();
    let native_kinds = kinds.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_kinds(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
                native_tags,
                solved,
                native_published_from,
                native_published_to,
                native_kinds,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_kinds(
                query,
                locale,
                preset_key,
                filters,
                author_ids,
                tags,
                solved,
                published_from,
                published_to,
                kinds,
            )
        },
    )
    .await
}

fn is_explicit_forum_category_scope(filters: &SearchPreviewFilters) -> bool {''',
)

# Canonical Forum plan.
path = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    path,
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters before owner eligibility, visible totals, facets and pagination. Remaining kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters; FORUM-23B2F4 adds an exact bounded Forum document-kind filter before owner eligibility, visible totals, facets and pagination. Remaining channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
)
replace_once(
    path,
    "### Compatibility and degraded mode\n\nNo database migration, manual backfill, Search query shape, dependency, public",
    '''### Delivered in `FORUM-23B2F4`

- the existing unified Forum-only GraphQL owner accepts an optional bounded
  `kinds` argument; additive GraphQL/native operations carry it without changing
  neutral Search inputs or shared DTOs;
- accepted values are trimmed ASCII-lowercase exact `topic` and `reply`, capped at
  two input values and deduplicated before evaluation;
- `topic` matches only `forum_topic`, `reply` matches only `forum_reply`, and
  categories/non-Forum rows do not match an active kind scope;
- kind intersects exact locale, author, tag, solved and date predicates after the
  stable bounded raw snapshot and before exact Forum owner eligibility, visible
  totals, facets, offset and limit while preserving ranking order;
- the raw 100-candidate cap remains before narrowing and query-rule pins remain
  disabled under an active kind filter;
- existing legacy, author-only, B2F2 and B2F3 GraphQL/native wire signatures remain
  unchanged; kind calls use additive `ForumStorefrontSearchByKinds` and
  `search/forum-storefront-search-by-kinds` transports;
- `forum-search-kind-filter.json`, `forum-23b2f4-search-kind-filter.md`, and
  `verify-forum-search-kind-filter.mjs` lock bounds, exact semantics, ordering,
  compatibility and degraded mode while recording execution as pending.

### Compatibility and degraded mode

No database migration, manual backfill, Search query shape, dependency, public''',
)
replace_once(
    path,
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3`. `FORUM-23B2F2` extends",
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3/B2F4`. `FORUM-23B2F2` extends",
)
replace_once(
    path,
    "unchanged. Admin/global Search behavior remains unchanged.\n\n### Remaining scope\n\n- add kind, channel/group and attachment-presence query filters;",
    "unchanged. An active `topic`/`reply` kind scope uses only Search entity identity, intersects all delivered Forum document filters and suppresses pins; future wiki/announcement/Q&A policies are not claimed. Admin/global Search behavior remains unchanged.\n\n### Remaining scope\n\n- add channel/group and attachment-presence query filters;",
)
content = read(path)
marker = "node scripts/verify/verify-forum-search-locale-date-filter.mjs\ncargo check -p rustok-search"
if content.count(marker) != 2:
    raise SystemExit(f"{path}: expected two verification markers")
content = content.replace(
    marker,
    "node scripts/verify/verify-forum-search-locale-date-filter.mjs\nnode scripts/verify/verify-forum-search-kind-filter.mjs\ncargo check -p rustok-search",
)
content = content.replace(
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3` source and contract records do not",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3/B2F4` source and contract records do not",
    1,
)
content = content.replace(
    "14. continue `FORUM-23` with kind, channel/group and attachment-presence\n    filters before owner revision ordering and reconciliation; execute B2D/F1/F2/F3\n    evidence with `LINK-FORUM-03` only after ordering is stable;",
    "14. continue `FORUM-23` with channel/group and attachment-presence filters\n    before owner revision ordering and reconciliation; execute B2D/F1/F2/F3/F4\n    evidence with `LINK-FORUM-03` only after ordering is stable;",
    1,
)
write(path, content)

# Canonical Search plan.
path = "crates/rustok-search/docs/implementation-plan.md"
replace_once(
    path,
    "Runtime and reindex evidence remain\npending.\n\nSearch settings have one owner boundary.",
    '''Runtime and reindex evidence remain
pending.

`FORUM-23B2F4` adds an exact bounded `topic`/`reply` document-kind filter to the
same unified Forum-only execution owner. At most two values are trimmed,
ASCII-lowercased, validated and deduplicated. The raw 100-candidate cap remains
before narrowing; kind intersects exact locale, author, tag, solved and date
predicates before owner eligibility and visible totals/facets/pagination. An
active kind scope excludes categories and suppresses query-rule pins. Existing
GraphQL/native operations remain unchanged; additive `ByKinds` transports carry
the new argument without changing neutral DTOs, `SearchQuery`, mixed/Product/admin
Search or Forum projection shape. Runtime evidence remains pending.

Search settings have one owner boundary.''',
)
replace_once(
    path,
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.",
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.\n- Exact Forum document-kind filter status:\n  `source_complete_execution_pending` under `FORUM-23B2F4`.\n- Exact Forum document-kind contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-kind-filter.json` and\n  `scripts/verify/verify-forum-search-kind-filter.mjs`.",
)
replace_once(
    path,
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Durable non-Forum projection replay/recovery remains `blocked`.",
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Exact Forum document-kind filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F4`.\n- Durable non-Forum projection replay/recovery remains `blocked`.",
)
replace_once(
    path,
    "    fail-closed legacy projection behavior under `FORUM-23B2F3`.\n\n## Next results",
    "    fail-closed legacy projection behavior under `FORUM-23B2F3`.\n23. Added exact bounded Forum topic/reply document-kind filtering, additive\n    GraphQL/native transport parity, pre-eligibility intersection and active-scope\n    pin suppression under `FORUM-23B2F4`.\n\n## Next results",
)
replace_once(
    path,
    "1. **Complete remaining Forum storefront query filters.** Add kind,\n   channel/group and attachment-presence filters without moving owner",
    "1. **Complete remaining Forum storefront query filters.** Add channel/group\n   and attachment-presence filters without moving owner",
)
replace_once(
    path,
    "- `node scripts/verify/verify-forum-search-locale-date-filter.mjs`",
    "- `node scripts/verify/verify-forum-search-locale-date-filter.mjs`\n- `node scripts/verify/verify-forum-search-kind-filter.mjs`",
)
replace_once(
    path,
    "- [Forum locale/date contract](../../rustok-forum/contracts/forum-search-locale-date-filter.json)",
    "- [Forum locale/date contract](../../rustok-forum/contracts/forum-search-locale-date-filter.json)\n- [Forum document-kind contract](../../rustok-forum/contracts/forum-search-kind-filter.json)",
)
