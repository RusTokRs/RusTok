# FORUM-20BJ — exact public Search projection

`FORUM-20BJ` wires Forum discovery into the Search-owned `search_documents`
projection without moving Forum visibility policy into Search SQL.

## Ownership

Forum remains the only owner of category and topic visibility. The module
publishes `ForumSearchProjectionSourceFactory` through the neutral
`SearchProjectionSourceRegistry`. Registration carries no database handle; the
Search event listener materializes the source from its runtime database only
after all module runtime extensions have been assembled.

The Forum source scans category/topic translation identities in bounded raw
pages. Every candidate is then re-read through `ForumPublicDiscoveryService`,
which already composes the inherited category floor, richer category layers,
open-topic state, route-channel rules and topic-local narrowing. Denied,
closed, missing, foreign and authentication-only candidates produce no Search
document. Search never evaluates or persists a copy of those audience rules.

Public Forum Search currently projects:

- one `forum_category` document per exact public category translation;
- one `forum_topic` document per exact public topic translation;
- no reply documents.

A topic's category is reauthorized before its category subtitle is included.
Calling the source without a route channel intentionally excludes topics that
require a route-channel match.

## Bounded cursor behavior

The source cursor advances over raw translation candidates rather than visible
output. Therefore a page may contain fewer records than requested, or no
records at all, while still returning `next_cursor`. Search continues until the
raw cursor is exhausted. Page size and per-entity locale fan-out are bounded,
and a non-advancing cursor is rejected.

## Search-owned persistence

`ForumSearchProjector` owns all writes to `search_documents`.

An explicit tenant Forum rebuild creates a PostgreSQL temporary staging table,
streams bounded authorized source pages into it, and replaces the existing
Forum category/topic scope only after the complete scan succeeds. An error
before replacement rolls back that Forum transaction and keeps the previous
Forum scope intact.

A targeted refresh deletes the existing entity/locale documents and inserts
the currently authorized records in one transaction. When the owner target is
missing, deleted, closed or no longer public, the empty authorized result
therefore removes stale documents.

The broader `search`/tenant/locale rebuild still executes the existing core and
Blog projectors before the Forum projector. Its replacement is not one atomic
transaction across all source modules; a later Forum source failure therefore
does not promise restoration of the Forum scope removed by the existing core
rebuild sequence. Cross-source atomic rebuild or per-source preservation remains
explicit downstream work.

The existing Search trigger continues to own `search_vector`; Forum supplies
only normalized document inputs. No Search or Forum migration is added.

## Event composition

When the Forum source is registered, Search refreshes a topic after existing
root events for topic create, reply, status, pin and reply-status changes.
Forum module enable rebuilds the scope and disable deletes it. Full Search,
tenant and locale rebuilds invoke the Forum projector.

Explicit reindex requests support:

- `forum` with no target for an atomic Forum scope rebuild;
- `forum_category` with an ID for a category refresh;
- `forum_topic` with an ID for a topic refresh.

Category/topic audience-policy replacement does not yet publish a root Search
reindex event. `FORUM-20BK` must add owner-transactional policy-change events
and route them to bounded category/topic or tenant reauthorization. Until then,
explicit Forum reindex remains the recovery path for policy-only changes.

## Compatibility and validation

Existing Search content, product and Blog projectors and query contracts are
unchanged. Existing Forum REST, GraphQL, storefront, SEO and public-discovery
contracts are unchanged. Forum now depends on the core Search crate only for
the neutral projection-source contract and declares the always-active Search
module dependency.

The workspace `Cargo.lock` is intentionally not regenerated in this slice,
because the implementation agent was instructed not to run Cargo commands. The
maintainer-owned Cargo validation workflow must regenerate and review the one
workspace dependency-edge update before locked validation.

Tests, Cargo commands, formatting, verifiers, workflows and CI were not run by
the implementation agent. The maintainer commands are recorded in
`contracts/forum-search-projection.json`.
