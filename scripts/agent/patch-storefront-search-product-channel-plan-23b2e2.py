from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old}")
    target.write_text(text.replace(old, new, 1))


forum_plan = "crates/rustok-forum/docs/implementation-plan.md"
search_plan = "crates/rustok-search/docs/implementation-plan.md"
previous_verifier = "scripts/verify/verify-forum-search-trusted-channel-authority.mjs"

replace_once(
    forum_plan,
    """| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`. Product channel projection/predicates, remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |
""",
    """| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`; FORUM-23B2E2 projects canonical Product channel allowlists and applies one fail-closed storefront predicate to Product-bearing Search paths. Remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |
""",
)

replace_once(
    forum_plan,
    """- `forum-search-trusted-channel-authority.json`,
  `forum-23b2e1-trusted-channel-authority.md`, and
  `verify-forum-search-trusted-channel-authority.mjs` lock the source boundary and
  record maintainer execution as pending.

### Compatibility and degraded mode
""",
    """- `forum-search-trusted-channel-authority.json`,
  `forum-23b2e1-trusted-channel-authority.md`, and
  `verify-forum-search-trusted-channel-authority.mjs` lock the source boundary and
  record maintainer execution as pending.

### Delivered in `FORUM-23B2E2`

- Search projects Product-owned
  `metadata.channel_visibility.allowed_channel_slugs` into the Search-owned
  Product payload without importing Product or Channel policy services;
- absence of the owner visibility object preserves the canonical global Product
  meaning as an empty array, canonical arrays are retained, and malformed explicit
  values remain non-arrays so storefront evaluation fails closed;
- `PgSearchEngine::search_storefront` applies one Product predicate before FTS or
  typo ranking, so result rows, totals, facets and attribute-filtered queries use
  the same trusted channel decision;
- storefront query-rule pins recheck Product payload eligibility and document
  suggestions reuse the SQL predicate, while query-text suggestions remain
  aggregate strings rather than Product result exposure;
- ordinary and Forum-only GraphQL/native Search preserve the exact
  `TrustedStorefrontChannel` through every bounded page and post-query rule step;
- Search bootstrap detects tenant Product documents with a missing or malformed
  allowlist projection and runs the existing product-scope rebuild; legacy drift is
  hidden before repair and does not require a database migration or manual backfill;
- admin preview/global Search retain their existing non-storefront execution path;
- `forum-search-product-channel-visibility.json`,
  `forum-23b2e2-product-channel-visibility.md`, and
  `verify-forum-search-product-channel-visibility.mjs` lock the projection,
  reconciliation and surface contract while recording execution as pending.

### Compatibility and degraded mode
""",
)

replace_once(
    forum_plan,
    """No migration, backfill, Search query shape, Forum projection shape, dependency,
public DTO or `Cargo.lock` change is required by
`FORUM-23B2A/B2B/B2C/B2D/B2E1`.
""",
    """No database migration, manual backfill, Search query shape, Forum projection
shape, dependency, public DTO or `Cargo.lock` change is required by
`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2`. The Search-owned Product payload gains the
channel allowlist projection, and existing drift is repaired by an automatic
product-scope rebuild during Search bootstrap.
""",
)

replace_once(
    forum_plan,
    """fails closed. Product channel allowlist projection and base-result filtering remain
open and are not claimed by `FORUM-23B2E1`.
""",
    """fails closed. `FORUM-23B2E2` applies the trusted channel to Product rows,
totals, facets, typo fallback, query-rule pins and document suggestions. Missing or
malformed Product projections remain hidden until the Search-owned product rebuild
repairs them; admin/global Search behavior remains unchanged.
""",
)

replace_once(
    forum_plan,
    """- project and backfill canonical Product channel allowlists, then apply the
  trusted `RequestContext` channel consistently to base results, totals, facets,
  typo fallback, suggestions, query rules and attribute operations;
- add author, tag, locale, date, solved, kind, channel/group and
""",
    """- add author, tag, locale, date, solved, kind, channel/group and
""",
)

replace_once(
    forum_plan,
    """cargo test -p rustok-search storefront_channel_authority -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
""",
    """cargo test -p rustok-search storefront_channel_authority -- --nocapture
cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture
cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
""",
)

replace_once(
    forum_plan,
    """node scripts/verify/verify-forum-search-trusted-channel-authority.mjs
cargo check -p rustok-search --features graphql --all-targets
""",
    """node scripts/verify/verify-forum-search-trusted-channel-authority.mjs
node scripts/verify/verify-forum-search-product-channel-visibility.mjs
cargo check -p rustok-search --features graphql --all-targets
""",
)

replace_once(
    forum_plan,
    """The `FORUM-23B2A/B2B/B2C/B2D/B2E1` source and contract records do not claim
successful runtime verification until the maintainer runs the commands above.
""",
    """The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2` source and contract records do not
claim successful runtime verification until the maintainer runs the commands above.
""",
)

