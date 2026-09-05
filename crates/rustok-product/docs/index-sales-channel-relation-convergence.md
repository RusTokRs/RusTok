# Product-SalesChannel relation convergence

Status: `source_complete_runtime_evidence_pending`.

## Purpose

Product-to-SalesChannel relation membership and its freshness witness already fail closed when Product
visibility or tenant Channel identity changes. That source-admission fence is safe, but without a
durable trigger a caller still had to notice the owner change and invoke the resolver.

This contract adds Product-owned durable convergence state while preserving module ownership:

- Product owns visibility-change requests, lease/checkpoint state, and retries;
- Channel owns only its tenant-scoped opaque identity generation;
- `rustok-distribution` owns the cross-owner worker that compares those facts and invokes the existing
  bounded resolver;
- Product still does not read Channel tables and has no `rustok-channel` or `rustok-index` dependency.

## Visibility request ledger

`product_sales_channel_index_relation_convergence_requests` is append-only and tenant ordered. Product
INSERT or a canonical `channel_visibility` change appends:

- tenant identity;
- Product identity;
- the final positive Product `index_revision` observed after Product BEFORE triggers;
- a tenant-local sequence.

The trigger intentionally ignores unrelated Product metadata changes. The durable cursor advances one
retained sequence at a time, so direct SQL cannot skip an earlier request.

The worker reconciles the Product's **current** owner state. A queued older revision is therefore safe:
if a newer visibility request arrived meanwhile, the first reconciliation may already satisfy it, and
the newer retained request is still consumed in order as an idempotent follow-up.

A Product deleted before its request is processed is also safe: the worker treats owner absence as a
completed exact request because hard-delete replay removes the graph and does not require live relation
freshness.

A rejected Product owner contract also cannot permanently block the tenant request queue. Invalid
visibility, an oversized visibility allowlist, or too many resolved Channel targets complete that exact
request without creating a freshness witness. The Product therefore remains individually fail-closed
at source admission. Correcting Product visibility appends a new exact request; a Channel-side fix such
as reducing the resolved identity set advances Channel generation and schedules a new tenant sweep.

## Channel generation sweep

`product_sales_channel_index_relation_convergence_state` retains one tenant checkpoint.
`channel_identity_generation = NULL` means that the baseline tenant sweep has never completed. A
positive or zero completed value is the last Channel identity generation for which the tenant was
boundedly traversed through exact Product resolver calls.

When current Channel generation is newer, the worker starts a durable sweep with:

- `sweep_generation` fixed for that pass;
- `sweep_after_product_id` as the bounded keyset cursor;
- at most 64 Products per work item.

One rejected Product does not head-of-line block valid Products later in the tenant. The sweep skips the
rejected Product after leaving it source-stale, then continues exact reconciliation for the remaining
bounded page. Retryable concurrency/storage/relation/freshness errors still stop the page and preserve
the same durable cursor for retry.

If Channel identity changes again during the sweep, the running pass still finishes against its fixed
checkpoint generation. Individual Product resolutions observe current owner facts. After the old pass
commits, the current Channel generation remains ahead of the completed checkpoint and another full pass
is due. No Channel generation is silently skipped.

## Lease and retry contract

One tenant state row owns the lease. Claims use `FOR UPDATE`; competing hosts may discover the same
tenant, but only one receives the durable lease. The lease is bounded to five minutes.

Each claimed work item either:

- completes one exact visibility request;
- completes one bounded tenant sweep page;
- records a retry delay while preserving the request/sweep checkpoint; or
- loses the lease, after which the retained state remains authoritative.

Retryable resolver/storage races are delayed for five seconds. Rejected Product owner data is isolated
from tenant progress rather than forming a permanent head-of-line blocker. An expired lease can be
reclaimed without losing the retained cursor.

The Product DDL guard also enforces the state machine directly:

- state starts only from the canonical empty checkpoint;
- visibility cursor advances exactly one leased retained request;
- in-progress sweep generation cannot change;
- partial sweep cursor advances strictly while completing a leased page;
- terminal sweep completion clears its cursor while checkpointing that exact generation;
- completed Channel generation can advance only by completing that exact leased sweep;
- lease acquisition advances attempt count exactly once;
- state cannot be deleted.

## Runtime composition

`rustok-distribution::product_index::channel_relation_convergence` registers one
generic `ModuleWorkRegistration` only when both Product and Channel are selected. The host's existing
`ModuleWorkScheduler` supplies the polling lifecycle; the bridge itself owns no `tokio::spawn`, sleep
loop, broker cursor, or event family.

The work source performs one read-only, bounded due-tenant discovery. The Product-owned convergence
store then performs the real `FOR UPDATE` claim, so duplicate discovery across hosts is harmless.

## What this closes

This closes the previous **manual relation convergence** gap:

- Product visibility changes are durable exact requests;
- Channel identity changes are detected by comparing current generation with a durable tenant
  checkpoint;
- rejected Product owner data is isolated without making that Product source-admissible;
- crashes and concurrent hosts retain request/sweep progress;
- the existing freshness witness is automatically re-established by bounded resolver work for valid
  Products.

## What remains open

This does **not** make Index materialization atomic with owner changes. A Product source page can still
be read under a valid witness, then a Channel identity change can commit before that already-produced
Index mutation is applied. Automatic convergence will repair the relation, and the next source read
fails closed, but an already-applied materialized record needs an explicit query/materialized freshness
fence or retained evidence proving an equivalent admission boundary before authoritative Storefront
cutover.

Still required:

- retained PostgreSQL multi-host, lease-expiry, retry, rejected-Product isolation, visibility,
  Channel-generation, delete/recreate, and restart evidence;
- materialized/query freshness admission for the source-read -> mutation-apply window;
- canonical Product typed event admission after event-contract digest verification;
- complete Product/Variant/Channel query parity and Storefront cutover evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
