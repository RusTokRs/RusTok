# Index drift source-page diagnosis

Status: `source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes one internal `IndexDriftSourcePageDiagnosisRuntime` after the immutable Index
source registry and guarded exact-entity diagnosis runtime are composed.

The runtime advances diagnosis from caller-known exact entities to one bounded owner-source page
without introducing a job, loop, scheduler, checkpoint store, or public discovery transport.

## Request boundary

`diagnose_source_page(context, schema, cursor, limit)` accepts:

- one request-bound `IndexReconciliationOperatorContext`;
- one typed `SchemaRef`;
- one optional server-held `IndexSourceCursor`;
- one page limit in `1..=32`.

Tenant and actor identities come only from the operator context. The runtime checks the current
request-local effective `modules:manage` snapshot before validating the page limit or constructing
`IndexSourceScanRequest`.

The frozen source registry then performs exactly one `IndexSource::scan` call. Its existing contract
revalidates tenant/schema scope, maximum page length, unique entity keys, non-empty continuation,
and cursor advancement.

## Candidate semantics

A source scan page contains current-state `Upsert` and retained `Delete` mutations.

This slice:

1. skips retained source `Delete` mutations;
2. treats each source `Upsert` as one source-present candidate;
3. delegates candidates sequentially to the existing guarded
   `IndexDriftDiagnosisOperatorRuntime::diagnose_entity`;
4. stops on the first source or exact-diagnosis failure;
5. returns only current-page counters, finding receipts, and the server-held next cursor.

The exact diagnosis operator repeats request-bound authorization for every candidate before source
load, materialized reads, digest calculation, or finding persistence.

The existing digest outcome intentionally does not expose whether a mismatch is materialized
`Missing`, stale fields, stale links, or another typed state difference. Therefore this capability
is accurately named source-page candidate diagnosis rather than missing-only diagnosis. A
missing-only selector over captured typed states remains a separate open slice.

## Output boundary

`IndexDriftSourcePageDiagnosisOutcome` contains only:

- scanned mutation count;
- source-present candidate count;
- skipped retained-delete count;
- consistent count;
- mismatch-recorded count;
- bounded finding receipts for mismatches;
- one optional server-owned continuation cursor.

It does not copy source entity IDs, source records, indexed records, fields, links, tenant IDs,
actor IDs, database errors, SQL, registry handles, or snapshot state into the outcome.

The cursor is not attached to GraphQL, HTTP, CLI, MCP, or native admin. No public identifier
discovery surface is added by this slice.

## Deliberate limits

This slice does not add or claim:

- a GraphQL or other public source-page transport;
- cursor persistence, multi-page accumulation, background iteration, scheduling, or restart state;
- missing-only mismatch classification;
- Index-only stale enumeration or orphan-link discovery;
- finding resolve/ignore lifecycle;
- targeted, full, dry-run, or shadow repair;
- retained authorization, PostgreSQL, GraphQL, workflow, or CI execution evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.
