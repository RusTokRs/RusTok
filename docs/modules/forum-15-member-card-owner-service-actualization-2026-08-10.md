# FORUM-15C member-card owner service actualization — 2026-08-10

Status: `source-ready / shared-owner-service / graphql-adapter-thin / storefront-integration-open / maintainer-execution-open`

## Fresh cursor

This slice began from `main@1d557e8ce1108287befd80c340a534752d3fe0d2`, the FORUM-15B merge. During preparation `main` first advanced by three commits to `1de7925b1055917e5bc37a379c000e0fe611bb7f`, then advanced once more to `676861d89227b18ddbe51074de6ff3d38f2be8f2` before the final merge gate.

The first intervening compare touched server composition/tests, Commerce, Distribution, Product, Page Builder sources/docs and one `crates/rustok-forum/admin/build.rs` line. The later single commit was Commerce/order-only. Neither movement touched the FORUM-15C GraphQL/member-card/user-stats files.

An attempted whole-tree rebase onto the first fresh main was rejected during static review because it would have reverted concurrent changes. The feature was reset and replayed only through its intended files. For the final main movement, the feature tree is rebuilt from the current main base tree with only the intended FORUM-15C blobs overlaid. The final branch must have no unrelated files and `behind 0` before merge.

FORUM-15 remains `in_progress`. FORUM-15B introduced the bounded authenticated GraphQL member-card read, but its Profiles presentation and Forum statistics composition still lived inside the GraphQL transport. The real Forum storefront uses a dual-path transport: GraphQL for headless/CSR and a native server adapter for SSR/hydrate. The storefront package intentionally does not depend directly on `rustok-profiles`.

Therefore directly wiring the 15B GraphQL helper into native storefront UI would either create an HTTP self-call or add a new Profiles dependency to the storefront package. This slice removes that architectural dead end before UI integration.

## Shared Forum owner contract

`rustok_forum::services::user_stats` now exposes:

- `MAX_FORUM_MEMBER_CARD_USER_IDS = 100`;
- `ForumMemberCardAudience` with anonymous, authenticated and trusted-service audiences;
- `ForumMemberStats`;
- `ForumMemberCard`;
- `ForumMemberCardService`.

The service is included through the already-public `services::user_stats` module, so no large crate-root export rewrite is required.

### Bounded request admission

`ForumMemberCardService::normalize_user_ids`:

- rejects more than 100 requested IDs before owner reads;
- rejects nil user IDs;
- deduplicates while preserving first-request order;
- permits an empty request and returns an empty result.

`read_for_audience` additionally rejects a nil tenant and rejects nil authenticated/trusted actor identities.

## Privacy/ownership order

The public `read_for_audience` method owns the safe cross-owner composition order:

1. convert the Forum audience descriptor to the Profiles-owned `ProfileAccessAudience`;
2. call `ProfilePresentationService::for_audience(...).find_profile_summaries(...)` once for the bounded deduplicated IDs;
3. keep only IDs actually returned by Profiles presentation;
4. query `forum_user_stats` once for those visible IDs only;
5. zero-fill missing Forum statistic rows;
6. return cards in first-request order.

A Forum statistics row cannot manufacture or reveal a profile that Profiles presentation did not admit.

Forum does not copy handle/display-name/avatar/privacy state into Forum persistence and does not read Profiles or Social Graph private tables.

Profiles presentation errors retain the Profiles owner `code()` and `is_retryable()` classification when mapped to the Forum capability error. The public Forum message stays generic while server diagnostics retain the owner error context.

## Deliberate API hardening

`compose_admitted_profiles` exists only as `pub(crate)` so the GraphQL adapter can reuse request-scoped `ProfileSummaryLoader` output. External crates cannot pass an arbitrary `ProfileSummary` map into Forum and bypass Profiles presentation.

The only external cross-owner composition entry point is `read_for_audience`, which always executes Profiles presentation before any Forum statistics are queried.

Transport authorization remains outside this owner composition method. Existing GraphQL admission remains authenticated `forum_topics:read`; future storefront integration must apply its existing public/authenticated storefront admission before calling the owner service.

## GraphQL 15B compatibility

`forumMemberCards(userIds, locale)` keeps the exact 15B GraphQL response shape and permission surface.

The request-scoped `DataLoader<ProfileSummaryLoader>` remains the preferred GraphQL path. When present, the loader performs audience-aware Profiles presentation and the GraphQL adapter passes only that admitted map to the crate-local composition helper.

When the host loader is absent, the fallback remains anonymous/fail-closed through `ForumMemberCardService::read_for_audience(..., ForumMemberCardAudience::Anonymous, ...)`, preserving FORUM-15A behavior.

The GraphQL transport no longer contains direct `forum_user_stats` SeaORM reads.

## Storefront cursor

The real storefront inventory was rechecked before this slice:

- `rustok-forum/storefront` models currently omit author identity/profile fields;
- the Leptos topic/reply UI currently renders no author/member card;
- GraphQL storefront transport has separate topic/reply requests;
- SSR/hydrate uses a native server adapter and the storefront package has no direct `rustok-profiles` dependency.

The next bounded FORUM-15 slice may now use `rustok_forum::services::user_stats::ForumMemberCardService` from the native adapter while adding a matching GraphQL storefront member-card transport, without moving Profiles ownership into the storefront package.

That future slice must preserve anonymous storefront availability and must not reuse the authenticated-only `forumMemberCards` transport as a public endpoint without an explicit storefront admission contract.

## Remaining FORUM-15 work

FORUM-15 is not complete. Remaining work includes:

- dual-path storefront member-card transport/composition;
- adding author identity/member-card presentation to the intended Forum storefront/admin surfaces;
- retained query-count/runtime evidence for bounded no-N+1 behavior;
- retained privacy/block/locale runtime or browser evidence.

The canonical FORUM-15 ledger remains materially correct and stays `in_progress`.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database scenario, migration, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-member-card-owner-service-source.mjs
```
