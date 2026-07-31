from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    write(path, content.replace(old, new, 1))


# Every explicit Forum GraphQL call uses the exact-locale owner; operation wire shapes stay additive/compatible.
path = "crates/rustok-search/src/graphql/forum_storefront.rs"
content = read(path)
content = content.replace(
    "    StorefrontSearchTransport, execute_forum_storefront_search,\n    execute_forum_storefront_search_with_date_window, resolve_trusted_storefront_channel_input,",
    "    StorefrontSearchTransport, execute_forum_storefront_search_with_date_window,\n    resolve_trusted_storefront_channel_input,",
)
old = '''        let execution = if published_from.is_some() || published_to.is_some() {
            execute_forum_storefront_search_with_date_window(
                db,
                category_scope_port,
                result_eligibility_port,
                ForumStorefrontSearchDateWindowRequest {
                    request,
                    published_from,
                    published_to,
                },
            )
            .await
        } else {
            execute_forum_storefront_search(
                db,
                category_scope_port,
                result_eligibility_port,
                request,
            )
            .await
        }
        .map_err(map_execution_error)?;'''
new = '''        let execution = execute_forum_storefront_search_with_date_window(
            db,
            category_scope_port,
            result_eligibility_port,
            ForumStorefrontSearchDateWindowRequest {
                request,
                published_from,
                published_to,
            },
        )
        .await
        .map_err(map_execution_error)?;'''
if content.count(old) != 1:
    raise SystemExit(f"{path}: GraphQL execution anchor drift")
write(path, content.replace(old, new, 1))

# Keep all legacy native endpoint signatures but delegate their internal helper to the exact-locale owner.
path = "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs"
content = read(path)
start = content.index("async fn execute_forum_storefront_search_native(")
end = content.index("async fn execute_forum_storefront_search_date_window_native(", start)
wrapper = '''async fn execute_forum_storefront_search_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
) -> Result<SearchPreviewPayload, ServerFnError> {
    execute_forum_storefront_search_date_window_native(
        query,
        locale,
        preset_key,
        filters,
        author_ids,
        tags,
        solved,
        None,
        None,
    )
    .await
}

'''
write(path, content[:start] + wrapper + content[end:])

# Restore unrelated health-boundary commentary removed by a previous formatting correction.
replace_once(
    "crates/rustok-search/src/lib.rs",
    "    async fn health(&self) -> HealthStatus {\n        HealthStatus::Degraded\n    }",
    "    async fn health(&self) -> HealthStatus {\n        // Module-level health has no host AppContext, so it cannot validate\n        // search_documents, indexing lag, query plans or connector reachability.\n        // The server readiness layer owns the concrete search backend/lag checks.\n        HealthStatus::Degraded\n    }",
)

# Search and owner notes must describe shared exact-locale execution, not unchanged internal owners.
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    "`FORUM-23B2F3` adds an additive Forum-only locale/date execution owner while\nleaving the legacy, author-only and B2F2 execution owners unchanged.",
    "`FORUM-23B2F3` adds a shared Forum-only locale/date execution owner while\npreserving the legacy, author-only and B2F2 GraphQL/native wire operations.",
)
replace_once(
    "crates/rustok-forum/docs/forum-23b2f3-search-locale-date-filter.md",
    "Locale-only execution preserves Forum categories, topics and replies and keeps\nquery-rule pins enabled. Existing legacy, author-only and B2F2 filter execution\nowners remain unchanged for rolling compatibility.",
    "Locale-only execution preserves Forum categories, topics and replies and keeps\nquery-rule pins enabled. Existing legacy, author-only and B2F2 GraphQL/native wire\noperations remain unchanged, but all delegate to the shared exact-locale owner.",
)

