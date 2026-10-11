# Reply deep links address one post through `?reply=<reply-uuid>`

- Date: 2026-10-11
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (link authoring in `src/notification_source.rs` and `src/search_projection.rs`, canonical topic route in `src/services/topic_route.rs`, route reading in `src/seo_targets.rs`); `apps/next-frontend` (the surfaces that consume the anchor: `src/app/[locale]/forum/t/[...slug]`, `src/app/[locale]/modules/forum`, `packages/rustok-forum`)
- Extends: `DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md`
- Supersedes: None
- Superseded by: None

## Context

The forum module already authors reply anchors. The notification fan-out routes a reply event to
`/modules/forum?category=…&topic=…&reply=<uuid>` and the search projection appends `?reply=<uuid>` to the canonical
topic route (`crates/modules/rustok-search/src/engine.rs` asserts that exact shape). No consumer honored it: `seo_targets.rs`
`parse_forum_route` reads `category` and `topic`, and the Next.js surfaces read the same two keys, so every anchor link
landed at the top of the topic. The module README recorded the gap as needing "a route-contract decision plus per-surface
plumbing".

Auditing the link target exposed a second, larger break. The owner publishes the canonical topic path as
`/{locale}/forum/t/{short_id}/{slug}`, the Rust storefront host serves that path, and the search and SEO documents embed
it — but the Next.js host mounted the topic page at `/[locale]/modules/forum/t/[...slug]`. So on the Next.js storefront
every owner-authored topic link 404'd, including the page's own canonical redirect (`redirect(canonical.path)` pointed at
a route the app did not serve), and the feed's full-page link used `t/{slug}`, which resolved into that same redirect.

Reply lists are cursor-paged at the tenant's `replies_per_page` size, so the addressed post is not necessarily in the
first page, and the owner exposes no "page containing this reply" read.

## Decision

1. `reply` is a reserved query parameter of a forum topic route. Its value is the reply UUID. Producers are the
   notification fan-out and the search projection; consumers are the Next.js forum surfaces.
2. The anchor is a presentation concern. It selects which post to scroll to and mark; it never changes which topic a
   route resolves to. `parse_forum_route`, route canonicalization and stored canonical paths keep reading `category`
   and `topic` only, and no anchor is persisted in a route row, a search document or a notification target.
3. Hosts serve the owner's canonical topic route prefix `/[locale]/forum/t/…`. A host that mounts it under an extra
   prefix breaks every link the owner emits; the Next.js topic page moved to
   `src/app/[locale]/forum/t/[...slug]/page.tsx` and the superseded `modules/forum/t` mount was deleted rather than
   aliased, because a second live path for one canonical route creates duplicate canonical targets.
4. A canonical redirect on the topic route preserves `reply` verbatim; dropping the query during normalization would
   silently lose the reader's position.
5. Consumers resolve the anchor against the posts they already have, then page forward through the existing owner read
   (`forumStorefrontReplies` with `after`) for at most 20 pages. A post outside that bound or not readable by this
   reader is reported as absent from the loaded part of the thread; the surface does not claim the link is broken and
   does not keep fetching.
6. The parameter value is untrusted URL input: a consumer validates it as a UUID before using it in a DOM selector or
   before paging, and an invalid value is ignored without any extra request.
7. No owner read-model change and no new transport parameter. The anchor rides on the cursor pagination the owner
   already publishes.

## Sources of truth and ownership

- The reply row in `forum_replies` owns the identifier the anchor carries.
- The forum module owns the link strings it authors: notification target routes, search document routes, and the
  canonical topic path built by `services/topic_route.rs`.
- Each host owns rendering. The anchor is never stored, so there is no projection or counter to rebuild.

## Invariants

- `reply` appears only as a query parameter of an authored link, never inside a canonical path, a sitemap entry or a
  stored route row.
- Anchor resolution uses the same tenant, channel, locale and audience-scoped reply reads as the thread view; it adds
  no second authorization path.
- Opening one link cannot page unboundedly: at most 20 reply pages, and the walk stops as soon as the post is found.
- The two host storefronts keep the same canonical topic path prefix, so an owner-authored link resolves on both.

## Non-goals

