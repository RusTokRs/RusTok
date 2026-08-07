# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@0746ffa4aa4eea4383732bd4f4679c3d800cbf1d` and continued on
`agent/index-product-channel-convergence-postgres-evidence-20260807`.

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
- Product-owned visibility convergence requests and tenant lease/checkpoint state;
- bounded generic ModuleWork Product-SalesChannel automatic convergence;
- generic schema-scoped PostgreSQL root query admission;
- canonical Product materialized/query freshness fence for the source-read -> mutation-apply window;
- persisted tenant schema readiness gate.

Two retained M7 PostgreSQL packets are now source-ready and execution/admission pending:

1. delayed Product scalar mutation + locale deletion materialized/query freshness;
2. Product visibility + Channel identity convergence with multi-host lease/restart and rejected-Product
   isolation.

Neither packet has been executed by the implementation agent.

## Canonical Product policy

Product Index has no parallel Product compatibility implementations. The selected distribution
registers one current Product schema through `product-postgres-primary`, one current ProductVariant
schema through `product-variant-postgres-primary`, and the current SalesChannel schema through
`sales-channel-postgres-primary`.

The generic numeric `SchemaVersion` inside `SchemaRef` remains an Index storage/routing primitive only;
it is not a Product compatibility matrix. The current Product graph contains Product scalars,
`variant_ids`/`variants`, and `sales_channel_ids`/`sales_channels`. Product visibility slugs stay
owner-side resolver input rather than transitional Index fields.

## Membership, ordering, freshness, convergence, and query admission are separate

1. `relation_epoch` changes only when resolved Product-to-SalesChannel UUID membership changes.
2. `projection_epoch` advances when complete Product record inputs move and is the only Product Index
   mutation `source_version`.
3. `product_sales_channel_index_relation_freshness_snapshots` records that one retained relation epoch
   was verified against Product visibility and a Channel identity generation.
4. `product_sales_channel_index_relation_convergence_requests` plus tenant convergence state ensure
   owner changes are durably driven back through the bounded resolver.
5. Product root query admission compares materialized `projection_epoch` and existing owner freshness
   evidence at query time so an already-produced stale mutation cannot become authoritative.

A freshness-only change never fabricates a relation membership change. A convergence checkpoint is not
an Index mutation clock. Query admission adds no Product schema and no duplicate relation watermark.

## Channel identity generation

`rustok-channel` owns `channel_index_identity_generations`, one durable generation per tenant.
Transactionally observed identity changes advance it for Channel insert/delete/id/tenant/canonical-slug
changes. `is_active` and unrelated Channel configuration do not invalidate Product relation identity.

Generation `0` represents a tenant with no historical Channel identity row. After the first identity
mutation, the positive generation is retained even if the tenant later has zero Channels.

## Freshness watermark source complete

For an exact Product, the distribution resolver:

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
projection relation epoch matches current visibility and current tenant Channel identity generation. A
witness Product watermark may be older than current Product revision only when visibility remains
unchanged; unrelated Product updates therefore do not falsely stale the relation. Product hard-delete
replay removes the graph and does not require a live freshness witness.

## Automatic owner-change relation convergence source complete

Product owns two additional durable surfaces:

- append-only exact Product requests for INSERT/canonical `channel_visibility` changes;
- one tenant convergence state with exact request cursor, completed opaque Channel generation,
  in-progress Channel sweep generation/Product cursor, bounded lease, retry availability, attempt
  count, and error marker.

The selected distribution registers `product_sales_channel_relation_convergence` through the existing
generic `ModuleWorkScheduler` only when Product and Channel are both selected. One work item processes
either one exact Product request or one bounded 64-Product convergence page, calling the existing exact
resolver for each Product. Product-owned `FOR UPDATE` claim state makes duplicate due discovery across
hosts harmless. Lease expiry preserves request/sweep progress across restart.

Channel identity changes during a sweep cannot be skipped: the pass checkpoints only the generation it
started with, so a newer current Channel generation remains due for another pass. Product visibility
requests remain tenant ordered and exact; deleted Products advance their obsolete request cursor without
requiring a live witness.

Rejected Product owner data is isolated from tenant progress. Invalid visibility, oversized allowlists,
or too many resolved Channel targets leave that Product individually source-stale without blocking
later Products in the same tenant. Correcting Product visibility creates a new exact request; a
Channel-side correction advances Channel generation and schedules another tenant pass. Retryable
concurrency/storage/relation/freshness failures preserve the current page for retry.

The Product DDL state machine prevents direct SQL from skipping visibility requests, changing an
in-progress sweep generation, resetting a partial Product cursor, forging a completed Channel
checkpoint, or deleting convergence state. Product still does not read Channel storage or depend on
`rustok-channel`/`rustok-index`.

## Materialized/query freshness fence source complete

The previous source-read -> mutation-apply window is now fenced at the root query boundary.

`rustok-index` owns a module-neutral `PostgresQueryRootAdmission` and exact-schema admission catalog.
The canonical query runtime snapshots those rules, rejects rules for schemas absent from the immutable
schema registry, and applies an admission predicate to the compiler-owned root `index_entities`
baseline in the same `REPEATABLE READ`, `READ ONLY` transaction as query execution.

