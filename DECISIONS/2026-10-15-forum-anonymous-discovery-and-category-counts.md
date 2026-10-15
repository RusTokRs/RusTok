# Forum anonymous discovery and category aggregate policy

- Date: 2026-10-15
- Decision status: Accepted for this remediation
- Owners: `rustok-forum` (visibility and settings), `rustok-seo-targets`/`rustok-seo` (public target context), `rustok-search` (storefront eligibility), server host (ports), Next.js Forum (counter presentation)
- Extends: `2026-10-09-forum-remove-unwired-module-settings.md` and the Forum audience-read contracts

## Decision

`allow_anonymous_reading=false` is a tenant-wide *public discovery* denial, not an RBAC override and not a channel toggle. It applies to anonymous category, topic and reply reads, topic/category routes (including public tombstone decisions), public SEO routes, sitemaps and bulk listings, Search projection, and **query-time** Search eligibility. Explicitly authenticated reads retain the existing category/topic audience, RBAC and channel gates. No authenticated request can borrow an anonymous public fallback to evade those gates. The default in a present/missing module settings snapshot is `true`, for compatibility with existing tenants; a missing settings-reader capability on a public path must not be interpreted as the default. Settings are read through the static-module-settings owner port, not through Forum queries against another module's tables.

The setting is hidden from the existing admin form for now: saving other Forum settings must preserve persisted `false`. Restoring a visible toggle needs a separate operator UX and rollout decision. Storefront channel disablement applies independently to both guests and signed-in readers; a signed-in request without sufficient Forum permissions must not be downgraded into an anonymous public read by native transports or Search adapters. There is no migration and no new settings key. Cached Search documents created before a setting change must be filtered again at search *read* time (including category hits, counts and facets); ingestion alone cannot revoke previously indexed documents. Reindexing after the switch can remove them from storage, but is not required for immediate public denial.

Forum's persisted `category.topic_count` and `category.reply_count` are **global aggregates for that category**, including topics not visible to an individual viewer. They are not viewer-scoped counts; keeping them public is an explicit product disclosure decision, not evidence that a viewer can access those topics. The category UI must describe these numbers as category totals and never use them to page or assert the number of visible topic cards. The owner topic list's cursor and results remain viewer-scoped. If the product later requires private aggregate sizes, the separate schema/owner-counts change must remove these public counters or introduce viewer-scoped aggregates; it must not silently relabel the global values.

## Failure / ownership boundaries

- Settings reader missing or failed: deny/fail the anonymous public operation; do not fall back to an unguarded public read. A missing/disabled *snapshot* from a functioning reader is different and uses the soft default.
- Settings `false`: public single reads behave like absence; lists have no contents or cursor; unscoped public Search returns no category/topic/reply hits or derived facets/counts, even when stale indexed rows exceed 100, because the owner preflight runs before FTS. An explicitly requested inaccessible category root can still fail during category-scope validation. Authenticated Search retains its pre-existing bounded raw-candidate behavior. Authoring SEO and authenticated owner/admin reads are unchanged.
- A category/topic policy or channel denial still applies independently when the setting is `true` or the viewer is authenticated.
- Stored aggregate counts do not grant any data access; the public aggregate disclosure is intentionally documented for operators.

## Search surface ownership

The generic storefront FTS and autocomplete paths do not compose the Forum
category/topic audience owner. Therefore they must **exclude Forum documents in
SQL**, including totals, facets and suggestions, regardless of the privacy
setting; their pinned query rules must not reintroduce a Forum hit. Admin Search
retains its existing scope. The explicit Forum-only storefront Search endpoint
alone may fetch raw Forum candidates and reauthorize each through the Forum
owner before presenting them. Its query rules may reorder that approved page
only; loading a pinned document from outside that page would bypass eligibility.
This deliberate generic-storefront exclusion may reduce cross-module Forum
search recall until a separately designed mixed-owner eligibility pipeline is
available; it is preferable to disclosing a private topic. The exported
`SearchQueryPort`/`SearchSuggestionPort` still perform raw host/admin queries;
they currently have no public consumer and must not be mounted as storefront
transports. A future FBA storefront adapter must resolve its trusted channel
and compose owner eligibility (or explicitly exclude Forum) before publication.
It requires no new settings key or migration.