- No per-reply route segment (`/t/{short}/{slug}/r/{reply}`) and no reply permalink in sitemaps or SEO targets: a reply
  is not independently indexable content here.
- No owner-side "page containing this reply" read. If the 20-page bound proves too small in practice, that read is the
  next step and needs its own decision.
- The Leptos module storefront widget is not a link target for `reply` — both producers emit Next.js-shaped routes — so
  it keeps no anchor handling. Its one-page feed limit stays recorded in its own README.

## Data, transaction, and concurrency boundary

Read-only. A view reads reply pages through the existing keyset cursor reads, each in its own read transaction. No
writes, no transactions spanning pages, no locks, no journal entries.

## Context dimensions

- Tenant: reply pages are resolved for the request tenant by the storefront audience reads. A foreign reply id never
  matches and is treated as absent.
- Channel, locale: unchanged. The anchor rides on the already-scoped thread read, so a reply hidden by channel or
  locale policy is not revealed by it.
- Policy, auth: a post is marked only if it is rendered for this reader; the anchor grants no read permission.
- Principal: none beyond the ordinary thread read.

## Events and projections

No new events. The notification target route and the search document route keep their existing shapes; only
consumption of `reply` changes.

## Failure semantics

- Malformed or non-UUID `reply`: ignored, the topic opens normally, no extra request is made.
- The linked reply is hidden, deleted, or beyond the 20-page bound: the thread renders and states that the post is not
  in the loaded part of the thread. The topic is not reported as missing and the link is not called broken.
- A page fetch failing during the walk surfaces through the load-error path (localized banner with a retry action),
  never as a silently short thread.
- A host route that does not serve the canonical prefix fails closed to a 404 for that host only; the owner keeps
  publishing one canonical path and does not branch per host.

## Migration and cutover

No data migration. Links already stored in notification rows and search index documents start resolving as soon as the
consumers deploy, because the parameter is additive and the path it rides on is unchanged. The Next.js
`/modules/forum/t/…` mount is deleted, not redirected: it was unreachable by owner-authored links and served no
canonical target, and the only in-app link to it (the topic feed) now emits the canonical prefix.

## Alternatives considered

- Carry the anchor as a URL fragment (`#reply-<uuid>`): fragments are not sent to the server, so a server-rendered page
  cannot decide which page of the thread to load, and the walk would start blind on every client render. Rejected.
- Add an owner read for the page containing a reply: exact and one round trip, but it needs a new public transport
  parameter plus a reverse keyset walk, and it changes the read contract for a case the bounded walk already covers.
  Deferred until the bound is demonstrably too small.
- Keep the Next.js mount under `modules/` and make the owner publish host-specific paths: rejected. Route identity would
  leak host layout into the owner, and two live paths for one topic create duplicate canonical targets.
- Ignore the anchor and keep topic-level links: rejected. A notification that says "X replied to your post" and drops
  the reader at the top of a long thread fails the reader's actual task.

## Verification

Scoped checks run locally: `node scripts/verify/verify-ui-i18n-parity.mjs`, `node scripts/verify/verify-ui-i18n-keys.mjs`,
`node scripts/verify/verify-i18n-contract.mjs` and `node scripts/verify/verify-adrs.mjs` exit 0, and the full
`scripts/verify` sweep reports no delta against the base revision. `cargo` and `pnpm` are not installed where this was
written, so `pnpm --filter @rustok/next-frontend typecheck`, `pnpm --filter @rustok/next-frontend lint`,
`cargo check -p rustok-forum` and `cargo test -p rustok-forum --test search_canonical_route_cutover_contract
--test canonical_route_seo_policy_contract` were not run; CI owns them.

## Consequences

- Notification, mention and search links land on the referenced post when the reader can already read it and it is
  within the bound; a miss says so instead of silently scrolling to the top.
- `reply` is now part of the route contract the forum module authors. A future route change must preserve the parameter
  or migrate stored links explicitly.
- The Next.js storefront serves the owner's canonical topic path, so `redirect(canonical.path)` and search-result
  hrefs resolve on both hosts, and the `FORUM-20` limitation narrows to the unreachable-post case plus the missing
  owner-side "page containing reply" read.
