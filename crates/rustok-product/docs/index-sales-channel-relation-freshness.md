# Product-SalesChannel relation freshness witness

Status: `source_and_automatic_convergence_complete_materialized_fence_and_runtime_evidence_pending`.

## Purpose

Resolved Product-to-SalesChannel membership is source-admissible only when it was observed from the
current Product visibility contract and the current tenant Channel identity set. Membership equality
alone is not enough evidence: an owner watermark can change while the final UUID set remains identical.

`product_sales_channel_index_relation_freshness_snapshots` is the Product-owned append-only witness for
that boundary. It is separate from `relation_epoch`; a freshness-only change does not pretend that the
graph membership changed.

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

Product visibility changes now append durable exact convergence requests, and tenant Channel identity
changes are detected by comparing the current Channel generation with Product-owned durable tenant
checkpoint state. The selected distribution invokes the same bounded resolver through the generic
ModuleWork scheduler, with tenant leases, restartable request cursors, and restartable 64-Product sweep
cursors.

This removes the previous requirement for an external caller to notice an owner change and manually
invoke reconciliation. The detailed owner/runtime contract is documented in
[index-sales-channel-relation-convergence.md](./index-sales-channel-relation-convergence.md).

## Materialized freshness boundary

The witness plus automatic resolver convergence still do **not** make source observation and Index
mutation application one cross-owner atomic transaction. A Channel identity change may commit after a
Product source page was read but before that already-produced mutation is applied. The next source
observation will fail closed and automatic convergence will repair owner relation state, but the
watermark alone is not a production materialized-view freshness guarantee for an already-produced or
already-applied mutation.

Therefore authoritative cutover still requires retained evidence for this in-flight window plus an
explicit materialized/query freshness fence (or an equivalent admission boundary). This distinction is
deliberate: the witness closes stale source admission, and the convergence worker closes manual owner
repair scheduling; neither by itself fences a previously produced Index mutation.

## Remaining admission

Still required before production cutover:

- materialized/query freshness fencing for the source-read -> mutation-apply window;
- retained PostgreSQL multi-host/concurrency/restart/delete-recreate/in-flight freshness and
  convergence evidence;
- canonical Product typed event admission/routes after event-contract digest admission;
- complete query equivalence and Storefront cutover evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
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
