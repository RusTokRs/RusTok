from pathlib import Path

PROJECTION = Path("crates/rustok-forum/src/search_projection.rs")
EXECUTION = Path("crates/rustok-search/src/forum_storefront_execution.rs")
GRAPHQL_OWNER = Path("crates/rustok-search/src/graphql/forum_storefront.rs")
NATIVE_ADAPTER = Path("crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs")
TRANSPORT = Path("crates/rustok-search/storefront/src/transport/mod.rs")
FORUM_PLAN = Path("crates/rustok-forum/docs/implementation-plan.md")
SEARCH_PLAN = Path("crates/rustok-search/docs/implementation-plan.md")


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old[:200]}")
    path.write_text(text.replace(old, new, 1))


def replace_all(path: Path, old: str, new: str, expected: int) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} replacements, found {count}\n{old[:200]}"
        )
    path.write_text(text.replace(old, new))


# Forum-owned projection: replies inherit the localized parent-topic tag list.
replace_once(
    PROJECTION,
    "        let author_payload = public_author_payload(author.as_ref());\n\n        let created_at = parse_timestamp(&reply.created_at, \"reply.created_at\")?;",
    "        let author_payload = public_author_payload(author.as_ref());\n        let topic_tags = topic.tags.clone();\n\n        let created_at = parse_timestamp(&reply.created_at, \"reply.created_at\")?;",
)
replace_once(
    PROJECTION,
    '                "author": author_payload,\n                "parent_reply_id": reply.parent_reply_id,',
    '                "author": author_payload,\n                "topic_tags": topic_tags,\n                "parent_reply_id": reply.parent_reply_id,',
)

# Search execution contract and exact input normalization.
replace_once(
    EXECUTION,
    "    pub category_ids: Vec<String>,\n    pub author_ids: Vec<String>,\n    pub attribute_filters: Vec<ForumStorefrontSearchAttributeFilter>,",
    "    pub category_ids: Vec<String>,\n    pub author_ids: Vec<String>,\n    pub tags: Vec<String>,\n    pub solved: Option<bool>,\n    pub attribute_filters: Vec<ForumStorefrontSearchAttributeFilter>,",
)
replace_once(
    EXECUTION,
    "        document_filters: ForumStorefrontDocumentFilters {\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,\n        },",
    "        document_filters: ForumStorefrontDocumentFilters {\n            author_ids: normalize_uuid_values(\"author_ids\", request.author_ids)?,\n            tags: normalize_tag_values(\"tags\", request.tags)?,\n            solved: request.solved,\n        },",
)
replace_once(
    EXECUTION,
    "fn normalize_uuid_values(\n    field: &str,\n    values: Vec<String>,\n) -> Result<Vec<Uuid>, ForumStorefrontSearchExecutionError> {",
    "fn normalize_tag_values(\n    field: &str,\n    values: Vec<String>,\n) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {\n    if values.len() > MAX_FILTER_VALUES {\n        return validation(format!(\n            \"{field} exceeds the maximum size of {MAX_FILTER_VALUES} values\"\n        ));\n    }\n    let mut normalized = values\n        .into_iter()\n        .map(|value| {\n            let value = value.trim();\n            if value.is_empty()\n                || value.chars().count() > MAX_FILTER_VALUE_LEN\n                || value.chars().any(char::is_control)\n            {\n                return validation(format!(\"{field} contains an invalid value\"));\n            }\n            Ok(value.to_string())\n        })\n        .collect::<Result<Vec<_>, _>>()?;\n    normalized.sort();\n    normalized.dedup();\n    Ok(normalized)\n}\n\nfn normalize_uuid_values(\n    field: &str,\n    values: Vec<String>,\n) -> Result<Vec<Uuid>, ForumStorefrontSearchExecutionError> {",
)

# GraphQL exposes optional Forum-only arguments while retaining the field owner.
replace_once(
    GRAPHQL_OWNER,
    "    /// exact topic/reply result eligibility and optional exact author scope. The\n    /// input must explicitly select only the `forum` source and at least one\n    /// category root.",
    "    /// exact topic/reply result eligibility and optional exact author, tag and\n    /// solved-state scope. The input must explicitly select only the `forum` source\n    /// and at least one category root.",
)
replace_once(
    GRAPHQL_OWNER,
    "        input: SearchPreviewInput,\n        author_ids: Option<Vec<String>>,\n    ) -> Result<SearchPreviewPayload> {",
    "        input: SearchPreviewInput,\n        author_ids: Option<Vec<String>>,\n        tags: Option<Vec<String>>,\n        solved: Option<bool>,\n    ) -> Result<SearchPreviewPayload> {",
)
replace_once(
    GRAPHQL_OWNER,
    "            author_ids: author_ids.unwrap_or_default(),\n            attribute_filters: input",
    "            author_ids: author_ids.unwrap_or_default(),\n            tags: tags.unwrap_or_default(),\n            solved,\n            attribute_filters: input",
)
replace_once(
    GRAPHQL_OWNER,
    '            "forum_category_scope_author_filter_result_eligibility_then_fts",',
    '            "forum_category_scope_document_filters_result_eligibility_then_fts",',
)

