from pathlib import Path

path = Path("crates/rustok-forum/docs/implementation-plan.md")
text = path.read_text()

replacements = [
    ("last_reviewed: 2026-07-30", "last_reviewed: 2026-07-31"),
    (
        "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution. Topic/reply Search eligibility, trusted channel authority, remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |",
        "| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination. Trusted channel authority, remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |",
    ),
    (
        """- `forum-search-storefront-scope.json`,
  `forum-23b2c-storefront-search-scope.md`, and
  `verify-forum-search-storefront-scope.mjs` lock the cross-owner and transport
  boundary while recording execution as pending.

### Compatibility and degraded mode
""",
        """- `forum-search-storefront-scope.json`,
  `forum-23b2c-storefront-search-scope.md`, and
  `verify-forum-search-storefront-scope.mjs` lock the cross-owner and transport
  boundary while recording execution as pending.

### Delivered in `FORUM-23B2D`

- `StorefrontSearchResultEligibilityPort` is a second neutral Search-owned
  optional contract; the server remains the only adapter importing both Search
  and Forum and publishes the exact Forum owner implementation;
- Search scans the existing Forum-only query from offset zero in bounded 50-row
  pages, rejects a raw result set above 100 rows, and fails closed when the raw
  total changes, a continuation page does not advance or a raw row repeats;
- `ForumSearchResultEligibilityService` batch-loads current approved reply-to-topic
  ownership and reuses `ForumTopicAudienceVisibilityService` for every distinct
  topic, including open state, route channel, inherited category layers,
  topic-local narrowing, roles, Forum trust, Channel, Groups and explicit
  allow/deny;
- missing, stale, closed, denied or non-approved topic/reply candidates are omitted
  without an existence oracle, while missing owner composition, disabled Forum,
  invalid owner subsets and unresolved required facts fail closed;
- Search preserves the raw ranking order of authorized rows and computes visible
  totals, facets, offset and limit before query rules or transport mapping;
- GraphQL and native transports share the same execution owner, while mixed,
  unspecified, Product, Blog, Content and Forum-without-category Search paths
  remain unchanged;
- `forum-search-result-eligibility.json`,
  `forum-23b2d-search-result-eligibility.md`, and
  `verify-forum-search-result-eligibility.mjs` lock the owner, bound, transport and
  post-authorization pagination contract while recording execution as pending.

### Compatibility and degraded mode
""",
    ),
    (
        """No migration, backfill, Search query shape, Forum projection shape, dependency or
`Cargo.lock` change is required by `FORUM-23B2A/B2B/B2C`. The ordinary GraphQL
`storefrontSearch` field and native `search/storefront-search` endpoint remain
unchanged; exact category behavior remains available for mixed, unspecified and
non-Forum-only requests. Search-disabled behavior and core Forum reads remain
unchanged.

The explicit visibility-safe Forum-only field requires the neutral owner port and
does not silently degrade to exact category filtering when the owner is absent.
Public evaluation does not require optional audience facts. Authenticated trust,
Channel, or Groups selectors fail closed only when a required owner fact remains
unresolved. Product category identifiers are never expanded through Forum policy.
""",
        """No migration, backfill, Search query shape, Forum projection shape, dependency,
public DTO or `Cargo.lock` change is required by `FORUM-23B2A/B2B/B2C/B2D`.
The ordinary GraphQL `storefrontSearch` field and native
`search/storefront-search` endpoint remain unchanged; exact category behavior
remains available for mixed, unspecified and non-Forum-only requests.
Search-disabled behavior and core Forum reads remain unchanged.

The explicit visibility-safe Forum-only field now requires both neutral owner
ports and does not silently degrade to exact category filtering or raw Search
results when either owner is absent. Public evaluation does not require optional
audience facts. Authenticated trust, Channel, or Groups selectors fail closed
only when a required owner fact remains unresolved. Raw Forum-only result sets
above 100 rows fail with a request to narrow the query or category scope rather
than creating partial pagination or an unbounded owner-call chain. Product
category identifiers are never expanded or filtered through Forum policy.
""",
    ),
    (
        """- apply exact topic-local narrowing and reply authorization to Search result
  eligibility;
- derive trusted channel authority consistently for every storefront Search
  result predicate;
""",
        """- derive trusted channel authority consistently for every storefront Search
  result predicate, especially Product visibility;
""",
    ),
    (
        """cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
""",
        """cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search storefront_result_eligibility -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
node scripts/verify/verify-forum-search-result-eligibility.mjs
""",
    ),
    (
        "The `FORUM-23B2A/B2B/B2C` source and contract records do not claim successful",
        "The `FORUM-23B2A/B2B/B2C/B2D` source and contract records do not claim successful",
    ),
    (
        """cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
""",
        """cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search storefront_result_eligibility -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
node scripts/verify/verify-forum-search-result-eligibility.mjs
""",
    ),
    (
        """14. continue `FORUM-23` with topic-local audience narrowing and reply Search
    eligibility after the delivered explicit Forum-only storefront path, then
    trusted channel authority, remaining filters, owner revision ordering and
    reconciliation;
""",
        """14. continue `FORUM-23` with trusted channel authority across Forum and Product
    storefront Search, then remaining filters, owner revision ordering and
    reconciliation; execute B2D evidence with `LINK-FORUM-03` only after ordering
    is stable;
""",
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"expected one canonical replacement, found {count}: {old[:100]!r}"
        )
    text = text.replace(old, new, 1)

path.write_text(text)