Admission is applied before user filter, cursor, order, pagination/limit, and exact count. The same
predicate is inserted into exact-count SQL. It changes no bind values, selected columns, plan
fingerprint, or cursor contract and fails closed if the compiler baseline no longer matches the exact
expected anchor.

The Product rule requires:

- live Product and exact locale translation;
- materialized `index_entities.source_version` equal to latest Product `projection_epoch`;
- projection Product component equal current `products.index_revision`;
- freshness witness for the projection relation epoch;
- witness Channel generation equal current tenant Channel generation;
- no visibility convergence request newer than the witness Product revision.

This closes the materialized/query freshness fence at source level without a new Product schema,
duplicate relation membership, a query-time visibility parser, or a new Index watermark.

## Product materialized freshness PostgreSQL packet 1

`crates/rustok-distribution/tests/product_materialized_query_freshness_postgres.rs` is source-ready and
execution-pending. It uses real Product/Index migrations, real Product source loading,
`PostgresMutationStore`, persisted schema registration, and the canonical shared Index query runtime.

The packet deliberately materializes a stale Product mutation after owner state has advanced, proves
the stale source version is physically present in `index_entities`, and requires it to be excluded
before title filter/order/cursor lookahead/limit/exact-count. It then applies the current mutation and
proves normal two-page cursor behavior. A second scenario applies a delayed locale upsert after the
owner translation is deleted and requires the physically stored row to remain query-inadmissible with
exact count zero.

This is retained source, not admitted evidence. No PostgreSQL execution result is claimed.

## Product visibility / Channel convergence PostgreSQL packet 2

`crates/rustok-distribution/tests/product_channel_convergence_postgres.rs` is source-ready and
execution-pending. It uses real Channel/Product/Index migrations, two independent generic
`ModuleWorkScheduler` hosts, Product-owned convergence state, the real Product replay source, generic
Index mutation storage, persisted schema readiness, and the canonical Product query fence.

The retained scenarios cover:

- a one-second Product-owned lease claimed by host A, active-lease exclusion on host B, expiry, and
  reclaim/completion by host B without resetting visibility progress;
- malformed Product visibility remaining without relation/freshness while a later valid Product still
  converges and the tenant Channel-generation sweep completes;
- Product visibility `alpha -> beta` after source read, physical old Index materialization, query
  exclusion, relation/projection advancement, and corrective current Product mutation;
- Channel slug identity change with unchanged unrestricted UUID membership, where query admission fails
  until freshness reaches the new generation but relation/projection remain unchanged and the same Index
  row becomes admissible again;
- a later Channel slug identity change that removes restricted membership, advances relation/projection,
  leaves the old restricted Index row inadmissible, and requires the current Product mutation before
  query authority returns.

Detailed contract:
`m7-product-channel-convergence-postgres-harness.md`.

This packet is retained source only. No PostgreSQL execution result is claimed.

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
- [x] Product visibility convergence request ledger and tenant lease/checkpoint state.
- [x] Automatic bounded Product visibility / Channel identity relation convergence through generic
      ModuleWork composition, including rejected-Product poison isolation.
- [x] Implement/admit a materialized/query freshness fence for the source-read -> mutation-apply
      in-flight window at source level.
- [x] Add source-ready PostgreSQL packet for delayed Product scalar mutation and locale deletion query
      admission across filter/order/cursor/limit/exact-count.
- [x] Add source-ready PostgreSQL Product visibility + Channel-generation convergence packet with
      multi-host lease expiry/restart, unchanged/changed membership, and rejected-Product evidence.
- [x] Remove parallel Product/ProductVariant compatibility implementations.
- [ ] Execute/admit the Product materialized freshness PostgreSQL packet.
- [ ] Execute/admit the Product visibility + Channel-generation convergence PostgreSQL packet.
- [ ] Retain any remaining Channel create/delete/tenant-move/delete-recreate identity evidence.
- [ ] Execute PostgreSQL evidence for schema readiness, relation/freshness storage, projection
      concurrency/delete ordering, and canonical replay not already covered by the two Product packets.
- [ ] Admit canonical Product typed wire events/routes/consumers after digest verification.
- [ ] Retain remaining out-of-order, locale fan-out, and linked-target availability evidence.
- [ ] Prove complete Product/Variant/Channel query parity, including linked-target availability.
- [ ] Move Storefront traffic only after readiness/equivalence/materialized-freshness evidence passes.

## Next implementation step

Primary owner step remains: execute and admit the locked M6 repair PostgreSQL packet.

The two largest Product freshness/convergence M7 runtime packets are now source-ready. The next
unblocked M7 source step is to retain the remaining Channel identity transition evidence that is not
covered by slug changes: Channel create/delete, tenant move, and delete-recreate behavior, including
Product query admission and relation/projection consequences. After that, continue complete
Product/Variant/Channel linked-target query parity. Do not add another Product schema or freshness
clock. Keep typed Product event work separately blocked until digest admission.

## Maintainer verification for this slice

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_convergence_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-convergence-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
