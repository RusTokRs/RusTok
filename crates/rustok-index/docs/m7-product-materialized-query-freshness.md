# M7 Product materialized/query freshness fence

Status: `source_complete_linked_target_recreate_packet_execution_pending`.

## Query freshness boundary

Product source admission and automatic Product-to-SalesChannel convergence protect source reads, but an
already-produced Index mutation can still arrive after owner state changes. A post-result freshness
check is too late because stale root or linked rows may already have affected filtering, ordering,
cursor pagination, many projections, aggregate ordering, and exact count.

`rustok-index` therefore owns `PostgresQueryEntityAdmission`, a trusted schema-scoped PostgreSQL
entity-admission contract applied to every compiler-owned materialized `index_entities` relation.

A rule:

- must reference the compiler-controlled `{{entity}}` alias;
- cannot contain bind placeholders, SQL statement boundaries, or comments;
- is bounded to 32 KiB;
- changes no user bind, selected column, plan fingerprint, or cursor contract;
- is applied to page and exact-count SQL;
- fails closed for unknown compiler entity alias families or missing canonical `is_deleted = FALSE`
  anchors.

Current compiler alias families covered are `tN`, `mpN_tN`, `mx_tN`, and `mo_tN`, covering root and
ordinary joins, many projections, many-filter `EXISTS`, and many aggregate ordering.

`PostgresIndexQueryAdmissionCatalog` keeps at most one owner rule per exact `SchemaRef`. Runtime-local
pass-through descriptors let an otherwise ungoverned root still apply governed rules to linked targets.

## Product graph owner rules

The selected Product distribution registers entity admission for Product and ProductVariant. When
Channel is selected it also registers SalesChannel admission. No new Index schema or freshness clock is
introduced.

### Product

A Product materialized row requires live Product + exact locale, materialized `source_version` equal to
current Product `projection_epoch`, current Product owner revision reflected in that projection, a
matching relation freshness witness, current Channel generation, and no newer Product visibility
convergence request.

### ProductVariant

A ProductVariant materialized row requires a live owner row for the same tenant/UUID and:

`product_variants.index_revision = index_entities.source_version`.

### SalesChannel

A SalesChannel materialized row requires a live owner row for the same tenant/UUID and:

`channels.index_revision = index_entities.source_version`.

These linked-target rules prevent stale/deleted target payloads from participating in ordinary joins,
nested many projections, many filters, aggregate ordering, and other materialized target paths.

## Recreate monotonicity is already source complete

A post-#3179 source recheck corrected an earlier assumption: ProductVariant and SalesChannel do **not**
need a new recreate clock. Their existing retained tombstone migrations already preserve the same
`index_revision` clock monotonically through hard delete and recreation of the same tenant/UUID.

### ProductVariant retained identity

`m20260731_000004_add_product_index_tombstones`:

- stores ProductVariant delete evidence at `OLD.index_revision + 1`;
- before insert of the same tenant/Variant UUID, seeds `NEW.index_revision` to at least retained
  `source_version + 1`;
- rejects exhausted revisions;
- clears the retained tombstone only after the live revision strictly supersedes it;
- retains equivalent move/identity safety.

Therefore an old materialized ProductVariant source version cannot equal the recreated incarnation's
current source version.

### SalesChannel retained identity

`m20260731_000011_add_channel_index_tombstones` provides the same protocol for SalesChannel:

- delete tombstone at `OLD.index_revision + 1`;
- recreate seed above the retained tombstone;
- fail closed at exhaustion;
- clear only after strict live supersession;
- retain identity-move safety.

Channel identity generation remains separate: it drives Product relation freshness/convergence and is
not a replacement for SalesChannel `index_revision`.

No new ProductVariant/SalesChannel ledger or schema version should be added for recreate ordering.

## Retained PostgreSQL packets

Four M7 packets are source-ready and execution/admission pending:

1. `product_materialized_query_freshness_postgres.rs` — delayed Product scalar mutation and locale
   deletion;
2. `product_channel_convergence_postgres.rs` — Product visibility / Channel identity convergence,
   lease reclaim, rejected Product isolation, changed and unchanged membership;
3. `product_channel_identity_transitions_postgres.rs` — Channel create/delete/tenant-move and
   delete+recreate Product relation/freshness behavior;
4. `product_linked_target_recreate_postgres.rs` — ProductVariant/SalesChannel retained recreate
   monotonicity plus stale linked-target payload exclusion and current-target recovery.

Detailed contract for packet 4:
`m7-linked-target-recreate-postgres-harness.md`.

None of these packets has been executed or admitted by the implementation agent.

## Remaining linked-target availability boundary

The entity-admission fence proves that a stale linked target payload cannot be authoritative. It does
**not** yet define the final semantics when a Product link exists but the target has not yet been
materialized or is temporarily filtered as stale.

Current left-join/many-subquery SQL can represent that situation as null/empty nested data. Complete
Product graph parity still needs an explicit fail-closed policy and retained PostgreSQL equivalence
proof so target unavailability cannot be mistaken for authoritative owner null/absence semantics.

This link-present/target-missing policy is now the next unblocked M7 source-design gap. Recreate source
ordering itself is no longer a gap.

## Remaining M7 admission

Still required before Storefront cutover:

- define and retain fail-closed linked-target availability semantics for link-present / target-missing
  windows;
- execute/admit the linked-target recreate packet and the other retained Product packets;
- complete Product/ProductVariant/SalesChannel query equivalence and linked-target availability proof;
- admit canonical Product typed events only after event-contract digest verification;
- pass schema readiness, equivalence, convergence, freshness, restart, and retained PostgreSQL evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_recreate_postgres -- --nocapture
node scripts/verify/verify-index-linked-target-recreate-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-linked-target-query-freshness.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
