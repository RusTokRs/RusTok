# FORUM-24O canonical category route storefront mount

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24O mounts the localized Forum category route in the shared Rust storefront:

```text
/{locale}/forum/c/{slug}
```

The slice includes:

- canonical category-card href construction from the admitted category effective locale and slug;
- an Axum host mount in the shared `rustok_storefront::router()`;
- execution of the FORUM-24N selected native/GraphQL transport decision;
- private permanent redirects to the owner-provided canonical path;
- private not-found and transport-failure responses;
- reuse of the existing Forum module page with the owner-resolved category UUID.

Machine contract:

```text
crates/rustok-forum/contracts/forum-category-route-storefront-mount.json
```

## Module and host boundaries

`rustok-forum-storefront` owns category href normalization, route DTOs and selected transport resolution. The host does not query Forum persistence, read immutable alias rows, calculate a canonical category path or evaluate category audience policy.

The host consumes only:

```rust
resolve_storefront_category_route(locale, slug)
```

FORUM-24N has already rechecked the canonical category through the exact storefront category-list audience boundary and routed channel module gate. The host does not repeat or broaden that decision.

## Host response policy

The mounted route behaves as follows:

- exact `CANONICAL`: render the existing Forum module page with `category=<owner UUID>`;
- `REDIRECT`: private `308 Permanent Redirect` to the owner-provided path;
- a canonical decision whose raw `OriginalUri.path()` differs from the owner path: private `308 Permanent Redirect`;
- missing, archived, channel-disabled or audience-hidden route: private `404 Not Found`;
- transport failure, persistence conflict or malformed public descriptor: private `503 Service Unavailable`.

Redirect, not-found and failure responses use:

```text
Cache-Control: private, no-store
```

There is no category `GONE` decision or `410 Gone` response. Category archive history remains undisclosed.

Axum-decoded route values are passed only to the module transport. Exact canonical equality uses the undecoded `OriginalUri.path()`, so case and percent-encoded variants redirect instead of being treated as canonical.

Before using an owner-provided path as `Location`, the host requires a local absolute path beginning with one `/`. Absolute external URLs, protocol-relative paths and values containing control characters fail closed as private `503` instead of becoming redirects.

When rendering, the host overwrites any supplied `category` query parameter with the owner-resolved UUID and removes an unrelated `topic` parameter. A category route therefore cannot be used to select an arbitrary topic while retaining category-route status.

## Category-card cutover

Category cards no longer emit:

```text
/{locale}/modules/forum?category=<uuid>
```

The module core builds:

```text
/{effective_locale}/forum/c/{slug}
```

from the already admitted `ForumCategoryListItem`. Locale and slug are normalized with the same bounded route-segment policy used by Forum route owners. Invalid persisted identity fails closed to the generic Forum module base rather than emitting a malformed canonical URL.

The generic module query route remains available as a compatibility surface for direct callers and internal rendering. Only category-card navigation is cut over.

## Transport selection

SSR/hydrate continues to select the native server function. CSR/headless continues to select GraphQL. A selected transport failure never falls back to the other path.

The host receives no alias identity, reason, policy layers, viewer facts or denial reason.

## Compatibility

This slice does not:

- change topic routes or topic-card links;
- add category tombstones or public archive disclosure;
- change category commands, lifecycle commands or alias storage;
- remove the generic Forum module route;
- change GraphQL, REST or native owner contracts introduced in FORUM-24N;
- add canonical document metadata, hreflang, schema.org or other SEO composition;
- change the Next storefront;
- add a migration.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, SQLite/PostgreSQL execution, migrations, workflows, registered-host requests, browser scenarios or CI were executed while preparing this slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-category-route-storefront-mount.mjs
cargo test -p rustok-forum --test category_route_storefront_mount_contract -- --nocapture
cargo test -p rustok-forum-storefront core::tests -- --nocapture
cargo test -p rustok-storefront forum_category_route::tests -- --nocapture
cargo check -p rustok-forum-storefront --all-targets --features ssr
cargo check -p rustok-storefront --all-targets --features ssr
```

## Remaining FORUM-24 scope after FORUM-24O

- canonical and hreflang document policy for Forum category/topic routes;
- Forum-specific SEO composition and matching schema.org semantics;
- Next storefront parity;
- maintainer SQLite, PostgreSQL, registered-host and browser evidence.

The canonical implementation plan remains the single roadmap. Its FORUM-24 ledger entry is not updated by this slice because the connected complete-file writer cannot safely retrieve and replace the full plan losslessly; this document records only the stable FORUM-24O contract and does not create a second backlog.
