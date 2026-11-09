# Distribution Index owner read contracts

- Date: 2026-11-08
- Decision status: Accepted
- Implementation status: In progress
- Owners: Product, Channel, Index, Distribution
- Extends: [Index PostgreSQL storage model](./2026-07-24-index-storage-layout.md)
- Supersedes: None
- Superseded by: None

## Context

Distribution selects the modules and adapters in a RusToK build. The Product
and SalesChannel Index adapters also contain queries against owner-private
tables. That couples Distribution to the owners' physical schemas.

## Decision

Product and Channel own their embedded PostgreSQL read contracts for replay,
freshness, absence, and relation resolution. Each owner defines the projection,
columns, and predicates that may be consumed by an adapter.

## Sources of truth and ownership

Product and Channel own their tables and the semantics of the published SQL.
Distribution binds inputs and registers composed Index capabilities. Index owns
its schema, storage, query execution, and replay and refresh machinery.

## Invariants

Only the owner queries its private tables. Distribution composes the owner
projections without acquiring a Rust dependency from either owner to the other
or to Index. Runtime values remain bound.

## Non-goals

This decision covers the embedded PostgreSQL Index adapter. A remote Index
provider needs a separate consistency contract.

## Data, transaction, and concurrency boundary

The native PostgreSQL adapter composes trusted owner SQL in the same statement
or existing repeatable-read transaction. The materialized freshness predicate
runs before Index filters, orders, paginates, or counts.

## Context dimensions

Tenant scopes every owner read. Product admission compares its source revision
and exact locale and the tenant's current Channel identity generation. These
are internal worker capabilities and preserve the existing authorization boundary.

## Events and projections

Replay retains the existing source identity, revision, and tombstone semantics.
The HTTP host maps the published Product event envelope into Index delivery
and uses the registered route schema. Index persists the refresh before the
worker acknowledges the delivery.

## Failure semantics

Missing or stale evidence fails closed. Invalid refresh events and unresolved
routes leave the broker offset uncommitted.

## Migration and cutover

The cutover moves query ownership while preserving the database schema, routes,
event names, and source identity. It requires no database migration.

## Alternatives considered

Separate port round trips create consistency windows. They would need a different
admission mechanism with equivalent visibility, paging, and count guarantees.

## Verification

Check the Distribution dependency boundary and run the selected Product and
Channel replay, freshness, absence, and refresh tests. The PostgreSQL tests
require a disposable PostgreSQL database. Run documentation and ADR checks.

## Consequences

Owners can change their storage implementation while maintaining their published
projection. The native adapter requires the owner projections to execute in one
PostgreSQL snapshot.
