# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@183c78d8c76ffb14ba9e37179e5a13fa693e11de` and continued on
`agent/index-product-relation-freshness-20260807`. The intervening Forum/Reactions/Pages work is
disjoint from this Product/Channel freshness slice.

`implementation-plan.md` remains historical architecture context. This file is the current execution
cursor.

## Current primary cursor

`M6 - execute and admit concrete repair PostgreSQL evidence`

The repair implementation, recovery policy, PostgreSQL harness, and retained-evidence admission source
are complete. Maintainer execution of the locked packet remains required and is not claimed by source
inspection. Independent M7 source work can continue while that owner-execution gate is pending.

## Current source-complete foundation

- mutation-source registry and commit-before-ack worker contract;
- bounded replay/reconciliation/retry/dead-letter/drift/repair foundations;
- Product locale and ProductVariant refresh ledgers plus durable relay cursor;
- Product/ProductVariant retained hard-delete identities;
- one canonical Product Index source and one canonical ProductVariant source;
- Product `variants` and `sales_channels` graph links;
- Product-owned Product-to-SalesChannel relation snapshots with independent `relation_epoch`;
- bounded cross-owner Product visibility to SalesChannel UUID resolver;
- Product-owned graph `projection_epoch` as the one complete Product mutation clock;
- projection-aware Product locale absence;
- Product-SalesChannel freshness witness ledger;
- tenant-scoped Channel identity generation;
- live Product replay/absence source-admission freshness gate;
- persisted tenant schema readiness gate.

## Canonical Product policy

Product Index has no parallel Product compatibility implementations. The selected distribution
registers one current Product schema through `product-postgres-primary`, one current ProductVariant
schema through `product-variant-postgres-primary`, and the current SalesChannel schema through
`sales-channel-postgres-primary`.

The generic numeric `SchemaVersion` inside `SchemaRef` remains an Index storage/routing primitive only;
it is not a Product compatibility matrix. The current Product graph contains Product scalars,
`variant_ids`/`variants`, and `sales_channel_ids`/`sales_channels`. Product visibility slugs stay
owner-side resolver input rather than transitional Index fields.

## Membership, ordering, and freshness are separate

1. `relation_epoch` changes only when resolved Product-to-SalesChannel UUID membership changes.
2. `projection_epoch` advances when complete Product record inputs move and is the only Product Index
   mutation `source_version`.
3. `product_sales_channel_index_relation_freshness_snapshots` records that one retained relation epoch
   was verified against current Product visibility and a Channel identity generation.

A freshness-only change never fabricates a relation membership change.

## Channel identity generation

`rustok-channel` owns `channel_index_identity_generations`, one durable generation per tenant.
Transactionally observed identity changes advance it for Channel insert/delete/id/tenant/canonical-slug
changes. `is_active` and unrelated Channel configuration do not invalidate Product relation identity.

Generation `0` represents a tenant with no historical Channel identity row. After the first identity
mutation, the positive generation is retained even if the tenant later has zero Channels.

## Freshness watermark source complete

For an exact Product, the distribution resolver now:

1. observes Product visibility, Product `index_revision`, tenant Channel identity generation, and
   resolved UUID membership under `REPEATABLE READ`, `READ ONLY`;
2. writes membership through `ProductSalesChannelIndexRelationStore`;
3. re-observes current owner facts;
4. requires the second UUID set to equal retained relation membership;
5. records the second observation through `ProductSalesChannelIndexRelationFreshnessStore`.

The Product owner accepts a freshness witness only for a live Product and the current retained
`relation_epoch`, under lock order Product row -> relation advisory lock -> freshness advisory lock.
Direct SQL inserts use the same DDL guard and lock order.

Live Product replay and Product locale absence fail closed unless the latest witness for the exact
projection relation epoch matches the current canonical visibility key and current tenant Channel
identity generation. A witness Product watermark may be older than the current Product revision only
when current visibility still matches; unrelated Product updates therefore do not falsely stale the
relation. Product hard-delete replay does not require a live freshness witness because it removes the
graph.

This completes **source admission** freshness fencing, not materialized-view convergence. Source read
and Index mutation application are not one cross-owner transaction: a Channel identity change can
commit after a Product source page was read but before its already-produced mutation is applied. The
next source read will reject the old witness, but authoritative query freshness still depends on
bounded automatic convergence or an equivalent materialized/query fence plus retained evidence.

## Event-contract admission status

Canonical Product typed Index events remain blocked on event-contract digest admission. This pass does
not run the generator or claim retained verify evidence.

Required sequence remains:

1. establish canonical digest status for reviewed `main`;
2. commit reviewed generator output if drift exists;
3. retain successful verify evidence;
4. add the one canonical Product typed event family;
5. register concrete routes/consumers and retain redelivery evidence.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh ledgers and durable relay step.
- [ ] Retain canonical event-contract digest admission for current main.
- [ ] Add canonical Product Index typed event family and concrete routes/consumers.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence.

## M6 replay, reconciliation, diagnosis, and repair

- [x] Bounded scan/load and stable replay identities.
- [x] Durable jobs, leases, checkpoints, multi-page replay, cancellation, and reconciliation.
- [x] Source timeout, dry-run, cooperative interruption, retry and dead-letter recovery.
- [x] Drift discovery/confirmation/finding lifecycle and targeted repair.
- [x] Concrete missing-entity/orphan-link repair and prepared-command recovery.
- [x] Real-migration PostgreSQL repair harness and retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Retain multi-host/restart/graceful-shutdown/command-transport evidence.
- [ ] Add remaining locale/partition checkpoint dimensions and explicit rebuild modes.

## M7 Product/ProductVariant/SalesChannel production graph

- [x] Canonical Product, ProductVariant and SalesChannel bounded sources.
- [x] Product `variants` and `sales_channels` links.
- [x] Product/ProductVariant retained delete semantics.
- [x] Product-to-SalesChannel relation membership ledger and bounded resolver.
- [x] Canonical Product graph projection epoch and projection-aware Product absence.
- [x] Product-SalesChannel freshness witness.
- [x] Channel identity generation.
- [x] Canonical Product replay/absence fail-closed source freshness gate.
- [x] Remove parallel Product/ProductVariant compatibility implementations.
- [ ] Execute PostgreSQL evidence for schema readiness, relation/freshness storage, resolver
      convergence, projection concurrency/delete ordering, and canonical replay.
- [ ] Implement bounded automatic owner-change convergence or an equivalent materialized/query
      freshness fence; specifically cover the source-read -> mutation-apply in-flight window.
- [ ] Admit canonical Product typed wire events/routes/consumers after digest verification.
- [ ] Retain Channel create/delete/slug/identity, Product visibility, retry/restart/delete-recreate,
      out-of-order, locale fan-out, in-flight mutation, and freshness evidence.
- [ ] Prove complete Product/Variant/Channel query parity.
- [ ] Move Storefront traffic only after readiness/equivalence/materialized-freshness evidence passes.

## Next implementation step

Primary owner step remains: execute and admit the locked M6 repair PostgreSQL packet.

Next unblocked M7 source step: compose **automatic freshness convergence** from existing owner changes
without weakening the fail-closed source watermark. Prefer a bounded durable queue/checkpoint or an
equivalent materialized freshness fence over a blind background sweep. Keep typed Product event work
separately blocked until digest admission.

## Maintainer verification for this slice

```bash
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
