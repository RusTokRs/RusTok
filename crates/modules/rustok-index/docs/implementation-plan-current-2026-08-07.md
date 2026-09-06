# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked from `main@283522e39d51b92e4fd3abef64f057edc375d546` and continued on
`agent/index-linked-target-replay-redelivery-evidence-20260807`.

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
- one canonical Product Index source, ProductVariant, and SalesChannel Index source contract;
- Product/ProductVariant/SalesChannel retained hard-delete identities;
- Product `variants` and `sales_channels` links;
- Product-owned Product-to-SalesChannel relation snapshots with independent `relation_epoch`;
- bounded cross-owner Product visibility to SalesChannel UUID resolver;
- Product-owned graph `projection_epoch` as the complete Product mutation clock;
- projection-aware Product locale absence;
- Product-SalesChannel freshness witness and tenant Channel identity generation;
- Product-owned visibility convergence requests and tenant lease/checkpoint state;
- bounded generic ModuleWork Product-SalesChannel automatic convergence with restart/reclaim and rejected-Product
  isolation;
- Freshness watermark source complete;
- Automatic owner-change relation convergence source complete;
- Materialized/query freshness fence source complete (covering the source-read -> mutation-apply window);
- generic PostgreSQL entity freshness admission for every compiler-owned materialized entity alias;
- Product, ProductVariant, and SalesChannel owner freshness rules;
- recreate-safe ProductVariant and SalesChannel `index_revision` through retained tombstone seed/clear;
- generic query-path-scoped linked-target availability admission for Product graph queries;
- source-ready linked-target filter/order/exact-count/runtime-recomposition PostgreSQL equivalence;
- source-ready canonical ProductVariant replay checkpoint-failure / duplicate / late-stale composition
  evidence through real PostgreSQL mutation storage and Product graph queries.

No parallel Product/ProductVariant compatibility source is selected. Generic numeric `SchemaVersion`
values remain Index storage/routing keys only; do not introduce a new Product schema/version to solve
freshness, availability, replay, or ordering.

## Clock and ownership boundaries

The Product graph keeps separate durable facts:

1. `relation_epoch` — Product-to-SalesChannel UUID membership changes only;
2. `projection_epoch` — complete Product record mutation ordering;
3. Product relation freshness witness — validates one relation epoch against Product visibility and
   Channel identity generation;
4. Channel identity generation — resolver invalidation/freshness, not SalesChannel entity source
   version;
5. ProductVariant `index_revision` — ProductVariant entity mutation source version;
6. SalesChannel `index_revision` — SalesChannel entity mutation source version;
7. entity query admission — owner freshness of a physically materialized entity row;
8. link-target availability admission — authority of a current link when a query actually traverses
   that link;
9. replay checkpoint — source progress only; it never replaces entity source-version authority.

A freshness-only Channel transition must not fabricate Product relation/projection epochs. Target entity
updates/recreates must not advance Product membership clocks merely to make target payload current.
Replay checkpoint loss after a durable mutation must not make the durable target non-authoritative or
allow an older target mutation to regress it.

## Product/Channel convergence source complete

Product owns append-only visibility requests and one tenant convergence state with request cursor,
completed Channel generation, in-progress sweep generation/Product cursor, lease, retry availability,
attempt count, and bounded error state.

The selected distribution registers `product_sales_channel_relation_convergence` through generic
`ModuleWorkScheduler` composition only when Product and Channel are selected. Work is bounded to one
exact request or one bounded Product page. Product-owned `FOR UPDATE` claim state makes duplicate
discovery across hosts harmless; lease expiry preserves progress across restart.

Rejected Product owner data is isolated. Invalid visibility/oversized allowlists/too many targets leave
that Product individually stale without blocking later valid Products.

## Materialized entity freshness source complete

`rustok-index` owns module-neutral `PostgresQueryEntityAdmission` plus an exact-schema owner catalog.
The query runtime rejects rules for schemas absent from the immutable registry and builds one owner
schema-dispatch predicate. Runtime-local pass-through roots ensure governed target rules still apply
when a query root itself has no owner rule.

Entity admission covers page and exact-count SQL aliases:

- `tN` root/ordinary joins;
- `mpN_tN` many projections;
- `mx_tN` many-filter `EXISTS` paths;
- `mo_tN` many aggregate-order paths.

Current Product graph owner rules require:

- Product: current projection/revision/relation freshness/Channel generation/visibility request/locale;
- ProductVariant: live same tenant/UUID with `product_variants.index_revision == source_version`;
- SalesChannel: live same tenant/UUID with `channels.index_revision == source_version`.

