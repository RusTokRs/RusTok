# Product-SalesChannel relation freshness witness

Status: `source_complete_materialized_convergence_and_runtime_evidence_pending`.

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

## Materialized freshness boundary

The witness does **not** make source observation and Index mutation application one cross-owner atomic
transaction. A Channel identity change may commit after a Product source page was read but before that
already-produced mutation is applied. The next source observation will fail closed, and a membership
change can later advance relation/projection state, but the watermark alone is not a production
materialized-view freshness guarantee.

Therefore authoritative cutover still requires automatic/bounded convergence plus retained evidence
covering this in-flight window (or an equivalent materialized/query freshness fence). This distinction
is deliberate: the witness closes stale source admission, not the remaining materialized convergence
problem.

## Remaining admission

Still required before production cutover:

- automatic or otherwise bounded owner-change convergence/materialized freshness fencing;
- retained PostgreSQL concurrency/restart/delete-recreate/in-flight freshness evidence;
- canonical Product typed event admission/routes after event-contract digest admission;
- complete query equivalence and Storefront cutover evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
