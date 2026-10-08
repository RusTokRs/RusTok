# Owner-held routes, cursor pagination, and batched conversion for Blog and Forum

- Date: 2026-10-08
- Decision status: Proposed
- Implementation status: Not started
- Owners: `rustok-blog` (post storage, post routes and redirects); `rustok-forum` (topic storage, topic routes and redirects); `rustok-content` (shared ports and the canonical route resolver contract); `rustok-content-orchestration` (Blog/Forum conversion and route resolver implementation)
- Extends: `DECISIONS/2026-03-28-content-orchestration-port-boundary.md`, `DECISIONS/2026-03-28-multilingual-content-contract.md`
- Supersedes: None (when accepted, this ADR supersedes the route-registry part of `DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`, namely the `Slug routes and redirects` section and the related invariants)
- Superseded by: None

## Context

The current implementation stores public routes of Blog posts in the
`canonical_url` and `url_alias` tables owned by `rustok-content`. Commit
`1634179` on `arena/e1b32256-rustok` writes Blog rename and delete routes into
that registry through `CanonicalUrlWriter`. This violates the platform rule that
domain tables belong to their domain module, and it leaves Forum routes in the
same registry.

Conversion between Blog and Forum has these properties:

- A Blog post is one large rich-text body plus Blog comments.
- A Forum topic is a first message plus Forum replies stored in `forum_reply`.
  Replies and comments are different records and are not interchangeable.
- The current `promote` and `demote` implementations copy translation rows,
  reject conversion when comments or replies exist, and load the whole reply
  set into memory with `.all()`.

Conversion is rare. Steady-state read and write paths for billions of posts and
topics must scale. Measured on the current code:

- Public and bulk listings use `PaginatorTrait` (`LIMIT/OFFSET`). Deep pages cost
  grows with the offset. The SEO bulk and sitemap scan also pages by offset.
- `resolve_route` queries `url_alias` by `(tenant_id, alias_url)`, but the unique
  index is `(tenant_id, locale, alias_url)`. The leading `locale` column does not
  serve this query before PostgreSQL skip-scan support.
- The Forum reply counter is serialized by a per-topic row lock trigger
  (`forum_lock_reply_counter_mutation`). Hot topics contend on that lock.
- The unique alias index allows the same URL in several locales for different
  targets, so the schema does not prevent ambiguous global Blog routes.

## Decision

1. **Owners hold their routes.** `rustok-content` owns no domain tables and no
   route registry. The `canonical_url` and `url_alias` tables and the
   `CanonicalUrlWriter` are retired.

2. **Canonical routes are derived, not stored.** Each owner builds its canonical
   route from its own identity:
   - Blog: `/modules/blog?slug={current_slug}`;
   - Forum: `/modules/forum?topic={topic_id}`.
   The owner's current slug or id is the single source of truth. No canonical
   route row exists.

3. **Redirect rows are owned by the owner of the source route.** A redirect is
   stored in the table of the module whose namespace the source route belongs to:
   - `blog_post_routes`: source routes under `/modules/blog?…`, primary key
     `(tenant_id, source_route)`, no locale column because Blog slugs are global.
     Columns: `target_kind`, `target_id`, `created_at`, `updated_at`.
   - `forum_topic_routes`: source routes under `/modules/forum?…` and localized
     topic slugs, primary key `(tenant_id, locale, source_route)`, because Forum
     slugs are locale-aware under `DECISIONS/2026-03-29-forum-slug-locale-contract.md`.
   The target may belong to another module (for example, a demoted post redirects
   to a forum topic). The target is a plain `(target_kind, target_id)` pair with
   no foreign key across modules.

4. **One resolver contract, dispatch by namespace.** `rustok-content` defines the
   port `CanonicalRouteResolver` with `resolve_route(tenant_id, locale, route)`.
   `rustok-content-orchestration` implements it because it already depends on
   Blog and Forum. The namespace prefix of the route selects the owner table, so a
   request performs one owner lookup, not one per module. The resolver order is:
   owner canonical route (derived), then the owner's redirect table. Consumers
   (`rustok-seo` routing, the storefront server function, and the
   `resolveCanonicalRoute` GraphQL query) receive the port through injection.
   None of them queries owner tables directly.