## Query-path-scoped linked-target availability source complete

Product registers exactly one generic link-target availability policy for its current Product
`SchemaRef`.

The query runtime uses validated `IndexQuery::referenced_paths()` to derive only the first-hop link names
that the current query traverses across selection, filtering, or ordering.

For scalar-only Product queries with no linked path, no availability predicate is injected. Such queries
remain independent of unrelated ProductVariant/SalesChannel materialization.

For a Product query that references `variants` and/or `sales_channels`, the query port injects the same
root precondition into page SQL and exact-count SQL before ordinary entity admission:

- inspect only `index_links` whose source identity and `source_version` exactly match the current
  materialized Product row;
- inspect only link names actually referenced by the query;
- require every current matching link row to have a live exact target row in `index_entities`;
- require that target row to pass the same owner freshness dispatcher used by normal query aliases;
- exclude the Product root if any current queried link target is missing, stale, or deleted.

Authority states are therefore:

- no current link row = authoritative absent relation;
- current link + current owner-admitted target = authoritative linked relation;
- current link + unavailable/stale/deleted target = fail closed for that Product graph query, not
  authoritative null/empty data.

The policy is generic Index storage/runtime logic. Product owner predicates still do not read
`index_links` or `index_entities`, and the generic SQL compiler has no Product-specific branch.

The policy is one-hop by design. Current canonical Product targets are link-free, so recursive target
availability is not required for this graph.

## Recreate monotonicity source complete

No additional ProductVariant/SalesChannel clock is required.

Product migration `m20260731_000004_add_product_index_tombstones` and Channel migration
`m20260731_000011_add_channel_index_tombstones` already:

- retain delete source version at `OLD.index_revision + 1`;
- seed same-UUID recreation above the retained source version;
- reject exhaustion;
- clear the retained tombstone only after strict live supersession.

Old materialized target versions therefore cannot collide with recreated incarnations.

## Linked-target availability equivalence source complete

`product_linked_target_availability_equivalence_postgres.rs` adds two Product roots with independent
Variant and SalesChannel targets.

For ProductVariant:

- SKU-only update increments Variant `index_revision` without changing Product membership/projection;
- stale old target remains physically materialized;
- linked `IN` filter that would otherwise match the stale old SKU returns only the unrelated current
  Product;
- `MIN(variants.sku)` ordering returns only the unrelated current Product;
- exact count is 1 on both query paths;
- a fresh query runtime on a new PostgreSQL session sees the same boundary;
- applying only the current Variant mutation restores the affected Product and exact count 2.

For SalesChannel:

- name-only update increments Channel `index_revision` without changing tenant Channel identity
  generation;
- Product relation/projection freshness therefore stays current;
- linked stale-name filtering and `MIN(sales_channels.name)` ordering return only the unrelated current
  Product with exact count 1;
- applying only the current Channel mutation restores both Products and exact count 2.

This packet is source-ready and execution-pending. It does not claim runtime test success.

## Linked-target replay/redelivery composition source complete

`product_linked_target_replay_redelivery_postgres.rs` reuses the canonical ProductVariant source and
generic `IndexReplayWorker` with real PostgreSQL `PostgresMutationStore`.

The packet starts with Variant v1 materialized, captures canonical Variant v2 without delivering it, then
advances owner state to v3. Product membership/projection stays current while the linked graph fails
closed because Variant materialization is still v1.

A bounded fail-once checkpoint adapter injects the exact generic crash boundary after v3 mutation
durability but before checkpoint durability:

- first replay applies canonical v3 into real PostgreSQL storage;
- checkpoint commit fails and remains absent;
- original and freshly composed query runtimes already expose v3 because target authority is durable and
  independent of replay cursor state;
- a newly created replay worker retries from the absent checkpoint, scans the same current owner state,
  derives the same stable event UUID and records `Duplicate` rather than rewriting the entity;
- retry commits a complete checkpoint at v3;
- the never-before-delivered canonical v2 mutation is then delivered late through the same replay sink
  and records `StaleIgnored`;
- a final exact v3 redelivery remains `Duplicate`;
- durable ProductVariant source version/payload and linked Product graph remain v3 throughout the late
  deliveries.

The injected checkpoint adapter is only a deterministic failure boundary. Durable PostgreSQL
checkpoint/job lease ownership remains the existing generic M6 contract; no Product-specific checkpoint
state is added.

This packet is source-ready and execution-pending.