## Subscription mutation response cutover

GraphQL `set/clearForumCategorySubscription` and `setForumTopicSubscription`
return full objects. They read the category/topic through its authenticated
audience owner *before* writing, then overlay the new `isSubscribed` flag after
the subscription owner confirms success. An absent or invisible target fails
before any write; profile/read errors cannot turn a committed subscription into
a false-negative response. REST category set/clear likewise prepare an owner
response before the write and overlay the resulting `is_subscribed` flag.
Category clears remain owner-gated in the service, including no-ops, to prevent
existence oracles.

Topic clears have different semantics: a user must be able to remove **only their
own row** after losing access to the topic, without learning whether the topic
still exists. The sole canonical GraphQL
`clearForumTopicSubscription(tenantId, topicId): Boolean!` delegates to that
ungated domain clear and returns `true` for hidden and absent targets alike,
without a topic read. The former object-returning GraphQL response was removed,
not kept as a parallel compatibility API. REST
`DELETE /api/forum/topics/{topic_id}/subscription` now returns `204 No Content`
on success without a topic body or a post-commit 404. Both response-shape changes
require clients to use a separate audience-authorized topic read when they need
content; no current repository caller consumes the old clear response. This
supersedes only the topic-clear transport response and GraphQL subscription
materialization clauses of the earlier
[`forum-vote-subscription-write-audience`](./2026-10-09-forum-vote-subscription-write-audience.md)
decision. Its domain write gate, blind clear, and vote contracts stay in force.

## Public SEO reads and keyset cursors

The Forum SEO wrapper and its lower-level topic mapper must use the **same**
static settings reader. An open tenant must be able to load public topic SEO
metadata and produce sitemap candidates after passing the outer discovery gate;
without that reader the mapper would fail closed even for allowed topics.
Authoring loads retain their existing system context. The runtime registers only
the audience-aware wrappers, never the mapper by itself.

Public SEO bulk pages cannot forward a cursor produced by an unfiltered mapper
page: it could identify a hidden category or topic, and filtering afterward
would yield sparse pages. Topic bulk uses the owner audience list's authorized
keyset; category bulk scans the raw category keyset internally and returns a
cursor from the last **visible** category, only when a further visible category
exists. Neither response exposes a hidden target ID as a cursor. Both may scan
more raw candidates than the requested page size; the public page size is
bounded by the Forum read limit and the extra work should be monitored.

## Persisted SEO sitemap files

SEO stores generated sitemap XML. Narrowing the tenant-wide anonymous setting
**or** a category/topic audience policy does not rewrite files already generated
by an operator. Whenever Forum contributes sitemap targets, its owner therefore
signals `public_sitemap_requires_refresh` even if anonymous reading is still
allowed. SEO renders the index and pages from **current** owner candidates,
never from previously stored XML; routes from other enabled modules remain in
the live index. Pages outside the live index (including stale numbered files)
are unavailable. Public sitemap responses use `Cache-Control: no-store`, so a
shared cache cannot retain pre-change URLs. This path re-collects candidates
on *every* public sitemap request and fails closed if an owner or settings reader
is unavailable. Operators must monitor its cost on large catalogs; only the
operator-generated sitemap job remains persisted.

## Verification

Test a stored `false` and `true` through the static settings port with a guest and an authenticated user for category, topic, reply, route and audience-list reads; test SEO bulk/sitemap and Search over pre-existing indexed documents, including category rows and facets. `seo_audience_targets_sqlite_tests.rs` covers an allowed tenant with hidden middle keyset rows, public metadata/sitemap loads, and absence of a settings reader; `tests/subscriptions.rs` covers GraphQL set/clear flags, hidden category denial and acknowledgement-only topic clear for hidden/absent IDs. `verify-forum-anonymous-discovery.mjs` and `verify-forum-subscription-levels.mjs --static-only` guard the source wiring. Run scoped Rust crates, Next typecheck/lint and audience verifiers. Do not claim a Rust runtime result where the Rust toolchain is unavailable.
