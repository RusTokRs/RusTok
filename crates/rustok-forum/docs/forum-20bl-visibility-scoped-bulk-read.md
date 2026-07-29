# FORUM-20BL — exact visibility-scoped bulk read commands

`FORUM-20BL` closes the category-subtree and tenant-wide read-state authorization
gap left by the initial resumable bulk owner. Public REST, GraphQL and
module-owned storefront transports now route those commands through the same
exact category/topic audience owners used by authenticated storefront reads.

## Owner composition

The exact command is owned by `ForumVisibilityScopedReadStateService`. It does
not copy category or topic policy into read-tracking SQL. For every request it:

1. validates the authenticated tenant/user `PortContext`;
2. normalizes the current route channel through `ForumTopicVisibilityScope`;
3. verifies the requested category root, when present, through
   `ForumCategoryAudienceVisibilityService`;
4. scans at most 100 raw topic candidates in stable `(created_at, id)` order;
5. reauthorizes every candidate through
   `ForumTopicAudienceVisibilityService` with the current route channel and
   optional host-published trust/Channel/Groups facts capability;
6. writes monotonic high-water state only for admitted topic IDs through the
   existing `forum_topic_read_states` helpers.

Missing, foreign or denied category roots are returned as `CategoryNotFound`.
Denied topic candidates are omitted without returning their identity or count.
No transport has a local SQL visibility predicate or fallback policy.

## Raw cursor versus visible writes

The exact cursor version is `brv1`. It is bound to:

- tenant ID;
- authenticated user ID;
- category-subtree or tenant-wide scope;
- a digest of the normalized route channel;
- the initial snapshot timestamp;
- the last raw `(created_at, topic_id)` candidate.

The cursor advances over raw candidates rather than visible results. Therefore a
bounded page may legitimately return:

```text
processed = 0
has_more = true
next_cursor = <opaque brv1 cursor>
```

This is required to make progress across private, closed or route-channel-denied
runs without exposing how many hidden topics exist. `processed` counts only
read-state rows actually authorized for the current page. Resuming the cursor in
a different tenant, user, category/all scope or route channel is rejected.

The snapshot excludes topics created after the first page. Current visibility is
re-evaluated on every resumed page, so later policy or membership changes cannot
reuse a stale admission decision.

## High-water semantics

The command reuses the established monotonic owner helpers:

- latest approved reply position;
- latest topic revision;
- conflict-safe maximum high-water upsert.

A replay is idempotent. A denied topic never receives a read-state row. The
slice adds no table, migration, counter or alternate unread projection.

## Transport composition

Existing public contracts remain available at the same names:

- REST `POST /api/forum/categories/{id}/mark-read`;
- REST `POST /api/forum/topics/mark-all-read`;
- GraphQL `markForumCategoryRead`;
- GraphQL `markAllForumTopicsRead`.

Those four entry points now use the exact owner rather than the trusted legacy
`br1` methods. Request DTOs still contain only cursor and limit; tenant, user,
route channel and locale come from authenticated transport context.

The module-owned storefront additionally exposes parallel selected transports:

- GraphQL `markForumStorefrontCategoryRead` and
  `markAllForumStorefrontTopicsRead`;
- native server functions `forum/storefront-category-read` and
  `forum/storefront-all-read`.

The storefront transport facade selects exactly one compile-profile path and
has no native-to-GraphQL or GraphQL-to-native fallback. This slice adds transport
parity only; it does not add or claim completed UI controls.

## Compatibility and remaining work

The original `br1` category/all methods remain available for trusted direct
compatibility and existing owner regression coverage. Public bulk transports no
longer call them. Single-topic mark-read and unread-list behavior are unchanged.
No route, REST DTO, workspace dependency, lockfile, migration or readiness status
changes are introduced.

The large canonical `implementation-plan.md` and `CRATE_API.md` remain
conflict-sensitive synchronization debt and are not rewritten through the
GitHub Contents API. The machine contract records the exact FORUM-16/FORUM-20
ledger update required after maintainer validation. FFA remains `in_progress`
and FBA remains `boundary_ready`; no central readiness promotion is claimed.

Tests, Cargo commands, formatting, verifiers, workflows and CI were not run by
the implementation agent. Suggested maintainer commands are recorded in
`contracts/forum-visibility-scoped-bulk-read.json`.