5. **Conversion moves metadata, and bulk work is batched.** Conversion is a
   resumable operation with a status row and a keyset cursor:
   - Phase 1, short transaction: create the operation row and mark the source
     topic or post as frozen for writes.
   - Phase 2, batches of bounded size with keyset cursors: move replies to
     comments (or the reverse) in one short transaction per batch. Each batch is
     idempotent by source id.
   - Phase 3, short transaction: switch the owner attributes, create the redirect
     row in the owner table, and clear the frozen flag.
   - Phase 4, batches: remove the source rows that were moved.
   Conversion never loads the full reply set into memory. A failed run resumes
   from the last committed cursor.

6. **Keyset pagination everywhere on hot paths.** Public and admin listings,
   reply and comment lists, and the SEO bulk and sitemap scans use keyset
   cursors:
   - Blog posts: `(published_at, id)`;
   - Forum topics: `(last_activity_at, id)` or the existing order key plus `id`;
   - Replies and comments: `(position, id)`;
   - Bulk and sitemap: `id`.
   `OFFSET` is not used on these paths.

7. **Locale rules.** Canonical and redirect keys are locale-neutral where the
   owner says so (Blog slugs, Forum topic ids). Localized Forum slugs are keyed by
   locale. Public reads resolve the request locale with the fallback chain
   requested → explicit → `en` → first available, using the shared helpers in
   `rustok-content::locale`. Uniqueness for Blog routes is enforced by the primary
   key `(tenant_id, source_route)`, across all locales.

## Sources of truth and ownership

- Canonical owners: `rustok-blog` owns post identity, slug, and `blog_post_routes`; `rustok-forum` owns topic identity, localized topic slugs, and `forum_topic_routes`; `rustok-content` owns the `CanonicalRouteResolver` port and the shared locale rules.
- Authoritative persisted state: owner tables only. Redirect rows are authoritative for redirects; canonical routes are derived from owner identity and are never stored.
- Derived state: none that participates in correctness. Any future cache is invalidated by `CanonicalUrlChanged`, `UrlAliasPurged`, and `TargetDeleted`.
- Transport representations: GraphQL `resolveCanonicalRoute`, the storefront server function, and SEO route context are read-side projections of the resolver result.
- Dependency direction: `rustok-content` defines the port and does not depend on Blog or Forum. `rustok-content-orchestration` depends on Blog, Forum, and `rustok-content` and implements the port. Consumers (`rustok-seo`, storefront, GraphQL) depend only on the port.

## Invariants

### Allowed states

- Each Blog post has exactly one canonical route, derived from its current slug.
- A source route appears at most once in its owner table per tenant (Blog) or
  per tenant and locale (Forum).
- A redirect target may belong to another module. The target must exist when the
  redirect is created and is removed when the target is deleted.

### Forbidden states

- A redirect source equal to a live canonical route of any target.
- Two owner tables holding the same source route for the same tenant.
- A forum redirect with a locale outside the topic's known locales (validated by
  the owner).
- Any conversion state where a message is visible in both the source and the
  target thread.

## Non-goals

- Moving Blog bodies or Forum messages into shared storage.
- Unifying Forum replies with Blog comments. The two models keep their own
  storage and their own rules.
- Localized Blog slugs. Blog slugs stay global by the multilingual contract.
- Ranking, search, or sitemap content policy changes beyond pagination.

## Data, transaction, and concurrency boundary

- Database-enforced: the primary keys of both redirect tables; the tenant-scoped
  indexes listed in the verification section; `NOT NULL` on targets.
- Route creation, the post or topic attribute switch, and the frozen flag change
  in one short transaction per phase. No transaction spans a full conversion.
- Conversion batches are idempotent by source id. A retry after a crash commits
  nothing twice.
- Frozen source topics reject writes with a conflict error. Readers see the
  previous owner until Phase 3 commits.
- Forum reply counters keep the existing serialized trigger. Hot-topic contention
  is documented as an accepted cost, not solved by this ADR.
- Deletion of a target removes redirect rows that point at it in the owner table
  of the redirect source. The removal runs from a domain event, is idempotent, and
  is retried until it succeeds.

## Context dimensions

- Tenant: every route and redirect row is tenant-scoped.
- Locale: Forum redirect and public Forum slug resolution use the request locale
  and the fallback chain. Blog routes are locale-neutral.
- Channel: channel visibility is still checked after the route is resolved, as
  today.
- Principal: conversion requires the same authorization as today. Read paths are
  public as today.

## Events and projections

