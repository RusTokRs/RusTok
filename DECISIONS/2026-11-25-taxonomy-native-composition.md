# Compose native Product and Groups Taxonomy adapters in Distribution

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-distribution, rustok-taxonomy, rustok-product, rustok-groups
- Extends: [Canonical FBA owner ports](./2026-11-11-canonical-fba-owner-ports.md)
- Supersedes: [Groups Taxonomy integration](./2026-11-12-groups-taxonomy-port-architecture.md) and the placement of Product's native Taxonomy adapter and its runtime dependency in the [Inventory and Taxonomy decoupling decision](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md)
- Superseded by: None

## Context

Placing the native bridges in Taxonomy meant building the vocabulary owner also
linked both consumer runtimes. A selected standalone Taxonomy capability needs
to build independently. Groups also used fabricated identities and labels when
its native provider was absent, so a successful categorized command did not
prove that Taxonomy had issued the category.

## Decision

The distribution composition root links both sides of the native Product and
Groups adapters. The monolith selects the Product adapter for its embedded
catalog. When both Groups and Taxonomy are present in the registry, Distribution
provides a factory that binds Taxonomy to the host database. Groups uses the
same runtime constructor for GraphQL and native server functions and honors an
explicit host-provided adapter.

## Sources of truth and ownership

Taxonomy owns terms, hierarchy, localization, routes, and tag persistence.
Product and Groups own their aggregate references and the shapes of native
calls that participate in their own transactions. Distribution owns the adapter
selection and DTO conversion. Published term-name reads use the Taxonomy-owned
contract in `rustok-taxonomy-ports`.

## Invariants

- The production Taxonomy, Product, and Groups crates build without one
  another's runtimes. The native distribution links the selected owners.
- Adapters call Taxonomy owner methods. The aggregate transaction remains the
  unit of atomicity for existing co-located writes.
- A categorized Group create uses one canonical category snapshot for validation,
  projection, and its durable receipt.

## Non-goals

This decision covers the native adapters and Groups category behavior. Remote
Taxonomy reads and cross-process write consistency follow the canonical FBA
decision and their owner plans. Inventory remains governed by its existing decision.

## Data, transaction, and concurrency boundary

Product category copy and tag writes reuse the Product transaction. A categorized
Group create reads the category inside the Group transaction after checking its
receipt and before writing the aggregate. The receipt preserves the committed
result during replay. Groups retains its nullable category reference. Taxonomy
continues to govern its own hierarchy and category lifecycle.

## Context dimensions

Taxonomy resolves the category within the caller's tenant and module scope.
Projection uses the requested locale and Taxonomy's locale policy. Groups and
Product continue to enforce access to their aggregates. The native connections
carry no separate network identity or channel context.

## Events and projections

Groups keeps its existing category identity and optional resolved summary in the
Group projection. Directory filtering uses the stored reference. Aggregate event
and receipt ownership stays with the existing writers. Group tag methods remain
available through the native contract for future group-tag commands.

## Failure semantics

Groups supports commands and reads without category references in deployments
that omit Taxonomy. A categorized command that requires an absent provider
returns an unavailable error. The selected provider's failures propagate.
An unknown category causes a validation error before a Group insert. A completed
receipt keeps its recorded result during a Taxonomy outage.

## Migration and cutover

Move both adapters with their production call sites and preserve the existing
schemas. The Product and Groups modules retain their locally owned references.
Any preexisting Groups reference that Taxonomy does not resolve projects no
category metadata. Module builds and the owner plans track subsequent remote
capability verification separately.

## Alternatives considered

Keeping the adapters in Taxonomy would preserve reverse runtime dependencies.
Promoting transaction handles to published contracts would couple deployments
to one database transaction. Synthetic category values would bypass owner
identity validation. Composition in Distribution preserves transaction behavior
with independent owner build graphs.

## Verification

The Taxonomy FBA layout lint and normal Cargo dependency graph cover standalone
packaging. Distribution's `product_taxonomy` and `groups_taxonomy` integration
tests cover composed owner operations, category presentation and routes, and
Groups provider selection, failures, and exact replay.

## Consequences

Taxonomy can ship its read capability without linking either consumer. The host
retains co-located native writes, and deployments that place owners on separate
databases need a consistency workflow before enabling those writes remotely.
