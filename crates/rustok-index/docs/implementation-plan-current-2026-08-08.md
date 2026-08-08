# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked from `main@6044dbd5110a65e2ebeee0a6a3a0053d9971b250` (#3210) and continued on
`agent/index-localized-query-runtime-20260808`.

`implementation-plan.md` remains historical architecture context. The 2026-08-07 overlay remains useful
history for the linked-target/replay sequence, but this file is the current execution cursor.

## Recheck result

The current Product/Index sequence now includes:

- #3190 retained linked-target replay/redelivery source evidence;
- #3192 added the fail-closed Product Storefront parity gate;
- #3193 added staged single-current schema supersession;
- #3194 added schema-scoped source delivery IDs for replacement rebuilds;
- #3197 advanced the canonical Product owner clock for EAV writes;
- #3198 defined canonical typed Product `attribute_terms`;
- #3199 replaced Product runtime code with one current 15-field Product contract on internal routing key
  `4`, with lower keys historical only;
- #3200 corrected Storefront parity after proving owner title search is all-translations and owner result
  projection is requested-locale -> fallback-locale;
- #3204 selected the generic localized-entity fold architecture;
- #3208 added explicit localized query/validation and dedicated cursor identity;
- #3210 added root-only PostgreSQL localized identity-fold page/count compilation and decoding.

This source slice continues from #3210 and wires that compiler/decoder through the canonical query runtime
without changing Product schema, Product traffic, ordinary exact-locale query semantics, or evidence
status.

## Old execution branch

`agent/index-linked-target-replay-redelivery-evidence-20260807` is no longer a valid continuation base.
Its source was already squash-merged through #3190. Do not reuse it.

The old branch should be deleted when repository tooling permits it. Branch deletion is repository
hygiene only; it is not part of Index correctness.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

The concrete repair implementation, recovery policy, PostgreSQL harness, and retained-evidence admission
source remain complete. Maintainer execution/admission is still required and is not claimed by source
inspection.

## Current Product/Storefront source state

The canonical Product graph keeps one current Product runtime contract and the existing ProductVariant and
SalesChannel target contracts.

Current Product source/runtime facts:

- one current Product schema on internal routing key `4`;
- 15 fields: `id`, `status`, `title`, `handle`, `description`, `seller_id`, `vendor`, `product_type`,
  `primary_category_id`, `tag_ids`, `created_at`, `published_at`, `attribute_terms`, `variant_ids`, and
  `sales_channel_ids`;
- `variants` and `sales_channels` links unchanged;
- Product replay IDs are schema-scoped through `derive_index_schema_source_event_id`;
- lower Product routing keys are historical persisted identities, not compatibility implementations;
- EAV writes advance the same Product owner `index_revision` / graph `projection_epoch` clock;
- dynamic Product EAV filters use stable UUID-keyed typed `attribute_terms`;
- ProductVariant/SalesChannel recreate ordering still uses retained tombstone-backed `index_revision`;
- Product root freshness and ordinary linked-target availability remain fail-closed.

## Localized Product query source state

The Storefront owner contract still requires all-translations title admission plus requested -> fallback
projection. Ordinary exact-locale `IndexQuery` cannot provide that identity contract by itself.

The localized fold source is now complete through runtime execution:

- `LocalizedEntityQuery` keeps requested locale in `query.scope.locale`, canonical fallback separately,
  and `any_locale_filter` as an identity-level existential predicate;
- `localized_projection_fields` explicitly marks selected root scalar fields that project requested ->
  fallback -> null and therefore cannot leak arbitrary third-locale content;
- fold validation requires `LocaleMode::Required`, reuses ordinary field/operator/value rules, and keeps
  the initial implementation root-only;
- `LocalizedCursorCodec` has independent scoped wire version `3` and binds fallback/search/projection/order
  semantics; ordinary cursor version `2` remains unchanged;
- `compile_postgres_localized_page_query` emits page + exact count using `t0` anchor, `t1` requested,
  `t2` fallback, `t3` any-locale predicate and `t4` lower-locale anti-duplicate candidate;
- every physical row role retains canonical `is_deleted = FALSE` anchors for generic owner admission;
- de-duplication occurs before ordering/lookahead/limit/exact count;
- requested/fallback projection uses row-presence `CASE` rather than value-level `COALESCE`;
- `decode_postgres_localized_query_page` validates ordinary/localized plan identities and emits only
  localized continuation cursors;
- `IndexQueryPort::execute_localized_query` is now an explicit capability with a fail-closed default for
  adapters that do not implement it;