## Retained M7 PostgreSQL packets

Six packets are source-ready and execution/admission pending:

1. `product_materialized_query_freshness_postgres.rs`
   - delayed Product scalar mutation and locale deletion;
   - stale physical row excluded before filter/order/cursor/limit/exact count.
2. `product_channel_convergence_postgres.rs`
   - two generic scheduler hosts;
   - lease exclusion/reclaim;
   - rejected Product isolation;
   - Product visibility race;
   - changed/unchanged Channel membership transitions.
3. `product_channel_identity_transitions_postgres.rs`
   - Channel create/delete;
   - tenant move;
   - same UUID delete+recreate before convergence with unchanged Product membership.
4. `product_linked_target_recreate_postgres.rs`
   - ProductVariant/SalesChannel same-UUID recreate monotonicity;
   - old target rows deliberately left physically materialized;
   - scalar Product authority separated from graph target authority;
   - graph query returns zero rows/exact count while a referenced current link target is stale or
     unavailable;
   - applying the current target mutation restores the graph payload.
5. `product_linked_target_availability_equivalence_postgres.rs`
   - two-root linked filter isolation;
   - many `MIN` aggregate ordering isolation;
   - exact-count parity;
   - fresh query-runtime recomposition while target is stale;
   - target-only mutation recovery for both ProductVariant and SalesChannel.
6. `product_linked_target_replay_redelivery_postgres.rs`
   - canonical ProductVariant source replay;
   - crash after PostgreSQL mutation durability/before checkpoint;
   - replay-worker restart and exact stable-event duplicate;
   - late never-delivered lower source version -> `StaleIgnored`;
   - query-runtime recomposition and graph authority preserved throughout.

None has been executed or admitted by the implementation agent.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh ledgers and durable relay step.
- [ ] Retain canonical event-contract digest admission for current `main`.
- [ ] Add the one canonical Product Index typed event family and concrete routes/consumers.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence for the typed incremental event route.

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
- [x] Define fail-closed semantics for link-present queried target materialization unavailable/stale.
- [x] Keep scalar-only Product queries independent of unused links.
- [x] Source-ready nested projection/exact-count target recreate packet.
- [x] Source-ready linked filter/many aggregate order/exact-count/runtime-recomposition equivalence
      packet for ProductVariant and SalesChannel target lag.
- [x] Source-ready replay-worker checkpoint-failure / duplicate / late-stale linked-target composition
      packet using canonical ProductVariant replay and real PostgreSQL mutation storage.
- [x] Source-ready Product delayed-mutation/locale-deletion PostgreSQL packet.
- [x] Source-ready Product visibility/Channel convergence multi-host PostgreSQL packet.
- [x] Source-ready Channel create/delete/tenant-move/delete-recreate PostgreSQL packet.
- [x] Remove parallel Product/ProductVariant compatibility implementations.
- [ ] Execute/admit the six retained Product M7 PostgreSQL packets.
- [ ] Execute any remaining schema-readiness/relation/freshness/projection concurrency evidence not
      already covered by those packets.
- [ ] Admit canonical Product typed wire events/routes/consumers after digest verification.
- [ ] Retain typed incremental-route crash-between-commit-and-ack/redelivery evidence.
- [ ] Retain remaining locale fan-out evidence.
- [ ] Prove complete Storefront Product/ProductVariant/SalesChannel query parity.
- [ ] Move Storefront traffic only after readiness/equivalence/freshness/availability evidence passes.

## Next implementation step

Primary owner step remains: **execute and admit the locked M6 repair PostgreSQL packet**.

The replay/redelivery ordering source gap for linked target recovery is now covered without adding a new
clock or Product-specific replay mechanism. The next unblocked M7/M5 source step is **canonical event-
contract digest admission for current `main`**. Recheck the actual event contract/evidence first; then
admit exactly one current Product Index typed event family and routes/consumers only if that digest gate
is source-complete. Do not introduce v2/v3 event branches or compatibility routes.

After typed route admission, retain its separate crash-between-commit-and-ack/redelivery and locale
fan-out evidence. Storefront cutover remains evidence-gated.

## Maintainer verification for current M7 source

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_convergence_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_identity_transitions_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_linked_target_recreate_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_linked_target_availability_equivalence_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_linked_target_replay_redelivery_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-convergence-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-identity-transitions-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-availability-equivalence-postgres-harness.mjs
node scripts/verify/verify-index-linked-target-replay-redelivery-postgres-harness.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
node scripts/verify/verify-index-link-target-availability.mjs
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