# Correct the additive native filter endpoint argument order.
replace_once(
    NATIVE_ADAPTER,
    "    execute_forum_storefront_search_native(\n        query, preset_key, locale, filters, author_ids, tags, solved,\n    )",
    "    execute_forum_storefront_search_native(\n        query, locale, preset_key, filters, author_ids, tags, solved,\n    )",
)

# Add a Forum-specific facade without changing the shared Search filter DTO.
transport_marker = """pub async fn fetch_forum_search_by_authors(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_authors(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_authors(
                query, locale, preset_key, filters, author_ids,
            )
        },
    )
    .await
}

fn is_explicit_forum_category_scope"""
transport_replacement = """pub async fn fetch_forum_search_by_authors(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_authors(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_authors(
                query, locale, preset_key, filters, author_ids,
            )
        },
    )
    .await
}

pub async fn fetch_forum_search_with_filters(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();
    let native_tags = tags.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_filters(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
                native_tags,
                solved,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_filters(
                query,
                locale,
                preset_key,
                filters,
                author_ids,
                tags,
                solved,
            )
        },
    )
    .await
}

fn is_explicit_forum_category_scope"""
replace_once(TRANSPORT, transport_marker, transport_replacement)

# Search canonical plan.
replace_once(
    SEARCH_PLAN,
    "Runtime evidence remains pending.\n\nSearch settings have one owner boundary.",
    "Runtime evidence remains pending.\n\n`FORUM-23B2F2` adds exact bounded tag and solved-state filters to the same\nexplicit Forum-only execution owner. Tag values are trimmed, case-sensitive,\nexact and intersect with AND semantics. Topics use `payload.tags`; approved\nreplies use Forum-projected parent `payload.topic_tags`. Solved topics are derived\nfrom `solution_reply_id`, while replies use the exact current `is_solution` marker.\nThe raw 100-candidate cap remains before narrowing, all active document filters\nintersect before owner eligibility, visible totals/facets/pagination are computed\nafter authorization, and query-rule pins remain disabled under any active document\nfilter. Legacy replies without `topic_tags` fail closed for tag-scoped queries until\na Forum Search reindex. Existing GraphQL/native legacy and author-only operations,\nneutral DTOs, mixed/Product/admin Search and unfiltered behavior remain unchanged.\nRuntime and reindex evidence remain pending.\n\nSearch settings have one owner boundary.",
)
replace_once(
    SEARCH_PLAN,
    "- Exact Forum author filter contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-author-filter.json` and\n  `scripts/verify/verify-forum-search-author-filter.mjs`.\n",
    "- Exact Forum author filter contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-author-filter.json` and\n  `scripts/verify/verify-forum-search-author-filter.mjs`.\n- Exact Forum tag and solved filter status:\n  `source_complete_execution_pending` under `FORUM-23B2F2`.\n- Exact Forum tag and solved contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-tag-solved-filter.json` and\n  `scripts/verify/verify-forum-search-tag-solved-filter.mjs`.\n",
)
replace_once(
    SEARCH_PLAN,
    "- Exact Forum author filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F1`.\n",
    "- Exact Forum author filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F1`.\n- Exact Forum tag and solved filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F2`.\n",
)
replace_once(
    SEARCH_PLAN,
    "20. Added the exact bounded Forum author filter on public projected author identity,\n    optional GraphQL plus additive native transport parity, pre-eligibility narrowing,\n    post-filter totals/facets/pagination, and active-scope pin suppression under\n    `FORUM-23B2F1`.\n",
    "20. Added the exact bounded Forum author filter on public projected author identity,\n    optional GraphQL plus additive native transport parity, pre-eligibility narrowing,\n    post-filter totals/facets/pagination, and active-scope pin suppression under\n    `FORUM-23B2F1`.\n21. Added exact bounded Forum tag and solved-state filters, parent-topic tag\n    projection for approved replies, additive GraphQL/native filter operations,\n    pre-eligibility intersection, post-authorization totals/facets/pagination, and\n    fail-closed legacy reply behavior under `FORUM-23B2F2`.\n",
)
replace_once(
    SEARCH_PLAN,
    "1. **Complete remaining Forum storefront query filters.** Add tag, locale, date,\n   solved, kind, channel/group and attachment-presence filters without moving owner",
    "1. **Complete remaining Forum storefront query filters.** Add locale, date, kind,\n   channel/group and attachment-presence filters without moving owner",
)