- `SharedIndexQueryRuntime` forwards the capability;
- `PostgresIndexQueryPort` compiles and applies availability/entity admission before beginning data
  execution, then verifies persisted schema readiness and executes page/count inside one
  `REPEATABLE READ, READ ONLY` transaction;
- localized results are decoded only through the localized decoder and transaction success/failure uses
  the same commit/rollback policy as ordinary queries.

Storefront traffic is still owner-native. Source-complete runtime is not execution/equivalence evidence.

## Remaining localized Storefront query gap

The identity/fallback problem is now solved at the generic query/runtime layer, but owner title search uses
`LIKE %search%` while the current generic Index scalar filter algebra has no bounded string text-pattern
operator.

The next source step is a generic scalar string text-pattern primitive that can be validated and compiled
inside `LocalizedEntityQuery::any_locale_filter`. It must remain generic Index behavior and must not add a
Product-specific SQL branch.

After that primitive exists, the Product Storefront adapter can map owner query inputs to the fold and
retained owner-vs-Index PostgreSQL evidence can be added.

## Retained M7 PostgreSQL packets

The six packets retained before the Product replacement remain valuable historical scenario definitions,
but several fixtures still hardcode historical Product routing key `3` and pre-replacement assumptions:

1. `product_materialized_query_freshness_postgres.rs`;
2. `product_channel_convergence_postgres.rs`;
3. `product_channel_identity_transitions_postgres.rs`;
4. `product_linked_target_recreate_postgres.rs`;
5. `product_linked_target_availability_equivalence_postgres.rs`;
6. `product_linked_target_replay_redelivery_postgres.rs`.

The Product locale-absence retained packet and related static guards also require recheck for historical key
assumptions.

Do not add a runtime alias for key `3`. Evidence must follow the one current Product contract.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh ledgers and durable relay step.
- [ ] Execute canonical event-contract digest admission on current reviewed `main` and commit the
      generated canonical digest artifact through its own reviewed PR.
- [ ] Add exactly one canonical Product Index typed event family and concrete routes/consumers only after
      the digest gate is valid.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence for the typed incremental route.

The digest workflow remains a maintainer execution gate. Source inspection must not fabricate the digest.

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
- [x] Query-path-scoped fail-closed linked-target availability semantics for ordinary Product queries.
- [x] One current 15-field Product Storefront-capable source contract.
- [x] Schema-safe single-current replacement/promotion mechanism.
- [x] Canonical typed EAV term representation and Product EAV owner clock.
- [x] Recheck owner all-translations search + requested/fallback projection mismatch.
- [x] Select one generic localized identity/fallback architecture without another Product routing key.
- [x] Add explicit localized query shape/validation and dedicated cursor identity.
- [x] Compile/decode the root-only localized identity fold page/count contract.
- [x] Wire localized execution through the canonical PostgreSQL query runtime with persisted readiness,
      generic admission and one read-only repeatable-read snapshot.
- [ ] Add generic scalar string text-pattern matching inside folded `any_locale_filter`.
- [ ] Implement the Product Storefront Index adapter and Taxonomy tag hydration boundary.
- [ ] Extend folded execution to linked paths only with dedicated target-availability evidence.
- [ ] Actualize retained Product PostgreSQL packets/guards to routing key `4` and the 15-field source.
- [ ] Retain source-ready folded-query owner-vs-Index PostgreSQL equivalence packets.
- [ ] Execute/admit current replacement Product PostgreSQL packets.
- [ ] Stage/rebuild/promote the current Product schema for a tenant.
- [ ] Move Storefront traffic only after readiness/equivalence/freshness/availability/restart evidence
      passes.

## Next implementation step

Primary maintainer gate remains: **execute and admit the locked M6 repair PostgreSQL packet**.

Next source-code step: **add one generic bounded scalar string text-pattern filter operator** to the Index
query contract/validation/PostgreSQL compiler and allow it inside folded `any_locale_filter`. Preserve
ordinary query compatibility and use explicit pattern semantics; do not add a Product-only SQL branch.

After text-pattern support, implement the Product Storefront Index adapter and a retained localized-query
PostgreSQL equivalence packet before any traffic switch.

In parallel, actualize retained Product PostgreSQL packets to routing key `4` / the 15-field current
contract. No historical-key compatibility path is allowed.

Typed Product events remain separately blocked on maintainer event-digest admission.

## Maintainer verification after this source slice

The implementation agent has not run these commands. Maintainer verification should include:

```bash
node scripts/verify/verify-index-localized-query-contract.mjs
node scripts/verify/verify-index-localized-query-postgres-fold.mjs
node scripts/verify/verify-index-localized-query-runtime.mjs
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
