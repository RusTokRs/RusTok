# Forum anonymous discovery and category aggregate policy

- Date: 2026-10-15
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-forum` (visibility and settings), `rustok-seo-targets`/`rustok-seo` (public target context), `rustok-search` (storefront eligibility), server host (ports), Next.js Forum (counter presentation)
- Extends: `2026-10-09-forum-remove-unwired-module-settings.md` and the Forum audience-read contracts
- Supersedes: None
- Superseded by: None

## Context

`allow_anonymous_reading=false` is a tenant-wide *public discovery* denial, not an RBAC override and not a channel toggle. It applies to anonymous category, topic and reply reads, topic/category routes (including public tombstone decisions), public SEO routes, sitemaps and bulk listings, Search projection, and query-time Search eligibility.

## Decision

`allow_anonymous_reading=false` is a tenant-wide *public discovery* denial, not an RBAC override and not a channel toggle. It applies to anonymous category, topic and reply reads, topic/category routes (including public tombstone decisions), public SEO routes, sitemaps and bulk listings, Search projection, and **query-time** Search eligibility. Explicitly authenticated reads retain the existing category/topic audience, RBAC and channel gates. No authenticated request can borrow an anonymous public fallback to evade those gates. The default in a present/missing module settings snapshot is `true`, for compatibility with existing tenants; a missing settings-reader capability on a public path must not be interpreted as the default. Settings are read through the static-module-settings owner port, not through Forum queries against another module's tables.

The setting is hidden from the existing admin form for now: saving other Forum settings must preserve persisted `false`. Restoring a visible toggle needs a separate operator UX and rollout decision. Storefront channel disablement applies independently to both guests and signed-in readers; a signed-in request without sufficient Forum permissions must not be downgraded into an anonymous public read by native transports or Search adapters. There is no migration and no new settings key. Cached Search documents created before a setting change must be filtered again at search *read* time (including category hits, counts and facets); ingestion alone cannot revoke previously indexed documents. Reindexing after the switch can remove them from storage, but is not required for immediate public denial.

Forum's persisted `category.topic_count` and `category.reply_count` are **global aggregates for that category**, including topics not visible to an individual viewer. They are not viewer-scoped counts; keeping them public is an explicit product disclosure decision, not evidence that a viewer can access those topics. The category UI must describe these numbers as category totals and never use them to page or assert the number of visible topic cards. The owner topic list's cursor and results remain viewer-scoped. If the product later requires private aggregate sizes, the separate schema/owner-counts change must remove these public counters or introduce viewer-scoped aggregates; it must not silently relabel the global values.

## Sources of truth and ownership

- Settings reader missing or failed: deny/fail the anonymous public operation; do not fall back to an unguarded public read. A missing/disabled *snapshot* from a functioning reader is different and uses the soft default.
- Settings `false`: public single reads behave like absence; lists have no contents or cursor; unscoped public Search returns no category/topic/reply hits or derived facets/counts, even when stale indexed rows exceed 100, because the owner preflight runs before FTS. An explicitly requested inaccessible category root can still fail during category-scope validation. Authenticated Search retains its pre-existing bounded raw-candidate behavior. Authoring SEO and authenticated owner/admin reads are unchanged.
- A category/topic policy or channel denial still applies independently when the setting is `true` or the viewer is authenticated.
- Stored aggregate counts do not grant any data access; the public aggregate disclosure is intentionally documented for operators.

## Invariants

- Generic storefront FTS and autocomplete paths exclude Forum documents in SQL.
- Explicit Forum-only storefront Search endpoint alone may fetch raw Forum candidates and reauthorize each through Forum owner.

> Amended 2026-10-25 by [Declared Search audience corpus](./2026-10-25-search-audience-declared-corpus.md):
> the blanket SQL exclusion of Forum documents is replaced by the declared
> `SearchAudienceClass::Gated` scope. Generic storefront Search and suggestions still
> never expose an unauthorized Forum document: Forum is excluded unless its discovery
> preflight admits it and its owner authorizes each candidate, and an undeclared module
> is denied outright.

## Non-goals

- Inventing new settings keys or running database schema migrations for forum discovery.
- Exposing viewer-scoped private aggregate counts on category headers.

## Data, transaction, and concurrency boundary

GraphQL `set/clearForumCategorySubscription` and `setForumTopicSubscription` return full objects. They read the category/topic through its authenticated audience owner *before* writing, then overlay the new `isSubscribed` flag after the subscription owner confirms success. An absent or invisible target fails before any write; profile/read errors cannot turn a committed subscription into a false-negative response. REST category set/clear likewise prepare an owner response before the write and overlay the resulting `is_subscribed` flag. Category clears remain owner-gated in the service, including no-ops, to prevent existence oracles.

Topic clears have different semantics: a user must be able to remove only their own row after losing access to the topic, without learning whether the topic still exists.

## Context dimensions

Tenant-wide anonymous reading flag participates in public discovery gate. Authenticated principal context participates in audience-authorized access.

## Events and projections

Whenever Forum contributes sitemap targets, its owner signals `public_sitemap_requires_refresh` even if anonymous reading is still allowed. SEO renders the index and pages from current owner candidates, never from previously stored XML.

## Failure semantics

Settings reader missing or failed denies the anonymous public operation. Rejects without fallback.

## Migration and cutover

Zero-legacy response shape for `clearForumTopicSubscription`: returns Boolean in GraphQL and 204 No Content in REST.

## Alternatives considered

- Returning empty public category counts when hidden: rejected to preserve global operator metrics.
- Reindexing on every setting change: rejected as too heavy compared to read-time filtering.

## Verification

Test a stored `false` and `true` through the static settings port with a guest and an authenticated user for category, topic, reply, route and audience-list reads; test SEO bulk/sitemap and Search over pre-existing indexed documents, including category rows and facets. `seo_audience_targets_sqlite_tests.rs` covers an allowed tenant with hidden middle keyset rows, public metadata/sitemap loads, and absence of a settings reader; `tests/subscriptions.rs` covers GraphQL set/clear flags, hidden category denial and acknowledgement-only topic clear for hidden/absent IDs. `verify-forum-anonymous-discovery.mjs` and `verify-forum-subscription-levels.mjs --static-only` guard the source wiring.

## Consequences

- Audience-safe public forum discovery with zero data disclosure leaks.
- Clear separation between public total category counts and viewer-scoped topic items.
