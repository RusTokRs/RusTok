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


forum_plan = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    forum_plan,
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters; FORUM-23B2G1 replaces Forum Search inbox wall-clock/UUID execution ordering with a durable PostgreSQL ingest sequence and sequence watermarks. Remaining kind, channel/group and attachment-presence filters, owner-issued revision ordering/reconciliation and maintainer runtime evidence remain.",
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters; FORUM-23B2F4 narrows explicit Forum-only Search to topics and approved replies assigned to the trusted current request channel; FORUM-23B2G1 replaces Forum Search inbox wall-clock/UUID execution ordering with a durable PostgreSQL ingest sequence and sequence watermarks. Arbitrary channel/group filtering remains owner-contract blocked, kind waits on FORUM-22, attachment presence waits on FORUM-14, and owner-issued revision reconciliation plus maintainer runtime evidence remain.",
)
replace_once(
    forum_plan,
    "### Delivered in `FORUM-23B2G1`",
    '''### Delivered in `FORUM-23B2F4`

- an optional `current_channel_only` filter narrows explicit Forum-only Search to
  topics explicitly assigned to the trusted request channel and approved replies
  inheriting the same parent-topic assignment;
- Forum projects parent-topic channel slugs onto reply documents, and legacy reply
  rows without that projection fail closed until reindexed;
- the filter accepts no caller-selected channel slug, excludes global topics and
  categories, runs before exact Forum owner eligibility/totals/facets/pagination,
  and suppresses query-rule pins while active;
- topic channel updates publish the existing transactional `forum_topic`
  invalidation so topic and parent-derived reply channel projections rebuild
  together;
- existing wire signatures remain unchanged; additive
  `ForumStorefrontSearchByCurrentChannel` and
  `search/forum-storefront-search-by-current-channel` transports share the
  existing execution owner;
- arbitrary channel/group selection remains blocked on a separately authorized
  Forum owner contract, kind filtering waits on `FORUM-22`, and attachment
  presence waits on `FORUM-14`.

### Delivered in `FORUM-23B2G1`''',
)
replace_once(
    forum_plan,
    "requested/fallback locale scopes every explicit Forum transport and post-scan result; date\nwindows use Forum-projected timestamps and legacy topic/reply rows fail closed until\nreindexed. Existing legacy, author-only and B2F2 GraphQL/native wire signatures remain\nunchanged. Admin/global Search behavior remains unchanged.",
    "requested/fallback locale scopes every explicit Forum transport and post-scan result; date\nwindows use Forum-projected timestamps and legacy topic/reply rows fail closed until\nreindexed. Current-channel scope uses only trusted `RequestContext`, excludes global/category\nrows and fails closed for legacy replies without parent-topic channel projection. Existing\nlegacy, author-only and B2F2 GraphQL/native wire signatures remain unchanged. Admin/global\nSearch behavior remains unchanged.",
)
replace_once(
    forum_plan,
    "- add kind, channel/group and attachment-presence query filters;\n- add Forum-owner-issued monotonic projection revisions",
    "- add owner-safe arbitrary channel/group filtering only after an exact authorized\n  Forum owner contract exists; add kind after `FORUM-22` and attachment presence\n  after `FORUM-14`;\n- add Forum-owner-issued monotonic projection revisions",
)
content = read(forum_plan)
marker = "node scripts/verify/verify-forum-search-locale-date-filter.mjs\nnode scripts/verify/verify-forum-search-durable-ingest-sequence.mjs"
if content.count(marker) != 2:
    raise SystemExit(f"{forum_plan}: expected two current verification blocks")
content = content.replace(
    marker,
    "node scripts/verify/verify-forum-search-locale-date-filter.mjs\nnode scripts/verify/verify-forum-search-current-channel-filter.mjs\nnode scripts/verify/verify-forum-search-durable-ingest-sequence.mjs",
)
write(forum_plan, content)

search_plan = "crates/rustok-search/docs/implementation-plan.md"
replace_once(
    search_plan,
    "`FORUM-23B2G1` adds a durable PostgreSQL-issued Forum inbox ingest sequence.",
    '''`FORUM-23B2F4` adds an optional exact trusted-current-channel filter to the
same explicit Forum-only execution owner. Topics match their Forum-projected
`channel_slugs`; approved replies inherit `topic_channel_slugs` from the parent
topic. The filter accepts no arbitrary channel input, excludes global topics and
categories, runs before owner eligibility/totals/facets/pagination and suppresses
pins. Existing transports remain unchanged while additive current-channel
GraphQL/native operations carry the boolean. Arbitrary channel/group selection,
future topic kinds and attachment presence remain blocked on their Forum owner
contracts. Runtime and reindex evidence remain pending.

`FORUM-23B2G1` adds a durable PostgreSQL-issued Forum inbox ingest sequence.''',
)
replace_once(
    search_plan,
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.\n- Durable Forum inbox ingest-sequence status:",
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.\n- Trusted current-channel Forum filter status:\n  `source_complete_execution_pending` under `FORUM-23B2F4`.\n- Trusted current-channel contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-current-channel-filter.json` and\n  `scripts/verify/verify-forum-search-current-channel-filter.mjs`.\n- Durable Forum inbox ingest-sequence status:",
)
replace_once(
    search_plan,
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Durable Forum inbox ingest ordering",
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Trusted current-channel Forum filtering is `source_complete_execution_pending`\n  under `FORUM-23B2F4`.\n- Durable Forum inbox ingest ordering",
)
replace_once(
    search_plan,
    "23. Added a PostgreSQL-issued immutable Forum inbox ingest sequence, deterministic\n    existing-row backfill",
    "23. Added trusted current-channel Forum narrowing, parent-topic channel projection\n    for approved replies, additive transport parity and transactional topic-update\n    invalidation under `FORUM-23B2F4`.\n24. Added a PostgreSQL-issued immutable Forum inbox ingest sequence, deterministic\n    existing-row backfill",
)
replace_once(
    search_plan,
    "1. **Complete remaining Forum storefront query filters.** Add kind,\n   channel/group and attachment-presence filters without moving owner\n   authorization into Search. **Done when:** GraphQL/native Forum-only Search expose\n   the same bounded filter contract and every owner-sensitive result still passes\n   exact post-retrieval eligibility.",
    "1. **Complete owner-backed Forum storefront query filters.** Add arbitrary\n   channel/group selection only through an exact authorized Forum owner contract;\n   add topic kinds after `FORUM-22` and attachment presence after `FORUM-14`.\n   **Done when:** no caller-selected audience identifier or owner policy is copied\n   into Search and every result still passes exact post-retrieval eligibility.",
)
write(search_plan, content if False else read(search_plan))
