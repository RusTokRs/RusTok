# FORUM-24Q Search canonical route cutover

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24Q cuts Forum Search navigation over from internal UUID module query URLs to the localized category and topic routes delivered by FORUM-24A through FORUM-24P:

```text
/{locale}/forum/c/{slug}
/{locale}/forum/t/{short_id}/{slug}
/{locale}/forum/t/{short_id}/{slug}?reply={reply_id}
```

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-canonical-route-cutover.json
```

## Owner boundary

`ForumSearchProjectionSource` remains the Forum-owned public Search projection. After exact anonymous visibility admission, it resolves route identity through:

```rust
ForumCategoryRouteService
ForumTopicRouteService
```

Category and topic documents require an exact projection locale. A locale fallback is not projected under the requested locale. Reply documents reuse the exact canonical topic descriptor and append only the existing `reply` selection key.

The Forum projection no longer emits:

```text
/modules/forum?category={category_id}
/modules/forum?topic={topic_id}
/modules/forum?topic={topic_id}&reply={reply_id}
```

## Search boundary

`rustok-search` remains the single transport-neutral result URL projection used by GraphQL, native storefront, Search admin and the admin application shell. It does not import Forum or recalculate Forum route identity.

For a Forum result, Search requires the owner-projected `payload.route` and validates:

- the canonical Forum source/entity pair;
- result and payload category/topic/reply UUID identity;
- exact normalized result-locale and route-locale equality;
- category or topic path shape;
- the twelve-lowercase-hex topic short identity against the topic UUID;
- the bounded lowercase kebab slug segment;
- one exact `reply={reply_id}` query for reply results;
- a root-relative path without protocol-relative prefixes, fragments, control characters, duplicate separators or extra query components.

Search returns the original owner-projected route after validation. It does not normalize a malformed path into a different destination.

## Reindex and degraded behavior

Existing indexed Forum documents still containing UUID query routes are intentionally non-navigable until rebuilt through the current Forum projection. Documents missing `payload.route` also fail closed.

No compatibility fallback is added. This repository is pre-release and the canonical cutover updates the surviving internal contract directly. Search result rows and ranking remain available, but a stale row exposes no unsafe or obsolete href.

A full Forum Search projection rebuild is therefore required before runtime promotion. The existing projection invalidation, owner-revision ledger, Search inbox, checkpoint and repair protocols are unchanged.

FORUM-24R adds the executable PostgreSQL handoff for this requirement:

```text
crates/rustok-search/tests/forum_canonical_route_reindex_postgres.rs
```

It exercises real Forum owner writes, the durable Forum inbox, staged tenant replacement, removal of legacy UUID routes, canonical Search URL acceptance, stale-orphan cleanup and cross-tenant isolation. Its source is present but has not been executed.

## Visibility and authorization

This slice does not broaden discovery. Forum still admits category, topic and approved-reply documents through `ForumPublicDiscoveryService`. Search storefront eligibility and admin permission checks remain unchanged.

A canonical route is not an authorization token. Every destination route continues to apply its existing module, audience, channel and lifecycle policy.

## Compatibility

This slice does not change:

- Search GraphQL, native or admin result DTOs;
- Search storage schema;
- Forum category, topic or reply commands;
- route alias or tombstone storage;
- Forum or Search event schemas;
- ranking, filtering, totals, facets or pagination;
- Product, Blog or generic Content result URLs;
- migrations.

## Verification handoff

No tests, Node verifiers, formatting, Cargo commands, SQLite/PostgreSQL execution, reindex, workflows, HTTP requests, browser scenarios or CI were executed while preparing FORUM-24Q or FORUM-24R.

Maintainers can run:

```bash
node scripts/verify/verify-forum-search-canonical-route-cutover.mjs
node scripts/verify/verify-forum-search-canonical-route-reindex-harness.mjs
node scripts/verify/verify-search-canonical-url-contract.mjs
node scripts/verify/verify-forum-public-discovery-seo.mjs
cargo test -p rustok-search engine::tests -- --nocapture
cargo test -p rustok-search --test forum_canonical_route_reindex_postgres -- --nocapture
cargo test -p rustok-forum --test search_canonical_route_cutover_contract -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets --features graphql
```

Runtime promotion additionally requires executing the PostgreSQL harness, rebuilding Forum Search documents in the target environment and confirming that category, topic and reply results expose only localized owner routes through GraphQL, native storefront and admin consumers.

## Remaining FORUM-24 scope

- complete Next storefront Forum product and canonical-route parity when its module-owned package exists;
- retain registered-host and browser evidence after the executable PostgreSQL reindex proof is run;
- reconcile the canonical FORUM-24 ledger after maintainer execution.

`crates/rustok-forum/docs/implementation-plan.md` remains the only authoritative roadmap. Its FORUM-24 ledger is stale relative to the merged source slices. The connected complete-file writer cannot safely replace that large document losslessly, so these bounded task documents do not create a second backlog or claim ledger synchronization.