# Canonical Forum ledger and B2F3 task card.
forum_plan = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    forum_plan,
    "FORUM-23B2F2 adds exact bounded Forum tag and solved filters before owner eligibility, visible totals, facets and pagination. Remaining locale, date, kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
    "FORUM-23B2F2 adds exact bounded Forum tag and solved filters; FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters before owner eligibility, visible totals, facets and pagination. Remaining kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
)
replace_once(
    forum_plan,
    "### Compatibility and degraded mode\n\nNo database migration, manual backfill, Search query shape, dependency, public",
    "### Delivered in `FORUM-23B2F3`\n\n- every explicit Forum-only GraphQL/native wire operation delegates to one shared\n  execution owner that normalizes the requested locale or tenant fallback and uses\n  it for PostgreSQL FTS/typo scope, category scope, owner eligibility and a\n  post-scan exact result assertion; missing or mismatched locale fails closed;\n- locale-only execution retains category, topic and reply results and query-rule\n  pins; no multi-locale candidate union is introduced;\n- topics and approved replies project Forum-owned creation time as UTC RFC3339\n  `payload.published_at`; legacy rows without it fail closed for date windows until\n  reindexed;\n- optional inclusive `published_from` / `published_to` bounds accept RFC3339, may\n  be one-sided, reject reversed ranges, exclude categories and fail closed on\n  malformed projected timestamps;\n- date narrowing intersects author/tag/solved after the stable bounded raw snapshot\n  and before exact Forum owner eligibility, visible totals, facets, offset and limit;\n- existing legacy, author-only and B2F2 GraphQL/native wire signatures remain\n  unchanged; date windows use additive `ForumStorefrontSearchByDateWindow` and\n  `search/forum-storefront-search-by-date-window` transports;\n- `forum-search-locale-date-filter.json`,\n  `forum-23b2f3-search-locale-date-filter.md`, and\n  `verify-forum-search-locale-date-filter.mjs` lock locale, projection, range,\n  ordering, compatibility and degraded-mode behavior while execution/reindex\n  evidence remains pending.\n\n### Compatibility and degraded mode\n\nNo database migration, manual backfill, Search query shape, dependency, public",
)
replace_once(
    forum_plan,
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2`. `FORUM-23B2F2` extends the\nForum reply projection payload with parent-topic `topic_tags`; legacy reply rows\nrequire reindex before positive tag matches and fail closed until repaired.",
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3`. `FORUM-23B2F2` extends\nthe Forum reply projection payload with parent-topic `topic_tags`; `FORUM-23B2F3`\nextends topic and reply payloads with Forum-owned `published_at`. Legacy rows\nrequire reindex before positive tag/date matches and fail closed until repaired.",
)
replace_once(
    forum_plan,
    "Legacy replies without `topic_tags` fail closed for tag queries until reindexed; existing\nlegacy and author-only GraphQL/native operations remain unchanged. Admin/global Search\nbehavior remains unchanged.\n\n### Remaining scope\n\n- add locale, date, kind, channel/group and attachment-presence query filters;",
    "Legacy replies without `topic_tags` fail closed for tag queries until reindexed. Exact\nrequested/fallback locale scopes every explicit Forum transport and post-scan result; date\nwindows use Forum-projected timestamps and legacy topic/reply rows fail closed until\nreindexed. Existing legacy, author-only and B2F2 GraphQL/native wire signatures remain\nunchanged. Admin/global Search behavior remains unchanged.\n\n### Remaining scope\n\n- add kind, channel/group and attachment-presence query filters;",
)
content = read(forum_plan)
marker = "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\ncargo check -p rustok-search"
if content.count(marker) != 2:
    raise SystemExit(f"{forum_plan}: expected two verification markers")
content = content.replace(
    marker,
    "node scripts/verify/verify-forum-search-tag-solved-filter.mjs\nnode scripts/verify/verify-forum-search-locale-date-filter.mjs\ncargo check -p rustok-search",
)
content = content.replace(
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2` source and contract records do not",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3` source and contract records do not",
    1,
)
content = content.replace(
    "14. continue `FORUM-23` with locale, date, kind, channel/group and\n    attachment-presence filters before owner revision ordering and reconciliation;\n    execute B2D/F1/F2 evidence with `LINK-FORUM-03` only after ordering is stable;",
    "14. continue `FORUM-23` with kind, channel/group and attachment-presence\n    filters before owner revision ordering and reconciliation; execute B2D/F1/F2/F3\n    evidence with `LINK-FORUM-03` only after ordering is stable;",
    1,
)
write(forum_plan, content)

# Strengthen the guardrail around shared exact-locale transport execution.
path = "scripts/verify/verify-forum-search-locale-date-filter.mjs"
content = read(path)
content = content.replace(
    '  "execute_forum_storefront_search_with_date_window",\n], paths.graphqlOwner);',
    '  "execute_forum_storefront_search_with_date_window",\n], paths.graphqlOwner);\nif (graphqlOwner.includes("execute_forum_storefront_search(")) {\n  failures.push(`${paths.graphqlOwner}: GraphQL Forum transport must use the exact-locale owner`);\n}',
)
content = content.replace(
    '  "ForumStorefrontSearchDateWindowRequest",\n], paths.nativeAdapter);',
    '  "ForumStorefrontSearchDateWindowRequest",\n  "execute_forum_storefront_search_date_window_native(",\n], paths.nativeAdapter);',
)
write(path, content)