- Owners emit `CanonicalUrlChanged` and `UrlAliasPurged` from `rustok-events`
  when they change redirects. `rustok-seo` keeps consuming them for automatic
  redirects. The event type contract is unchanged.
- Owners consume `TargetDeleted` to purge redirects that point at a deleted
  target, in the owner of the redirect row.
- The resolver is not a projection. It reads owner tables directly. Caches, if
  added later, are invalidated by the same events.

## Failure semantics

- A redirect whose target is missing resolves to "not found" and emits a metric
  and a log entry. It never redirects to a missing record. A reconciliation job
  finds and removes such rows.
- A conversion that fails mid-run is resumable. The operation status shows the
  last committed cursor. An operator can cancel and unfreeze the source.
- Route resolution and redirects fail closed on owner-table errors. They do not
  fall back to another owner.

## Migration and cutover

- The registry tables `canonical_url` and `url_alias` and their migrations
  (`m20260328_000001_create_content_url_tables`,
  `m20260721_000004_expand_content_locale_storage_columns`) are removed under the
  unreleased-schema rule (amend pending), if they have not been released. If any
  release has created them, a dedicated drop migration is required instead. This
  question must be answered before implementation starts.
- `blog_post_routes` and `forum_topic_routes` are new tables in the owner crates.
- Existing `url_alias` and `canonical_url` rows created by commit `1634179` or by
  earlier conversions must be migrated once into the owner tables before the drop
  runs. The migration is one-off and idempotent. No live data is expected because
  the schema is unreleased; this must be verified in each environment.
- Blog migration `m20261008_000031_remove_blog_post_view_count` is unchanged.
- Public list and bulk APIs change from offset to cursor arguments. This is a
  contract change and must follow the release policy for GraphQL and REST.

## Alternatives considered

- **Keep the registry in `rustok-content`.** Rejected: it stores Blog and Forum
  domain routes in content-owned tables, which violates the ownership rule.
- **Owner tables with cross-module lookup on every request.** Rejected: it issues
  one query per module and has no single resolver contract.
- **Unify Forum replies and Blog comments into one thread model.** Rejected: it
  requires a one-time data migration and gives no steady-state benefit that the
  owners need.
- **Shared body storage in `rustok-content`.** Rejected: it reverses
  `DECISIONS/2026-03-28-content-orchestration-port-boundary.md` and conflicts with
  the i18n rule that localized text lives in owner translation tables.
- **Synchronous full-copy conversion.** Rejected: its cost grows with thread size
  and holds one transaction for the whole operation.

## Verification

Required evidence before the decision moves to Accepted and before
implementation is marked complete:

- Database: primary keys of `blog_post_routes` and `forum_topic_routes`;
  tenant-scoped index for the namespace lookup; `EXPLAIN` on the resolver query
  for each owner table with a production-sized sample, showing an index scan.
- Pagination: tests that keyset cursors return every row exactly once across
  pages, including ties on the sort key; no `OFFSET` on the listed hot paths
  (static check added to the verify scripts).
- Conversion: integration tests for each phase; a test that stops after a batch
  and resumes; a test that a retry commits nothing twice; a test that writes to
  the frozen source are rejected; a test that conversion never loads the full
  reply set (the batch size is the only bound).
- Routes: integration test that a renamed Blog post resolves its old route to the
  current post; a claim test where a new post takes a retired slug; a delete test
  that the redirect is purged on the `TargetDeleted` event; a missing-target test
  that returns "not found" and emits the metric.
- Locale: test that a Forum localized slug resolves through the fallback chain
  and that a Blog route is unique across locales.
- Contract: updated verify scripts for `verify:content:orchestration` and the
  Blog and Forum route contracts.

## Consequences

- Blog and Forum own their storage, routes, and redirects. `rustok-content` keeps
  shared rules, the port, and the resolver contract.
- Conversion cost depends on the size of the source thread, but it runs in
  bounded batches. It never blocks steady-state reads.
- Public list APIs change to keyset cursors. This is the largest external
  compatibility cost.
- The Blog route part of `DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`
  and the current H-3 implementation in commit `1634179` must be replaced.
- Follow-up work, in order: decide the migration policy for the registry tables;
  add owner tables and migrations; move `promote` and `demote` to batched phases;
  implement the resolver port and inject it into SEO, the storefront, and GraphQL;
  switch listings and bulk scans to keyset cursors; update ADR, module docs,
  runbook, CHANGELOG, and verify scripts.