replace_once(
    search_plan,
    """`FORUM-23B2E1` closes the transport half of storefront channel authority.
GraphQL and native storefront Search now derive channel ID and slug from trusted
`RequestContext`; caller-provided `channel_id` is now only a compatibility assertion
and a mismatched value fails closed. The same owner is used by ordinary and
Forum-only Search, and the shared Forum execution path revalidates tenant and
channel context. Product channel visibility remains blocked: `PgSearchEngine`
still applies channel only to attribute filters, facets, and sorting, while product
Search documents omit the canonical
`metadata.channel_visibility.allowed_channel_slugs` projection.
""",
    """`FORUM-23B2E1/B2E2` close the storefront channel authority and Product visibility
source boundary. GraphQL and native storefront Search derive channel ID and slug
from trusted `RequestContext`; caller-provided `channel_id` is only a compatibility
assertion. Search projects Product-owned
`metadata.channel_visibility.allowed_channel_slugs`, hides missing or malformed
projections, and applies one storefront-only predicate before FTS or typo ranking.
Rows, totals, facets, attribute-filtered queries, query-rule pins and document
suggestions therefore share the trusted channel decision. Existing Product drift
triggers the Search-owned product-scope rebuild, while admin preview/global Search
retain the previous non-storefront path. Runtime evidence remains pending.
""",
)

replace_once(
    search_plan,
    """- Trusted channel contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-trusted-channel-authority.json` and
  `scripts/verify/verify-forum-search-trusted-channel-authority.mjs`.
""",
    """- Trusted channel contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-trusted-channel-authority.json` and
  `scripts/verify/verify-forum-search-trusted-channel-authority.mjs`.
- Product channel visibility status:
  `source_complete_execution_pending` under `FORUM-23B2E2`.
- Product channel visibility contract and guardrail:
  `crates/rustok-forum/contracts/forum-search-product-channel-visibility.json` and
  `scripts/verify/verify-forum-search-product-channel-visibility.mjs`.
""",
)

replace_once(
    search_plan,
    """- Trusted storefront channel authority is `source_complete_execution_pending`
  under `FORUM-23B2E1`.
- Product channel visibility remains blocked.
""",
    """- Trusted storefront channel authority is `source_complete_execution_pending`
  under `FORUM-23B2E1`.
- Product channel visibility is `source_complete_execution_pending` under
  `FORUM-23B2E2`.
""",
)

replace_once(
    search_plan,
    """18. Added the Search-owned trusted storefront channel authority and bound ordinary
    plus Forum-only GraphQL/native Search to middleware `RequestContext`; the
    legacy public `channel_id` is assertion-only under `FORUM-23B2E1`.
""",
    """18. Added the Search-owned trusted storefront channel authority and bound ordinary
    plus Forum-only GraphQL/native Search to middleware `RequestContext`; the
    legacy public `channel_id` is assertion-only under `FORUM-23B2E1`.
19. Projected canonical Product channel allowlists, added fail-closed product-scope
    bootstrap repair, and applied one storefront predicate to FTS, typo fallback,
    rows, totals, facets, query-rule pins and document suggestions under
    `FORUM-23B2E2`.
""",
)

replace_once(
    search_plan,
    """1. **Close Product channel visibility projection and predicate.** Reuse the
   trusted `RequestContext` authority delivered by `FORUM-23B2E1`, denormalize
   canonical product channel visibility into Search-owned documents, backfill
   existing documents safely, and make base results, totals, facets, typo fallback,
   suggestions, query rules, and attribute operations use one fail-closed channel
   predicate. **Done when:** a restricted product is absent from every Search
   response outside its allowed channel.
""",
    """1. **Complete Forum storefront query filters.** Add author, tag, locale, date,
   solved, kind, channel/group and attachment-presence filters without moving owner
   authorization into Search. **Done when:** GraphQL/native Forum-only Search expose
   the same bounded filter contract and every owner-sensitive result still passes
   exact post-retrieval eligibility.
""",
)

replace_once(
    search_plan,
    """- `cargo test -p rustok-search storefront_result_eligibility -- --nocapture`
- `cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture`
""",
    """- `cargo test -p rustok-search storefront_result_eligibility -- --nocapture`
- `cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture`
- `cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture`
- `cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture`
""",
)

replace_once(
    search_plan,
    """- `node scripts/verify/verify-forum-search-result-eligibility.mjs`
- `npm run verify:search:canonical-url`
""",
    """- `node scripts/verify/verify-forum-search-result-eligibility.mjs`
- `node scripts/verify/verify-forum-search-product-channel-visibility.mjs`
- `npm run verify:search:canonical-url`
""",
)

replace_once(
    previous_verifier,
    """// B2E1 intentionally does not claim the Product projection/filter work.
rejectAll(
  projector,
  ["allowed_channel_slugs'", "channel_visibility'"],
  `${paths.projector} B2E1 non-claim`,
);
if (engine.includes("allowed_channel_slugs")) {
  failures.push(`${paths.engine}: product visibility predicate moved into B2E1 unexpectedly`);
}

""",
    """// Later slices may add Product visibility while B2E1 continues to guard only
// trusted transport authority and assertion semantics.

""",
)

print("FORUM-23B2E2 plans and predecessor verifier synchronized")
