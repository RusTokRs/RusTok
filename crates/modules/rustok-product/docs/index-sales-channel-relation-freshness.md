# Product-SalesChannel relation freshness witness

Status: `source_convergence_and_materialized_fence_complete_runtime_evidence_pending`.

## Purpose

Resolved Product-to-SalesChannel membership is source-admissible only when it was observed from the
current Product visibility contract and the current tenant Channel identity set. Membership equality
alone is not enough evidence: an owner watermark can change while the final UUID set remains identical.

`product_sales_channel_index_relation_freshness_snapshots` is the Product-owned append-only witness for
that boundary. It is separate from `relation_epoch`;
a freshness-only change does not pretend that the graph membership changed.

## Witness

Each row retains exact tenant/Product identity, the verified `relation_epoch`, observed positive Product
`index_revision`, an opaque canonical Product visibility key, observed tenant Channel identity
generation, and a positive tenant-local append sequence.

The Product owner validates only opaque/numeric evidence and the referenced relation epoch. It does not
read Channel storage and has no dependency on `rustok-channel` or `rustok-index`. The ledger is
append-only, numeric owner watermarks cannot regress, and an exact duplicate tuple is idempotent.

## Channel identity watermark

`rustok-channel` owns `channel_index_identity_generations`, one durable generation per tenant. The
generation advances transactionally for Channel insert/delete/id movement/tenant movement/canonical
slug change. `is_active`, targets, OAuth configuration, resolution policies, and unrelated Channel
state do not advance it.

A tenant with no historical Channel identity row is generation `0`. After the first identity mutation,
the positive generation remains retained even if all Channels are later removed.

## Resolver admission

For one exact Product the distribution resolver:

1. observes Product metadata, Product `index_revision`, tenant Channel identity generation, and resolved
   UUID membership in `REPEATABLE READ`, `READ ONLY`;
2. writes membership through `ProductSalesChannelIndexRelationStore::replace`;
3. performs a fresh second observation;
4. requires the second UUID set to equal retained relation membership;
5. records that observation as the Product-owned freshness witness.

If owner state changes before a later source read, the canonical Product source sees the new current
facts and rejects the stale witness.

## Canonical replay gate

Live Product replay and Product locale absence require:

- current projection Product watermark equal to the current Product revision;
- a witness for the exact relation epoch referenced by projection state;
- current canonical visibility key equal to the witness key;
- current tenant Channel identity generation equal to the witness generation;
- witness Product source version not newer than the current Product revision.

A missing/stale witness therefore fails closed at source observation. Product hard-delete replay does
not require a live witness because the mutation removes the graph rather than publishing membership.

## Automatic convergence

Product visibility changes append durable exact convergence requests, and tenant Channel identity
changes are detected by comparing current Channel generation with Product-owned durable tenant
checkpoint state. The selected distribution invokes the same bounded resolver through the
generic ModuleWork scheduler, with tenant leases, restartable request cursors, and restartable 64-Product sweep
cursors.

This removes the previous requirement for an external caller to notice an owner change and manually
invoke reconciliation. The detailed owner/runtime contract is documented in
[index-sales-channel-relation-convergence.md](./index-sales-channel-relation-convergence.md).

## Materialized freshness boundary

The witness and convergence worker do not make source observation and Index mutation application one
cross-owner atomic transaction. Instead, the canonical Index query boundary now supplies the separate
materialized/query freshness fence.

A Product root row is query-admissible only when the materialized Product `projection_epoch` matches the
latest owner projection, the projection Product component matches current Product revision, the live
Product and exact locale still exist, the freshness witness matches current Channel generation, and no
visibility convergence request is newer than the witness Product revision.

This means a mutation produced before a later owner change may still be accepted physically by generic
Index mutation storage, but it cannot become query-authoritative while its owner evidence is stale. The
fence is intentionally outside Product ownership: Product still does not depend on `rustok-index` or
read Channel storage.

The first retained PostgreSQL materialized-freshness packet is source-ready. It delays a real Product
mutation across an owner revision change, confirms that the stale version is physically present in
`index_entities`, and requires Product query admission to exclude it before filter/order/cursor/limit
and exact count. The same packet covers locale deletion after source read. It has not been executed or
admitted by the implementation agent.

## Remaining admission

Still required before production cutover:

- execute/admit the delayed-mutation/locale-deletion PostgreSQL query-freshness packet;
- retained Product visibility + Channel-generation convergence evidence for unchanged/changed
  membership;
- retained PostgreSQL multi-host/concurrency/restart/delete-recreate/rejected-Product convergence
  evidence;
- canonical Product typed event admission/routes after event-contract digest admission;
- complete query equivalence and Storefront cutover evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
