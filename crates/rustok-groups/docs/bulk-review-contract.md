# Groups membership application bulk review contract

Status: `implemented_source`; runtime, parity, replay, concurrency, accessibility, and recovery evidence remains open under `GROUPS-06`.

## Boundary

`GroupApplicationBulkReviewCommandPort` is owned by `rustok-groups`. It accepts one
confirmed review decision for a bounded set of membership application IDs and returns
one result per requested ID.

The operation is intentionally a partial-result batch, not one cross-application
transaction:

- the request contains between 1 and 50 unique application IDs;
- `confirmed` must be `true` at the FFA core and owner boundaries;
- review note normalization and envelope validation complete before the first item
  transaction;
- every item uses the authorization-first owner review path and therefore receives its
  own application/group locks, transaction, authorization check, immutable audit row,
  command receipt, membership transition, and group-version update;
- authorization completes before pending-status disclosure;
- success for one item is not rolled back when a later item fails;
- the response preserves request order and exposes `result` or typed `PortError` for
  every application ID, plus succeeded/failed counters;
- the batch-level `PortContext` requires deadline and idempotency semantics;
- each item receives a bounded deterministic child idempotency key derived from the
  batch key and application UUID, independent of request order;
- replay of a committed item returns its receipt result; changing the decision or note
  under the same batch/application key is rejected by the existing request-hash guard;
- no implicit native/GraphQL fallback is allowed.

## GraphQL

The final Groups mutation root composes
`bulkReviewGroupMembershipApplications(idempotencyKey, input)` through
`GroupsApplicationBulkReviewMutation`.

Input:

- `applicationIds: [UUID!]!`;
- `decision: APPROVE | REJECT`;
- optional bounded review note, validated before item writes and again by each owner
  review command;
- `confirmed: Boolean!`.

Output:

- ordered item results;
- successful `ReviewGroupMembershipApplicationResultGql` when committed or replayed;
- stable error code/message/retryable fields when an item fails;
- succeeded and failed counters.

## FFA contract

The module-owned admin package provides:

- framework-neutral preparation that normalizes UUIDs, enforces unique 1..50 selection,
  bounds the note, requires confirmation, and generates one batch idempotency key per
  operator action;
- native `#[server]` and GraphQL adapters;
- one explicit no-fallback transport facade operation,
  `groups.admin.applications.bulk_review`;
- a mounted Leptos workspace that loads pending applications, supports bounded row
  selection, invalidates confirmation whenever decision/note/selection changes,
  disables submit without selection and confirmation, and renders ordered per-item
  success/failure/replay results;
- English and Russian catalog entries, including ARIA selection labels;
- polite live regions for load/busy/summary/per-item result announcements.

## Open evidence

The source contract does not promote FFA/FBA readiness. PostgreSQL migration/runtime,
native/GraphQL parity, idempotent replay, partial-failure recovery, concurrent review,
lock ordering, keyboard/focus behavior, screen-reader behavior, security, retry, and
performance evidence remain required. ProfilesReader summaries and semantic
application-review events remain later integration work.
