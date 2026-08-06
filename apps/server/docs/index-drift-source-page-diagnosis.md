# Index drift source-page diagnosis

Status: `missing_only_source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes one internal `IndexDriftSourcePageDiagnosisRuntime` after the immutable Index
source registry and guarded exact-entity diagnosis runtime are composed.

The runtime advances diagnosis from caller-known exact entities to one bounded owner-source page
without introducing a job, loop, scheduler, checkpoint store, or public discovery transport.

## Request boundary

`diagnose_source_page(context, schema, cursor, limit)` currently accepts:

- one request-bound `IndexReconciliationOperatorContext`;
- one typed `SchemaRef`;
- one optional **server-held** `IndexSourceCursor`;
- one page limit in `1..=32`.

Tenant and actor identities come only from the operator context. The runtime checks the current
request-local effective `modules:manage` snapshot before validating the page limit or constructing
`IndexSourceScanRequest`.

The frozen source registry then performs exactly one `IndexSource::scan` call. Its existing contract
revalidates tenant/schema scope, maximum page length, unique entity keys, non-empty continuation,
and cursor advancement.

## Missing-only candidate semantics

A source scan page contains current-state `Upsert` and retained `Delete` mutations.

This slice:

1. skips retained source `Delete` mutations;
2. treats each source `Upsert` as one source-present candidate;
3. delegates candidates sequentially to
   `IndexDriftDiagnosisOperatorRuntime::diagnose_missing_entity_candidate`;
4. captures and validates one exact source/materialized pair per candidate;
5. records a finding only for source `Upsert` plus materialized `Missing`;
6. returns `NotCandidate` for materialized `Upsert` or `Delete`, including stale fields, stale links,
   and version differences;
7. stops on the first source or exact-diagnosis failure;
8. returns only current-page counters, missing finding receipts, and the server-held next cursor.

The diagnosis operator repeats request-bound authorization for every candidate before source load,
materialized reads, typed-state classification, digest calculation, or finding persistence.

The missing-only outcome intentionally exposes no captured state shape. A non-missing candidate is
reported only through the aggregate `non_missing_count`; raw source and materialized states are not
returned.

## Output boundary

`IndexDriftSourcePageDiagnosisOutcome` contains only:

- scanned mutation count;
- source-present candidate count;
- skipped retained-delete count;
- non-missing candidate count;
- missing-recorded count;
- bounded finding receipts only for missing entities;
- one optional server-owned continuation cursor.

It does not copy source entity IDs, source records, indexed records, fields, links, tenant IDs,
actor IDs, database errors, SQL, registry handles, or snapshot state into the outcome.

The raw cursor is not attached to GraphQL, HTTP, CLI, MCP, or native admin. The cursor is not
attached to GraphQL by the current internal method. No public identifier discovery surface is added
by this slice.

## Confidential continuation prerequisite

`rustok-index` now provides the transport-neutral `IndexSourceContinuationCodec` and
`IndexSourceContinuationScope` contracts. The codec encrypts the raw cursor with AES-256-GCM and
binds authenticated claims to:

- tenant;
- exact `SchemaRef`;
- canonical owner module and source name resolved from the frozen source registry;
- contract version;
- issued-at time and bounded expiry.

It rejects tampering, wrong scope, unsupported version, expiry, excessive future clock skew,
oversized tokens, and unavailable or retired key material before returning raw cursor state. This
is the source-complete server-owned continuation envelope contract; server key composition remains
open.

The codec is not yet composed into this server runtime. The current page method therefore remains an
internal-only raw-cursor boundary. A future sealed page method must authorize before token parsing,
open the token before constructing the source scan request, and seal the returned continuation before
leaving the service boundary.

## Deliberate limits

This slice does not add or claim:

- a GraphQL or other public source-page transport;
- caller-visible raw `IndexSourceCursor` JSON;
- server continuation-key configuration or secret resolution;
- a sealed page-runtime method;
- cursor persistence, multi-page accumulation, background iteration, scheduling, or restart state;
- Index-only stale enumeration or orphan-link discovery;
- finding resolve/ignore lifecycle;
- targeted, full, dry-run, or shadow repair;
- retained authorization, cryptographic integration, PostgreSQL, GraphQL, workflow, or CI execution
  evidence.

The next safe server slice is a bounded keyring composed from secret references plus one internal
sealed page method. Public transport remains later and must not expose raw cursor JSON.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-drift-digest-producer.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, cryptographic integration, PostgreSQL or GraphQL
scenarios, workflows, or CI were executed by the implementation agent.
