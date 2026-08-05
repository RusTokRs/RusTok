# FORUM-24A topic route identity owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-24A establishes the Forum-owned identity contract for localized public topic routes without mounting a new HTTP or storefront route. It keeps the existing locale-specific topic translation slug as the readable segment, adds one deterministic short topic identity, and provides a transport-neutral resolver for canonical, redirect and gone outcomes.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-route-identity-owner.json
```

## Canonical route shape

The canonical topic path is:

```text
/{locale}/forum/t/{short_id}/{slug}
```

`short_id` is the first 48 bits of the topic UUID rendered as twelve lowercase hexadecimal characters. The slug is readable and canonicalized, but it is not the identity. A request with the correct short identity and an old, missing or otherwise non-canonical slug resolves to a redirect to the current canonical path.

The owner reads at most two current topic candidates for one tenant, locale and short identity. One candidate is required. A short-identity collision or any ambiguous route history fails closed with `FORUM_TOPIC_ROUTE_RESOLUTION_CONFLICT`; the resolver never chooses a candidate by slug.

## Merge composition

`ForumTopicRouteService` reuses the existing bounded `ForumTopicCanonicalResolutionService`. A route whose short identity belongs to an archived merge source resolves to the terminal retained topic and its current exact-locale slug. Merge receipts remain the only source of source-topic-to-target-topic canonical identity.

This slice does not modify the existing ID-based REST redirect from FORUM-21J.

## Immutable alias and tombstone ledger

Migration `m20260805_000024_add_forum_topic_route_aliases` adds PostgreSQL and SQLite `forum_topic_route_aliases` storage. Each row owns one immutable route tuple:

- tenant, locale, short identity and slug;
- original topic identity;
- `redirect` or `gone` disposition;
- target topic and target locale for redirects;
- bounded reason and creation timestamp.

The route tuple is unique per tenant. Update and delete attempts fail closed. Redirect rows store the target topic and locale rather than a target slug, so the resolver recomputes the target's latest canonical path and can follow later merge or rename composition without mutating historical aliases.

The service exposes crate-local idempotent transaction helpers for future rename, merge and delete owners. Exact repeated alias writes return the existing row; disposition, target or reason drift fails closed.

## Authorization boundary

Route identity is not visibility authorization. The resolver intentionally does not publish SEO state or decide whether a private, pending or channel-scoped topic may be disclosed. Every host or transport must apply the same canonical topic read/visibility admission before returning a descriptor, redirect location or tombstone result.

## Compatibility and remaining work

FORUM-24 remains `planned`. This slice adds one owner service and one migration only. It does not add a GraphQL field, REST route, Next/Leptos storefront page, category route, hreflang output, schema.org output or SEO publication rule.

Remaining work includes:

- composing immutable alias writes into topic slug rename, merge and delete transactions;
- mounting authorized localized topic routes in both storefront hosts;
- localized category route identity and move/rename aliases;
- locale alternates, canonical metadata and private/pending SEO exclusion;
- retained SQLite, PostgreSQL and mounted-browser evidence.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
cargo test -p rustok-forum services::topic_route::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_identity_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