# Forum canonical ledger and task card.
old_ledger = "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`; FORUM-23B2E2 projects canonical Product channel allowlists and applies one fail-closed storefront predicate to Product-bearing Search paths; FORUM-23B2F1 adds an exact bounded Forum author filter before owner eligibility, visible totals, facets and pagination. Remaining tag, locale, date, solved, kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |"
new_ledger = "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`; FORUM-23B2E2 projects canonical Product channel allowlists and applies one fail-closed storefront predicate to Product-bearing Search paths; FORUM-23B2F1 adds an exact bounded Forum author filter; FORUM-23B2F2 adds exact bounded Forum tag and solved filters before owner eligibility, visible totals, facets and pagination. Remaining locale, date, kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |"
replace_once(FORUM_PLAN, old_ledger, new_ledger)

b2f1_tail = """- `forum-search-author-filter.json`,
  `forum-23b2f1-search-author-filter.md`, and
  `verify-forum-search-author-filter.mjs` lock the public-author source,
  transport, ordering, bounds, and compatibility contract while recording
  execution as pending.

### Compatibility and degraded mode
"""
b2f2_section = """- `forum-search-author-filter.json`,
  `forum-23b2f1-search-author-filter.md`, and
  `verify-forum-search-author-filter.mjs` lock the public-author source,
  transport, ordering, bounds, and compatibility contract while recording
  execution as pending.

### Delivered in `FORUM-23B2F2`

- the Forum-only GraphQL owner accepts optional bounded `tags` and nullable
  `solved` arguments; an additive GraphQL operation and native endpoint carry
  author/tag/solved filters without changing neutral Search inputs or shared DTOs;
- tag values are trimmed, case-sensitive exact values capped at ten entries and
  64 characters each; every requested tag must occur in the projected list;
- topics match `payload.tags`, approved replies match Forum-projected parent
  `payload.topic_tags`, and legacy replies missing that projection fail closed
  under an active tag scope until reindexed;
- solved topics match the presence or absence of `solution_reply_id`; replies match
  the exact current projected `is_solution` boolean;
- active author, tag and solved predicates intersect after the stable bounded raw
  snapshot and before exact Forum owner eligibility, visible totals, facets,
  offset and limit while preserving ranking order;
- categories and non-Forum rows do not match any active document filter, and
  query-rule pins remain disabled while any document filter is active;
- existing `ForumStorefrontSearch`, `ForumStorefrontSearchByAuthors`,
  `search/forum-storefront-search`, and
  `search/forum-storefront-search-by-authors` wire contracts remain unchanged;
- `forum-search-tag-solved-filter.json`,
  `forum-23b2f2-search-tag-solved-filter.md`, and
  `verify-forum-search-tag-solved-filter.mjs` lock the owner projection, exact
  semantics, ordering, compatibility and degraded-mode contract while recording
  execution and reindex evidence as pending.

### Compatibility and degraded mode
"""
replace_once(FORUM_PLAN, b2f1_tail, b2f2_section)
replace_once(
    FORUM_PLAN,
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1`. The Search-owned Product payload gains the",
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2`. The Search-owned Product payload gains the",
)
replace_once(
    FORUM_PLAN,
    "Forum author scope uses only the public projected author identity, excludes categories and\nmissing/redacted authors, and suppresses query-rule pins; the previous native endpoint and\nempty-author behavior remain unchanged. Admin/global Search behavior remains unchanged.",
    "Forum author scope uses only the public projected author identity, excludes categories and\nmissing/redacted authors, and suppresses query-rule pins. Tag/solved scopes use only\nForum-projected state, exclude categories, intersect with author scope, and suppress pins.\nLegacy replies without `topic_tags` fail closed for tag queries until reindexed; existing\nlegacy and author-only GraphQL/native operations remain unchanged. Admin/global Search\nbehavior remains unchanged.",
)
replace_once(
    FORUM_PLAN,
    "- add tag, locale, date, solved, kind, channel/group and attachment-presence\n  query filters;",
    "- add locale, date, kind, channel/group and attachment-presence query filters;",
)
replace_all(
    FORUM_PLAN,
    "node scripts/verify/verify-forum-search-author-filter.mjs\n",
    "node scripts/verify/verify-forum-search-author-filter.mjs\nnode scripts/verify/verify-forum-search-tag-solved-filter.mjs\n",
    2,
)
replace_once(
    FORUM_PLAN,
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1` source and contract records do not",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2` source and contract records do not",
)
replace_once(
    FORUM_PLAN,
    "14. continue `FORUM-23` with exact tag and solved filters, then locale, date,\n    kind, channel/group and attachment-presence filters before owner revision\n    ordering and reconciliation; execute B2D/F1 evidence with `LINK-FORUM-03` only\n    after ordering is stable;",
    "14. continue `FORUM-23` with locale, date, kind, channel/group and\n    attachment-presence filters before owner revision ordering and reconciliation;\n    execute B2D/F1/F2 evidence with `LINK-FORUM-03` only after ordering is stable;",
)

print("FORUM-23B2F2 source, transports and canonical plans synchronized.")
