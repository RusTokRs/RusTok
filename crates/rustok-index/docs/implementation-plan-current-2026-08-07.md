# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@68d8c86b1371929bc096576f204f730a7a7b9bb9` and continued on
`agent/index-linked-target-recreate-postgres-evidence-20260807`.

`implementation-plan.md` remains historical architecture context. This file is the current execution
cursor.

## Current primary cursor

`M6 - execute and admit concrete repair PostgreSQL evidence`

The repair implementation, recovery policy, PostgreSQL harness, and retained-evidence admission source
are complete. Maintainer execution of the locked packet remains required and is not claimed by source
inspection. Independent M7 source work continues while that owner-execution gate is pending.

## Current source-complete foundation

- mutation-source registry and commit-before-ack worker contract;
- bounded replay/reconciliation/retry/dead-letter/drift/repair foundations;
- persisted tenant schema readiness;
- Product locale and ProductVariant refresh ledgers plus durable relay cursor;
- one canonical Product, ProductVariant, and SalesChannel source contract;
- Product/ProductVariant/SalesChannel retained hard-delete identities;
- Product `variants` and `sales_channels` links;
- Product-owned Product-to-SalesChannel relation snapshots with independent `relation_epoch`;
- bounded cross-owner Product visibility -> SalesChannel UUID resolver;
- Product-owned graph `projection_epoch` as the complete Product mutation clock;
- projection-aware Product locale absence;
- Product-to-SalesChannel freshness witness and tenant Channel identity generation;
- Product visibility convergence requests plus tenant lease/checkpoint state;
- bounded generic ModuleWork Product/Channel convergence with restart/reclaim and rejected-Product
  isolation;
- generic schema-scoped PostgreSQL **entity** query admission covering root and linked materialized
  entity aliases;
- Product, ProductVariant, and SalesChannel owner freshness admission rules;
- recreate-safe ProductVariant and SalesChannel `index_revision` through retained tombstone seed/clear
  protocols.

No parallel Product/ProductVariant compatibility source is selected. Generic numeric `SchemaVersion`
values remain Index storage/routing keys only; do not introduce a new Product schema/version to solve
freshness or ordering.

## Clock and ownership boundaries

The current Product graph intentionally keeps distinct facts:

1. `relation_epoch` — Product-to-SalesChannel UUID membership changes only;
2. `projection_epoch` — complete Product record mutation ordering;
3. Product relation freshness witness — verifies one relation epoch against current Product visibility
   and Channel identity generation;
4. Channel identity generation — tenant-scoped resolver invalidation/freshness, not SalesChannel entity
   source version;
5. ProductVariant `index_revision` — ProductVariant entity mutation version;
6. SalesChannel `index_revision` — SalesChannel entity mutation version;
7. Product/Variant/Channel entity query admission — prevents physically materialized stale rows from
   becoming query-authoritative.

A freshness-only Channel transition must not fabricate `relation_epoch` or `projection_epoch`. A target
entity update/recreate must not advance Product membership clocks merely to make the target payload
current.

## Product/Channel convergence source complete

Product owns append-only visibility requests and one tenant convergence state with request cursor,
completed Channel generation, in-progress sweep generation/Product cursor, lease, retry availability,
attempt count, and bounded error state.

The selected distribution registers `product_sales_channel_relation_convergence` through generic
`ModuleWorkScheduler` composition only when Product and Channel are both selected. Work is bounded to
one exact request or one bounded Product page. Product-owned `FOR UPDATE` claim state makes duplicate
discovery across hosts harmless; lease expiry preserves progress across restart.

Rejected Product owner data is isolated. Invalid visibility/oversized allowlists/too many targets leave
that Product individually stale without blocking later valid Products. Product correction enqueues a
new exact request; Channel identity correction advances generation and schedules another tenant pass.

## Materialized entity query freshness source complete

`rustok-index` owns module-neutral `PostgresQueryEntityAdmission` plus an exact-schema admission catalog.
The query runtime rejects owner rules for schemas absent from the immutable registry and builds one
schema-dispatch predicate. Runtime-local pass-through roots ensure governed target rules still apply
when a query root itself has no owner rule.

The admission predicate is inserted into every compiler-owned materialized entity relation used by page
SQL and exact count, including:

- `tN` root/ordinary joins;
- `mpN_tN` many projections;
- `mx_tN` many-filter `EXISTS` paths;
- `mo_tN` many aggregate-order paths.

It changes no user binds, selected columns, plan fingerprint, or cursor contract and fails closed for
unknown alias families or missing canonical `is_deleted = FALSE` anchors.

Current Product graph rules require:

- Product: current projection/revision/relation freshness/Channel generation/visibility request/locale;
- ProductVariant: live same tenant/UUID with `product_variants.index_revision == source_version`;
- SalesChannel: live same tenant/UUID with `channels.index_revision == source_version`.

This fences stale target payload participation. It does **not** yet define authoritative semantics for a
link that exists while its target has not been materialized or is temporarily unavailable.

## Recreate monotonicity source complete

A post-#3179 recheck confirmed no new owner clocks are required.

Product migration `m20260731_000004_add_product_index_tombstones` already preserves ProductVariant
reincarnation ordering:

- delete retains `OLD.index_revision + 1`;
- same tenant/UUID insert seeds live `index_revision` above retained source version;
- tombstone clears only after strict live supersession.

