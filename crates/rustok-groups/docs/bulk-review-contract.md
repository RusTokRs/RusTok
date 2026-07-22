# Groups membership application bulk review contract

Status: `implemented_source_backend`; admin FFA confirmation/results UI remains open under `GROUPS-06`.

## Boundary

`GroupApplicationBulkReviewCommandPort` is owned by `rustok-groups`. It accepts one
confirmed review decision for a bounded set of membership application IDs and returns
one result per requested ID.

The operation is intentionally a partial-result batch, not one cross-application
transaction:

- the request contains between 1 and 50 unique application IDs;
- `confirmed` must be `true` at the owner boundary;
- every item reuses the existing owner review command and therefore receives its own
  application/group locks, transaction, authorization check, immutable audit row,
  command receipt, membership transition, and group-version update;
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
- optional bounded review note, validated again by the owner review command;
- `confirmed: Boolean!`.

Output:

- ordered item results;
- successful `ReviewGroupMembershipApplicationResultGql` when committed or replayed;
- stable error code/message/retryable fields when an item fails;
- succeeded and failed counters.

## Remaining FFA work

The admin application workspace still needs:

- bounded row selection capped at 50;
- explicit approve/reject confirmation displaying the selected count;
- disabled submit while no rows are selected or confirmation is absent;
- one generated batch idempotency key per operator action;
- visible per-item success/failure/replay results;
- focus restoration and screen-reader announcements;
- native and GraphQL adapters through the existing no-fallback transport facade.

No runtime, parity, replay, concurrency, accessibility, or migration evidence is claimed
by this source-only backend slice.
