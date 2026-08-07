# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@ca0672bf5f4d022df0316708035e49bf5fb23dd9` and continued on
`agent/index-product-canonical-schema-20260807`.

This file is the current status overlay. `implementation-plan.md` remains historical architecture and
milestone context.

## Current primary cursor

`M6 - execute and admit concrete repair PostgreSQL evidence`

The repair implementation, recovery policy, PostgreSQL harness, and retained-evidence admission
source are complete. Maintainer execution of the locked evidence packet remains required and is not
claimed by source inspection.

Independent source work may continue while that owner-execution gate is pending.

## Current source-complete foundation

- mutation-source registry and commit-before-ack worker contract;
- Social Graph production mutation route and consumer policy;
- bounded replay, retry/dead-letter/requeue, reconciliation, scheduling, drift diagnosis, findings,
  targeted repair, and prepared-command recovery;
- Product locale and ProductVariant refresh ledgers plus durable relay cursor;
- Product/ProductVariant retained hard-delete identities;
- Product-owned Product-to-SalesChannel relation snapshots with independent monotonic relation epoch;
- bounded cross-owner Product visibility to SalesChannel UUID resolver;
- Product-owned complete graph `projection_epoch` combining Product and relation watermarks;
- one canonical Product Index source with ProductVariant and SalesChannel links;
- one canonical ProductVariant Index source;
- projection-aware Product locale absence semantics;
- fail-closed persisted tenant schema readiness gate.

## Canonical Product policy

Product Index no longer keeps parallel compatibility implementations.

The selected distribution registers exactly:

- one current Product schema through `product-postgres-primary`;
- one current ProductVariant schema through `product-variant-postgres-primary`;
- one current SalesChannel schema through `sales-channel-postgres-primary`.

The generic Index key still contains a positive numeric `SchemaVersion`; that is an Index storage
primitive, not a Product compatibility matrix. Old Product/ProductVariant schema branches, old replay
event-domain branches, and the shared version-switching `graph.rs` implementation are removed.

The current Product record contains:

- Product identity/scalars;
- `variant_ids` and `variants` link;
- `sales_channel_ids` and `sales_channels` link.

Product visibility slug metadata stays owner-side input to the resolver and is no longer duplicated as
transitional Index fields.

The complete Product record uses `projection_epoch` as its single mutation `source_version`.
`products.index_revision` and relation `relation_epoch` remain retained component watermarks only.

## Product graph projection

`product_index_graph_projection_snapshots` is the canonical owner projection storage. A compatibility
migration renames the short-lived versioned runtime object names while preserving already-written
rows, then installs canonical trigger/function names.

For one tenant/Product identity:

- projection epoch begins at 1 and advances exactly once;
- Product and relation input watermarks cannot regress;
- unchanged input pairs do not append;
- rows are append-only;
- Product insert/update/delete and relation insert reconcile projection state;
- hard delete first retains final empty relation membership, then reconciles final graph state.

The canonical Product replay source joins the exact retained relation epoch referenced by projection
state. Live Product rows fail closed when the retained Product watermark does not equal the current
`products.index_revision`. Product absence uses the same projection clock.

## Remaining freshness boundary

Projection ordering does not prove cross-owner relation freshness. Product visibility or Channel
identity can change before the bounded resolver has converged the latest UUID membership.

Authoritative Product graph use therefore remains blocked until durable convergence triggering or an
admitted freshness watermark/checkpoint exists and PostgreSQL evidence proves it.

## Event-contract admission status

Product typed Index events remain blocked on canonical event-contract digest admission. The current
digest artifact has changed since the earlier stale-artifact observation, but this implementation pass
did not run the generator or retained verify workflow.

Required sequence:

1. maintainer establishes canonical digest status for reviewed `main`;
2. review/commit generator output if drift exists;
3. retain successful verify evidence;
4. add the Product Index typed event family using only the canonical Product/ProductVariant contracts;
5. then register concrete routes/consumers and retain redelivery evidence.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Database-neutral mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh owner ledgers and durable relay step.
- [ ] Retain canonical event-contract digest admission for current main.
- [ ] Add canonical Product Index typed event family and concrete routes/consumers.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence.

## M6 replay, reconciliation, diagnosis, and repair

- [x] Bounded scan/load and stable replay identities.
- [x] Durable jobs, leases, checkpoints, multi-page replay, cancellation, and reconciliation.
- [x] Source timeout, no-write dry-run, cooperative interruption, retry and dead-letter recovery.
- [x] Drift snapshot/digest, candidate discovery/confirmation, finding lifecycle and targeted repair.
- [x] Concrete missing-entity/orphan-link repair and prepared-command recovery.
- [x] Real-migration PostgreSQL repair harness and retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Retain multi-host/restart/graceful-shutdown/command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions and explicit rebuild modes.

## M7 Product/ProductVariant/SalesChannel production graph

- [x] Canonical Product, ProductVariant and SalesChannel bounded sources.
- [x] Product `variants` link.
- [x] Product `sales_channels` link backed by resolved UUID membership.
- [x] Product/ProductVariant retained delete semantics.
- [x] Product-to-SalesChannel owner relation ledger and bounded resolver.
- [x] Canonical Product graph projection epoch and projection-aware Product absence semantics.
- [x] Remove parallel Product/ProductVariant compatibility source/schema implementations.
- [ ] Execute PostgreSQL evidence for schema readiness, relation storage/resolver convergence,
      projection concurrency/delete ordering, and canonical replay.
- [ ] Add durable Product-visibility and Channel-identity convergence trigger/watermark.
- [ ] Admit canonical Product typed wire events/routes/consumers after digest verification.
- [ ] Retain Channel create/delete/slug/identity, Product visibility, retry/restart/delete-recreate,
      out-of-order, locale fan-out, and freshness evidence.
- [ ] Prove complete Product/Variant/Channel query parity.
- [ ] Move Storefront query traffic only after readiness/equivalence/freshness gates pass.

## Next implementation step

Primary owner step remains: execute and admit the locked M6 repair PostgreSQL packet.

Next unblocked source step for M7: add durable relation convergence/freshness triggering or an admitted
watermark so the canonical Product graph cannot materialize stale SalesChannel membership after owner
changes. In parallel, establish event-contract digest admission for the one canonical Product event
family.

## Maintainer verification for this slice

```bash
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-variant-source.mjs
node scripts/verify/verify-index-product-tombstone-source.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