Channel migration `m20260731_000011_add_channel_index_tombstones` provides the same guarantee for
SalesChannel.

Therefore old materialized target versions cannot collide with a recreated incarnation's current owner
revision. Do not add another ProductVariant/SalesChannel version ledger for this purpose.

## Retained M7 PostgreSQL packets

Four packets are source-ready and execution/admission pending:

1. `product_materialized_query_freshness_postgres.rs`
   - delayed Product scalar mutation;
   - locale deletion after source read;
   - stale physical row excluded before filter/order/cursor/limit/exact count.
2. `product_channel_convergence_postgres.rs`
   - two generic scheduler hosts;
   - active lease exclusion/reclaim;
   - rejected Product isolation;
   - Product visibility race;
   - unchanged-membership freshness-only Channel transition;
   - changed-membership relation/projection transition.
3. `product_channel_identity_transitions_postgres.rs`
   - Channel create/delete;
   - tenant move affecting both tenant generations;
   - same UUID delete+recreate before convergence with unchanged membership and freshness-only Product
     recovery.
4. `product_linked_target_recreate_postgres.rs`
   - ProductVariant same UUID delete+recreate monotonic owner revision;
   - SalesChannel same UUID delete+recreate monotonic owner revision;
   - old target rows deliberately left physically materialized;
   - Product root brought back to current owner state;
   - stale Variant/Channel payload excluded from nested Product projections;
   - current target mutation restores the recreated payload.

None has been executed or admitted by the implementation agent.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh ledgers and durable relay step.
- [ ] Retain canonical event-contract digest admission for current `main`.
- [ ] Add the one canonical Product Index typed event family and concrete routes/consumers.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence.

## M6 replay, reconciliation, diagnosis, and repair

- [x] Bounded scan/load and stable replay identities.
- [x] Durable jobs, leases, checkpoints, multi-page replay, cancellation, and reconciliation.
- [x] Source timeout, dry-run, cooperative interruption, retry and dead-letter recovery.
- [x] Drift discovery/confirmation/finding lifecycle and targeted repair.
- [x] Concrete missing-entity/orphan-link repair and prepared-command recovery.
- [x] Real-migration PostgreSQL repair harness and retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Retain remaining multi-host/restart/graceful-shutdown/command-transport evidence.
- [ ] Add remaining locale/partition checkpoint dimensions and explicit rebuild modes.

## M7 Product/ProductVariant/SalesChannel production graph

- [x] Canonical Product, ProductVariant, and SalesChannel bounded sources.
- [x] Product `variants` and `sales_channels` links.
- [x] Product/ProductVariant/SalesChannel retained delete identities.
- [x] Product-to-SalesChannel relation membership ledger and bounded resolver.
- [x] Canonical Product graph projection epoch and projection-aware Product absence.
- [x] Product-to-SalesChannel freshness witness and Channel identity generation.
- [x] Product visibility convergence requests and bounded automatic generic ModuleWork convergence.
- [x] Rejected-Product poison isolation.
- [x] Product materialized root freshness fence.
- [x] Entity-level stale linked-target freshness fence for ProductVariant and SalesChannel.
- [x] ProductVariant/SalesChannel recreate-safe monotonic `index_revision` via retained tombstones.
- [x] Source-ready Product delayed-mutation/locale-deletion PostgreSQL packet.
- [x] Source-ready Product visibility/Channel convergence multi-host PostgreSQL packet.
- [x] Source-ready Channel create/delete/tenant-move/delete-recreate PostgreSQL packet.
- [x] Source-ready ProductVariant/SalesChannel linked-target recreate PostgreSQL packet.
- [x] Remove parallel Product/ProductVariant compatibility implementations.
- [ ] Define fail-closed semantics for **link present but target materialization unavailable/stale** so
      unavailable target data cannot be interpreted as authoritative null/empty relation state.
- [ ] Retain PostgreSQL query-equivalence evidence for that availability policy across nested
      projection, linked filtering, aggregate ordering, exact count, delete, recreate, and restart.
- [ ] Execute/admit the four retained Product M7 PostgreSQL packets.
- [ ] Execute any remaining schema-readiness/relation/freshness/projection concurrency evidence not
      already covered by those packets.
- [ ] Admit canonical Product typed wire events/routes/consumers after digest verification.
- [ ] Retain remaining out-of-order and locale fan-out evidence.
- [ ] Prove complete Product/ProductVariant/SalesChannel query parity.
- [ ] Move Storefront traffic only after readiness/equivalence/freshness/availability evidence passes.

## Next implementation step

Primary owner step remains: **execute and admit the locked M6 repair PostgreSQL packet**.

The next unblocked M7 source-design step is now **linked-target availability admission**: distinguish a
legitimately absent Product link from a link whose ProductVariant/SalesChannel target exists in the link
set but is not currently authoritative/materialized. Define one generic fail-closed behavior that works
for ordinary joins and many projection/filter/order SQL without adding owner-specific logic to the Index
compiler.

After that, retain a PostgreSQL equivalence packet for target-missing/stale/delete/recreate/restart
behavior. Do not add another Product schema, relation copy, or freshness clock. Typed Product event work
remains separately blocked until event-contract digest admission.

## Maintainer verification for current M7 packets

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_convergence_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_identity_transitions_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_linked_target_recreate_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-convergence-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-identity-transitions-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
