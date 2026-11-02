# Forum owner-write acknowledgements do not re-read hidden content

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: [Forum anonymous discovery and category counts](./2026-10-15-forum-anonymous-discovery-and-category-counts.md)
- Supersedes: None
- Superseded by: None

## Context

Vote set/clear for topics and replies, and solution mark/clear are commands, not selected-object reads. The existing post-commit read can reject the caller under an audience policy that differs from the authorized write and produce 404 after the write has already committed.

## Decision

These six commands return only the owning service's successful completion:
- REST: `204 No Content`, no body.
- GraphQL: `Boolean!` with `true` after a successful commit.

## Sources of truth and ownership

`rustok-forum` owns vote/solution state and command completion semantics.

## Invariants

### Allowed states
- A successful write command returns 204 No Content or Boolean! true.

### Forbidden states
- Re-reading hidden content post-commit to synthesize a read payload during a write command.

## Non-goals

- Altering underlying vote/solution storage schema or outbox transactions.

## Data, transaction, and concurrency boundary

Changes transport result shapes only. Existing database transactions and outbox atomicity remain intact.

## Context dimensions

- Tenant: Verified against TenantContext before owner service writes.
- Auth/Policy: RBAC and write-audience enforcement apply before mutation.

## Events and projections

Vote clear emits no topic projection invalidation when no-op. Full object updates require an authorized read.

## Failure semantics

Fails closed on validation, authorization, or database errors; no success is synthesized on error.

## Migration and cutover

No legacy response DTO is retained. Consumers update query selection sets.

## Alternatives considered

1. **Post-write projection read:** Rejected because audience policy changes can cause false 404s after committed writes.

## Verification

Scoped GraphQL and REST tests verify 204 No Content and Boolean! true return shapes.

## Consequences

Eliminates post-commit read failures and decouples write acknowledgements from viewer visibility.
