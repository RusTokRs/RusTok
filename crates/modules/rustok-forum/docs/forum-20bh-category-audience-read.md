# FORUM-20BH — exact category audience reads

`FORUM-20BH` publishes a canonical category-read owner that composes the
existing public/authenticated visibility floor with every inherited richer
category audience layer before category content reaches a transport.

## Delivered boundary

- `ForumCategoryAudienceVisibilityService` evaluates the inherited base
  visibility floor first, then every normalized category audience layer from
  root to target.
- Explicit deny, explicit allow and role decisions remain local. Trust,
  Channel, and Groups constraints use the optional host-published audience
  facts port and fail closed when required facts are unavailable.
- `ForumCategoryAudienceReadService` owns selected-category, paginated list,
  storefront list, and authenticated tree reads.
- Selected category reads return the same absent result for missing, foreign,
  base-floor-denied, and richer-audience-denied categories.
- Category list pagination scans a bounded base sequence, filters it through the
  exact owner, and derives `items` plus `total` from the same allowed sequence.
- Category trees are filtered before output. Denied nodes and their inherited
  descendants are pruned, then child counts, `total_nodes`, and `max_depth` are
  recomputed from the allowed tree.
- REST category get/list/tree reads use trusted REST contexts derived from
  authenticated extensions.
- Existing GraphQL `forumCategories`, `forumCategory`,
  `forumStorefrontCategories`, and `forumCategoryTree` fields use the exact
  owner without changing their names or response types.
- Native storefront categories use the exact owner and reuse the host facts
  port. A requested or topic-derived category is checked through the exact
  selected-category owner independently of the first rendered category page.
- The canonical GraphQL query module is now `query_runtime.rs`. The previous
  `query.rs` remains uncompiled as a temporary source-verifier compatibility
  snapshot.

## Compatibility

No migration, dependency, REST route, GraphQL field name, native server-fn
endpoint, request DTO, response DTO, storefront model, topic/reply path, or
mark-read path changes.

The GraphQL storefront adapter continues to request
`forumStorefrontCategories`; that existing field is now backed by exact
category authorization.

## Explicitly not delivered

`FORUM-20BH` does not migrate search/index, SEO target resolution, deep links,
or visibility-scoped category/all-read commands. It also does not remove the
uncompiled legacy GraphQL snapshot because older source verifiers still inspect
that path.

The canonical `implementation-plan.md` and `CRATE_API.md` are not replaced
through the GitHub contents API in this slice. Their conflict-safe
repository-local synchronization debt remains explicit in the machine contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum category_read_transport -- --nocapture
cargo test -p rustok-forum --test category_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test category_owner_visibility_sqlite -- --nocapture
node scripts/verify/verify-forum-reply-legacy-cutover.mjs
node scripts/verify/verify-forum-category-audience-read.mjs
cargo xtask module validate forum
```
