from pathlib import Path

FORUM_PLAN = Path("crates/rustok-forum/docs/implementation-plan.md")
VERIFIER = Path("scripts/verify/verify-forum-search-author-filter.mjs")


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old[:160]}")
    path.write_text(text.replace(old, new, 1))


replace_once(
    FORUM_PLAN,
    "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`; FORUM-23B2E2 projects canonical Product channel allowlists and applies one fail-closed storefront predicate to Product-bearing Search paths. Remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |",
    "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`; FORUM-23B2E2 projects canonical Product channel allowlists and applies one fail-closed storefront predicate to Product-bearing Search paths; FORUM-23B2F1 adds an exact bounded Forum author filter before owner eligibility, visible totals, facets and pagination. Remaining tag, locale, date, solved, kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |",
)

b2e2_tail = """- `forum-search-product-channel-visibility.json`,
  `forum-23b2e2-product-channel-visibility.md`, and
  `verify-forum-search-product-channel-visibility.mjs` lock the projection,
  reconciliation and surface contract while recording execution as pending.

### Compatibility and degraded mode
"""
b2f1_section = """- `forum-search-product-channel-visibility.json`,
  `forum-23b2e2-product-channel-visibility.md`, and
  `verify-forum-search-product-channel-visibility.mjs` lock the projection,
  reconciliation and surface contract while recording execution as pending.

### Delivered in `FORUM-23B2F1`

- the explicit Forum-only GraphQL and native paths accept a separate author-ID
  argument capped at ten raw UUID values without changing neutral
  `SearchPreviewInput`, `SearchQuery`, or the shared storefront filter DTO;
- Search matches only the existing Forum-owned public
  `payload.author.user_id` projection for topics and approved replies; categories,
  non-Forum rows, and missing, denied, redacted, or malformed author summaries do
  not match an active author scope;
- the stable raw Forum candidate snapshot and 100-row bound are resolved before
  author narrowing, so a broad query cannot bypass the existing owner-call cap;
- exact author narrowing runs before topic/reply owner eligibility, visible totals,
  facets, offset, and limit while preserving the original ranking order;
- query-rule pins are disabled while an author filter is active because the pin
  loader has no author argument and must not reintroduce an out-of-scope document;
- ordinary storefront, mixed, Product, admin preview, and admin global Search
  remain unchanged, and existing explicit Forum calls pass an empty author list;
- `forum-search-author-filter.json`,
  `forum-23b2f1-search-author-filter.md`, and
  `verify-forum-search-author-filter.mjs` lock the public-author source,
  transport, ordering, bounds, and compatibility contract while recording
  execution as pending.

### Compatibility and degraded mode
"""
replace_once(FORUM_PLAN, b2e2_tail, b2f1_section)

replace_once(
    FORUM_PLAN,
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2`. The Search-owned Product payload gains the",
    "`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1`. The Search-owned Product payload gains the",
)

replace_once(
    FORUM_PLAN,
    "malformed explicit owner values remain hidden until Product is corrected. Admin/global\nSearch behavior remains unchanged.\n",
    "malformed explicit owner values remain hidden until Product is corrected. An active\nForum author scope uses only the public projected author identity, excludes categories and\nmissing/redacted authors, and suppresses query-rule pins; an empty author list preserves\nthe previous explicit Forum behavior. Admin/global Search behavior remains unchanged.\n",
)

replace_once(
    FORUM_PLAN,
    "- add author, tag, locale, date, solved, kind, channel/group and\n  attachment-presence query filters;",
    "- add tag, locale, date, solved, kind, channel/group and attachment-presence\n  query filters;",
)

replace_once(
    FORUM_PLAN,
    "cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search storefront_channel_authority -- --nocapture",
    "cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search forum_document_filters -- --nocapture\ncargo test -p rustok-search storefront_channel_authority -- --nocapture",
)

replace_once(
    FORUM_PLAN,
    "node scripts/verify/verify-forum-search-product-channel-visibility.mjs\n",
    "node scripts/verify/verify-forum-search-product-channel-visibility.mjs\nnode scripts/verify/verify-forum-search-author-filter.mjs\n",
)

replace_once(
    FORUM_PLAN,
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2` source and contract records do not\nclaim successful runtime verification until the maintainer runs the commands above.",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1` source and contract records do not\nclaim successful runtime verification until the maintainer runs the commands above.",
)

# The complete release verification block repeats the focused commands once.
replace_once(
    FORUM_PLAN,
    "cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture",
    "cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search forum_document_filters -- --nocapture\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture",
)
replace_once(
    FORUM_PLAN,
    "node scripts/verify/verify-forum-search-result-eligibility.mjs\ncargo check -p rustok-search --features graphql --all-targets",
    "node scripts/verify/verify-forum-search-result-eligibility.mjs\nnode scripts/verify/verify-forum-search-author-filter.mjs\ncargo check -p rustok-search --features graphql --all-targets",
)

replace_once(
    FORUM_PLAN,
    "14. continue `FORUM-23` with trusted channel authority across Forum and Product\n    storefront Search, then remaining filters, owner revision ordering and\n    reconciliation; execute B2D evidence with `LINK-FORUM-03` only after ordering\n    is stable;",
    "14. continue `FORUM-23` with exact tag and solved filters, then locale, date,\n    kind, channel/group and attachment-presence filters before owner revision\n    ordering and reconciliation; execute B2D/F1 evidence with `LINK-FORUM-03` only\n    after ordering is stable;",
)

replace_once(
    VERIFIER,
    '    "exact Forum author filter",',
    '    "exact bounded Forum author filter",',
)
replace_once(
    VERIFIER,
    '    "exact Forum author filter",',
    '    "exact bounded Forum author filter",',
)

print("FORUM-23B2F1 canonical plan and guardrail synchronized.")
