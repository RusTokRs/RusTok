# Product-SalesChannel relation freshness witness

Status: `source_complete_runtime_evidence_pending`.

## Purpose

Resolved Product-to-SalesChannel membership is correct only when it was observed from the current
Product visibility contract and the current tenant Channel identity set. Membership equality alone is
not enough evidence: an owner watermark can change while the final UUID set remains identical.

`product_sales_channel_index_relation_freshness_snapshots` is the Product-owned append-only witness for
that boundary. It is separate from `relation_epoch`; a freshness-only change does not pretend that the
graph membership changed.

## Witness

Each row retains:

- exact tenant and Product identity;
- the retained Product-to-SalesChannel `relation_epoch` that was verified;
- the observed positive Product `index_revision`;
- an opaque canonical Product visibility key;
- the observed tenant Channel identity generation;
- a positive tenant-local append sequence.

The Product owner validates only opaque/numeric evidence and the referenced relation epoch. It does not
read Channel storage and still has no dependency on `rustok-channel` or `rustok-index`.

The witness ledger is append-only. Numeric owner watermarks cannot regress. An exact duplicate tuple is
idempotent and does not append another row.

## Channel identity watermark

`rustok-channel` owns `channel_index_identity_generations`, one durable generation per tenant. The
generation advances transactionally for Channel identity changes that can alter Product relation
resolution:

- Channel insert;
- Channel delete;
- Channel id change;
- Channel tenant movement, bumping both affected tenants;
- canonical slug change.

`is_active`, Channel targets, OAuth configuration, resolution policies, and other unrelated Channel
state do not advance this watermark. A tenant that has never had a Channel is represented by generation
`0`; after its first Channel identity mutation the retained generation is positive and survives later
removal of all Channels.

## Resolver admission

For one exact Product the distribution resolver:

1. observes Product metadata, Product `index_revision`, tenant Channel identity generation, and resolved
   Channel UUID membership in one `REPEATABLE READ`, `READ ONLY` transaction;
2. writes membership through `ProductSalesChannelIndexRelationStore::replace`;
3. performs a fresh second observation;
4. requires that second resolved UUID set to equal the retained relation membership;
5. records the second observation as a Product-owned freshness witness for that relation epoch.

A concurrent owner change after the second observation does not create a false authoritative read: the
canonical Product source compares the retained witness with current owner facts on every live replay.

## Canonical replay gate

Live Product replay and Product locale absence require:

- current projection Product watermark to equal the current Product revision;
- a freshness witness for the exact relation epoch referenced by projection state;
- current canonical Product visibility key to equal the witness key;
- current tenant Channel identity generation to equal the witness generation;
- witness Product source version not to exceed the current Product revision.

A missing/stale witness therefore fails closed. Product hard-delete replay does not require a live
freshness witness because the mutation removes the graph rather than publishing membership.

## Remaining admission

This closes the source-level freshness watermark gap. It does not provide automatic owner-change
scheduling: after a visibility or Channel identity change, a caller still has to invoke exact Product
reconciliation or a bounded tenant sweep before the live Product source becomes readable again.

Still required before production cutover:

- retained PostgreSQL concurrency/restart/delete-recreate/freshness evidence;
- durable scheduling/triggering policy if automatic convergence latency is required;
- canonical Product typed event admission and routes after event-contract digest admission;
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
