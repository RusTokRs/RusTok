# Forum topic-feed selection belongs to the paged read owner

- Date: 2026-10-17
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (topic list and audience/read-state composition), module storefront adapters
- Extends: `DECISIONS/2026-10-15-forum-anonymous-discovery-and-category-counts.md`, `DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md`
- Supersedes: None
- Superseded by: None

## Context

Next's Search, Top and Solved controls filter/sort only already loaded topic pages. A match or
high-reply topic on an older page is absent until a reader manually loads it. A local warning
makes the limitation visible but does not implement global selection. Browser-wide scans would
replace owner pagination with an unbounded client operation and still duplicate locale semantics.

## Decision

The existing topic-list filter gains `search`, `order` (`latest` or `top`) and `solved_only`.
Search is a trimmed, Unicode-lowercased literal substring of the hydrated effective-locale title
or slug, limited to 200 normalized Unicode characters. It is a Forum feed filter, not a new search index or
full-text relevance engine. Locale fallback remains the Content locale resolver's decision.
Solved means the canonical solution relation exists. Latest retains the existing pinned/activity
keyset order. Top orders by canonical `reply_count DESC`, then all latest keys including the UUID
tie-breaker. It is not a vote ranking. Counts remain the existing disclosed owner aggregates.

Selection is performed by the owner before output pagination. Candidate SQL pages remain bounded;
locale-aware selection may scan multiple such pages, just like audience/unread selection. No
cross-page total count is invented. Every returned item still passes the same audience authorization;
unread selection additionally applies the canonical read-state aggregate. Sparse selections refill
until the requested page is full or exhausted; a cursor requires another selected authorized item.

Opaque topic cursors carry ranking position and a fingerprint of tenant, viewer, surface/channel,
category/status, requested/fallback locale and normalized selection. They are not authorization:
every continuation rechecks current policy. A cursor from a different selection is rejected, never
silently reused. The existing independent read-model cursor uses an unbound latest position and
rejects a selected-feed cursor. The pre-release opaque encoding is cut over atomically, with no
old-format decoder; clients must restart a list when a stale cursor is rejected.

REST, GraphQL and native adapters delegate to the same filter/owner. Next resets pages and guards
late responses on query/tab/category/viewer changes, preserves owner order, and does no local
search/Top/Solved filtering. Leptos adapters carry the same optional selection; its current UI may
continue requesting the default latest feed without exposing all controls.

## Sources of truth and ownership

`rustok-forum` owns the selection over `forum_topics`, hydrated translations and
`forum_solutions`. Content owns locale resolution; the Forum audience and read-state owners
compose their existing visibility rules. DTOs and native/GraphQL/Next variables are projections,
not independent selection implementations. No Search module dependency is added.

## Invariants

- Existing tenancy, channel, locked-topic settings and category/topic audience rules remain mandatory.
- Empty search normalizes to no search. Oversized search and invalid/mismatched cursors fail validation.
- Selection changes never append pages from a previous selection, including unread searches.
- Candidate/page failures remain errors, not empty successful results or a transport fallback.
- Mutable reply/activity counts have the same live-keyset concurrency limits as the existing list;
  this change does not promise a multi-request snapshot across concurrent writes.

## Non-goals

No full-text/relevance engine, new counts, browser-wide scan, vote ranking, route change,
write-policy change or completion of the broader Forum audit/parity program.

## Data, transaction, and concurrency boundary

The operation is read-only and introduces no transaction or snapshot spanning requests. Existing
constraints and writes remain authoritative. Migration `m20261017_000039_add_forum_top_feed_indexes`
adds global and category-scoped tenant-first Top keyset indexes with explicit PostgreSQL null
placement; down drops those two indexes only. Latest's index remains unchanged. There is no data
rewrite or persisted selection. Counter/index writes incur additional index maintenance.

## Context dimensions

Tenant, principal, category/status, effective requested/fallback locale, channel and surface
participate in the cursor fingerprint. Search, order and solved selection participate; page size
and trace/deadline do not. Authorization and unread status are re-evaluated per request, not frozen
in a cursor. The existing read-state owner continues using these underlying topic positions;
clients reset pages when toggling unread. Timezone/currency are not selection dimensions.

## Events and projections

No new event, outbox entry or cache is written. Top reads the existing canonical reply counter;
no new ranking projection or rebuild worker. Solution presence reads the existing tenant-scoped
relation. Counter reconciliation remains the existing owner's responsibility.

## Failure semantics

Invalid search/cursor/context fails validation. Database, locale, policy and read-state failures
propagate, never become empty success or trigger another transport. The UI can retry a first page
without a stale cursor. Current authorization is checked again on continuation.

## Migration and cutover

Apply the owner migration chain before deploying readers. Deploy adapters and owner together;
old opaque cursor tokens are rejected without a compatibility decoder. Restart their list from
the first page. No persisted cursor migration, dual reader or temporary fallback is introduced.

## Alternatives considered

Client-only filters and automatically downloading every topic were rejected. A separate Search
index would change matching/availability semantics and cannot be the source of truth for this
owner list. SQL-only matching was not chosen because it would duplicate the locale fallback and
Unicode substring contract; the existing bounded candidate scan preserves those semantics.

## Verification

Owner fixtures must cover sparse search/solved pages, top ordering/ties, locale fallback, hidden
matches, context-mismatched cursors, unread composition and failures. Adapter/UI tests must prove
selection forwarding and stale-response rejection. Scoped checks and outstanding runtime evidence
are recorded in `crates/modules/rustok-forum/docs/implementation-plan.md`.

## Consequences

Search/Solved can scan many bounded localized candidate pages for a sparse match; memory remains
bounded, but total request cost is not constant in topic count. Production measurements and
PostgreSQL EXPLAIN/buffer evidence remain required under the existing keyset ADR; a small SQLite
fixture or index definition is not proof of production performance. Maintainers must execute the
Rust/native feature checks and runtime fixtures before implementation is marked complete.
