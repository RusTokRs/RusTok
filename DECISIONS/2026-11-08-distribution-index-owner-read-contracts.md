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

ProductVariant replay is scoped to Product and uses its typed replay port for
both in-process and gRPC reads. Product executes the same projection in both
modes. Distribution maps the returned records and tombstones into Index
mutations while preserving their revisions, cursor encoding, and source identity.
Localized Product replay also uses the owner port. In the embedded deployment
it reads the Channel generation in the same SQL statement. In the standalone
deployment it loads Product rows first, then checks the Channel generation over
the identity port before releasing live rows. A deleted row retains its graph
identity and can replay without a live Channel witness.

Channel exposes a tenant-scoped identity snapshot. A single Channel statement
returns the current generation and the matching UUIDs in canonical order. The
embedded resolver uses that statement inside its existing Product/Channel
snapshot. An independent caller can request the same snapshot through Channel's
read port. A generation read and a later list read cannot be paired as evidence
of membership.

Native localized-record owners may register the bounded `rustok-api::localized_index`
replay port. Distribution maps explicitly registered records into the existing
PostgreSQL Index source and schema catalogs. The owner returns exact locale keys,
owner revisions and tombstones for unpublished copies. Distribution and Index
read through the port. Localized record owners also publish a sealed
`index.localized_record.refresh_requested` fact with module, entity, record
and minimum committed revision. The selected Index listener acknowledges only
after fetching all exact locales in one owner statement and persisting the
corresponding revisioned mutations. Storefront admission remains independent.

## Sources of truth and ownership

Product and Channel own their tables and the semantics of the published SQL.
Distribution binds inputs and registers composed Index capabilities. Index owns
its schema, storage, query execution, and replay and refresh machinery.

## Invariants

Only the owner defines queries against its private tables. Distribution composes
published SQL for embedded reads that require a shared snapshot and calls the
ProductVariant replay port for variant replay. Neither owner acquires a Rust
dependency on the other or on Index. Runtime values remain bound.

## Non-goals

Remote localized Product replay, absence, Channel convergence, and serving
admission need a consistency contract for independent Product and Channel
databases. ProductVariant replay does not grant remote storefront serving.

## Data, transaction, and concurrency boundary

The native PostgreSQL adapter composes trusted owner SQL in the same statement
or existing repeatable-read transaction. The materialized freshness predicate
runs before Index filters, orders, paginates, or counts.

The ProductVariant replay port reads owner rows and tombstones in a single
Product statement. Index compares the event's minimum source revision and
persists to its own database before acknowledging the broker delivery.

Channel's identity snapshot proves which UUIDs matched at its reported generation.
Product checks the exact visibility and index revision under the Product row lock.
It commits the membership, the graph projection advance, and the freshness
witness in one Product transaction. An older Channel generation cannot replace a
witness. If the observation has the latest witness's visibility and generation,
its membership must match. A repeat observation is idempotent. Remote replay must still compare
the witness with current Channel authority. Publishing a witness does not freeze either owner. Serving needs
a separate admission fence that applies before filtering, ordering, pagination,
and counting. A delayed event or cached generation cannot establish current admission.
For standalone convergence, Product reads its visibility and revision, then
calls the configured Channel identity port. It commits through the same
revision-checked writer as the embedded resolver and re-observes both owners
before completing work. Both profiles use Product's leased cursors and the
same work execution. The standalone process discovers work only for its
configured tenants.
The generation belongs to one Channel history. A restore that rewinds that
history requires rebuilding dependent witnesses and Index projections before
they can serve queries.

## Context dimensions

Tenant scopes every owner read. Product admission compares its source revision
and exact locale and the tenant's current Channel identity generation. These
are internal worker capabilities and preserve the existing authorization boundary.

## Events and projections

Replay retains the existing source identity, revision, and tombstone semantics.
Replay and incremental refresh derive the same delivery identity from the source
schema, tenant, record, exact locale and owner revision. A retry after a partial
batch skips completed locale writes. A later revision wins over an older
snapshot, and a missing or behind owner snapshot blocks acknowledgement.
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
