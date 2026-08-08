# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked from `main@678606a78916ed631669e40c617c60a031097138` (#3208) and continued on
`agent/index-localized-query-postgres-fold-20260808`.

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
- #3204 selected the one generic localized-entity fold architecture and kept Storefront cutover
  fail-closed pending implementation/evidence;
- #3208 added the explicit generic localized query shape, fail-closed validation, dedicated cursor wire
  identity, and kept ordinary exact-locale `IndexQuery` unchanged.

This source slice continues directly from #3208. It makes the root-only PostgreSQL localized fold compiler
and decoder source-complete without publishing a production execution method, changing Product traffic,
changing Product schema, or bypassing retained evidence gates.

## Old execution branch

`agent/index-linked-target-replay-redelivery-evidence-20260807` is no longer a valid continuation base.
It diverges from current `main` and contains source content already squash-merged through #3190. Do not
cherry-pick or continue development from that branch.

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
- Product root freshness and linked-target availability remain fail-closed.

## Corrected Storefront parity boundary

The 15-field replacement fixed the old missing-field/EAV gap, but #3200 proved a separate query identity
mismatch:

1. owner title search can match any Product translation;
2. owner result projection prefers requested translation, then fallback translation;
3. Product Index physically stores one entity per actual translation locale;
4. ordinary `IndexQuery` scopes one exact locale and therefore cannot by itself preserve owner search,
   fallback, global pagination, de-duplication, and exact count.

A scalar substring/LIKE operator on the current exact-locale query remains insufficient.

## Localized Product query architecture, contract, compiler and decoder

Architecture decision: complete in
`m7-product-storefront-localized-query-architecture.md`.

Selected architecture remains:

- preserve owner Storefront semantics;
- keep exactly one current Product schema/routing key;
- use a generic Index query-layer localized-entity fold whose logical page identity is
  `(tenant_id, schema_ref, entity_id)` while physical storage remains locale-keyed;
- evaluate any-locale title search as an identity predicate across current/admitted locale rows;
- choose result localization independently: requested locale -> fallback locale -> no localized row;
- apply exact count, sort, lookahead, and cursor semantics to Product identities before page truncation;
- forbid client-side merging of independently paginated locale queries;
- keep existing schema readiness and Product owner freshness admission mandatory.

The query/cursor contract from #3208 remains source-complete and this slice closes the compiler/decoder
source gap:

- `LocalizedEntityQuery` stays explicit and ordinary `IndexQuery` remains exact-locale;
- `localized_projection_fields` explicitly identifies selected root scalar fields that must be projected
  requested -> fallback -> null instead of leaking a third-locale identity anchor;
- localized projection fields cannot drive the ordinary identity filter or identity ordering;
- the first compiler deliberately accepts root-only query paths; linked folded paths remain a later
  capability/evidence step;
- `SchemaRegistry::compile_postgres_localized_page_query` emits one page statement and matching exact
  count using canonical physical aliases `t0` anchor, `t1` requested, `t2` fallback, `t3` any-locale
  predicate and `t4` lower-locale anti-duplicate candidate;
- all physical row roles retain the ordinary `is_deleted = FALSE` compiler anchor so generic
  `PostgresQueryEntityAdmission` can inject owner freshness into every participating row;
- one admitted identity anchor survives before ordering/lookahead/limit/count by excluding a lower-locale
  admitted `t4` row;
- requested/fallback projection uses row-presence `CASE`, so a present requested row with a nullable field
  never incorrectly falls through to fallback;
- exact count uses the same identity/admission boundary but does not depend on requested/fallback
  projection-row availability;
- `LocalizedQueryPlanFingerprint` separately binds canonical fallback, any-locale predicate and localized
  projection roles on top of the ordinary query plan fingerprint;
- `SchemaRegistry::decode_postgres_localized_query_page` checks both plan fingerprints, exact column/count
  shape and lookahead bounds;
- SQL null is accepted for a physically non-null field only when that field is explicitly listed as a
  localized projection field, meaning no requested/fallback row was admitted;
- lookahead emits only the dedicated `LocalizedCursorCodec` wire-version-3 continuation.

No `execute_localized_query` runtime method exists yet. The compiler/decoder output is therefore not an
authoritative production query path until the PostgreSQL port wires readiness, admission and one
repeatable-read execution snapshot around it.

## Retained M7 PostgreSQL packets

The six packets retained before the Product replacement remain valuable historical scenario definitions,
but they are **not yet current replacement evidence** because several fixtures still hardcode historical
Product routing key `3` and pre-replacement assumptions.

Packets requiring source actualization before execution/admission:

1. `product_materialized_query_freshness_postgres.rs`;
2. `product_channel_convergence_postgres.rs`;
3. `product_channel_identity_transitions_postgres.rs`;
4. `product_linked_target_recreate_postgres.rs`;
5. `product_linked_target_availability_equivalence_postgres.rs`;
6. `product_linked_target_replay_redelivery_postgres.rs`.

The separate Product locale-absence retained packet and related static guards must also be rechecked for
historical key assumptions.

Do not add a runtime alias for key `3` to make these tests pass. Evidence must follow the one current
Product contract.

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
- [x] Query-path-scoped fail-closed linked-target availability semantics for ordinary Product graph
      queries.
- [x] One current 15-field Product Storefront-capable source contract.
- [x] Schema-safe single-current replacement/promotion mechanism.
- [x] Canonical typed EAV term representation and Product EAV owner clock.
- [x] Recheck and document owner all-translations search + requested/fallback projection mismatch.
- [x] Choose one generic localized Product query identity/fallback architecture without another Product
      routing key.
- [x] Add explicit generic localized query shape/validation while keeping ordinary exact-locale
      `IndexQuery` unchanged.
- [x] Add dedicated localized cursor identity/version bound to requested/fallback/filter/projection/order
      semantics.
- [x] Compile the root-only localized-entity fold to one PostgreSQL page/exact-count contract.
- [x] Add the localized page decoder with explicit requested/fallback-null semantics and dedicated cursor.
- [ ] Wire localized execution through the PostgreSQL query port using persisted readiness, generic
      entity admission and one read-only repeatable-read snapshot.
- [ ] Add generic scalar text-pattern matching inside the folded any-locale identity predicate.
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

Next source-code step: **wire explicit localized execution into the PostgreSQL Index query port**. Reuse
the current persisted-schema readiness verifier, `PostgresQueryEntityAdmission`, page/exact-count statement
mapping and one read-only repeatable-read transaction. Admission/preparation failures must fail closed
before page/count storage execution, and results must be decoded only through
`decode_postgres_localized_query_page`.

Do not publish Product Storefront traffic in that runtime slice. After the runtime boundary exists, add the
generic scalar text-pattern primitive inside `any_locale_filter`, then implement the Product Storefront
adapter and retained equivalence packet.

In parallel, the retained Product PostgreSQL packets need a mechanical source actualization to routing key
`4`/15-field Product contract before they can be treated as current evidence. No historical-key runtime
compatibility path is allowed.

Typed Product events remain separately blocked on maintainer event-digest admission.

## Maintainer verification after this source slice

The implementation agent has not run these commands. Maintainer verification should include:

```bash
node scripts/verify/verify-index-localized-query-contract.mjs
node scripts/verify/verify-index-localized-query-postgres-fold.mjs
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
