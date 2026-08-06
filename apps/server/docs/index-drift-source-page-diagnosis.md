# Index drift source-page diagnosis

Status: `graphql_sealed_transport_source_complete_owner_execution_pending`.

## Purpose

`IndexDriftSourcePageDiagnosisRuntime` advances exact drift diagnosis to one bounded owner-source page
without adding a loop, scheduler, checkpoint store, lifecycle command, or repair capability.

The runtime retains:

- the historical internal raw method `diagnose_source_page`;
- the transport-safe `diagnose_source_page_sealed` method;
- one optional private server-owned continuation keyring.

Only the sealed method is mounted through GraphQL. The raw method remains inside the server service
boundary.

## Request boundaries

`diagnose_source_page(context, schema, cursor, limit)` remains internal-only and accepts a
server-held `IndexSourceCursor`.

`diagnose_source_page_sealed(context, schema, continuation, limit)` accepts one typed schema, one
optional opaque continuation string, and one page limit in `1..=32`.

Both methods require a request-bound non-nil tenant/actor context and effective `modules:manage`.
The sealed method authorizes and validates the limit before token work. It then:

1. derives canonical tenant/schema/owner/source scope from the frozen source registry;
2. resolves bounded deployment-owned key references into one short-lived codec;
3. authenticates, decrypts, scope-checks, and expires the incoming token;
4. constructs `IndexSourceScanRequest` only after token admission;
5. performs exactly one `IndexSource::scan` call;
6. diagnoses that page once;
7. seals any outgoing raw cursor before returning.

The raw cursor is never returned by the sealed method.

## Missing-only semantics

For every source page, the runtime:

- skips retained source `Delete` mutations;
- treats every source `Upsert` as one candidate;
- delegates candidates sequentially to `diagnose_missing_entity_candidate`;
- records a finding only for source `Upsert` plus materialized `Missing`;
- reports every materialized `Upsert` or `Delete` as `NotCandidate`;
- stops on the first source or exact-diagnosis failure.

The page size is limited to 32, so current-page counters and finding receipts remain bounded.

## Sealed output boundary

`IndexDriftSourcePageDiagnosisSealedOutcome` contains only:

- scanned mutation count;
- candidate count;
- skipped-delete count;
- non-missing count;
- missing-recorded count;
- bounded missing-finding receipts;
- one optional `IndexSourceContinuationToken`.

It contains no raw `IndexSourceCursor`, source entity ID, source/index record, field, link, source
identity, tenant, actor, SQL, database cause, secret reference, or key material.

## GraphQL transport

The root mutation now exposes:

- `diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)`.

Tenant and actor come only from authenticated request context. The resolver checks effective
`modules:manage` before parsing module/entity/version, limit, or continuation. It accepts no source
name, owner module, entity ID, entity list, or raw cursor JSON and delegates exactly once to
`diagnose_source_page_sealed`.

The GraphQL payload contains only current-page counts, bounded finding receipts, completion state,
and the opaque continuation token.

## Deployment-owned keyring

`RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` stores only bounded key IDs, lifetime, and
`SecretRef` values. This slice supports deployment-owned `env` and `mounted_file` aliases.

The configuration is bounded to 16 KiB before parsing. At most 16 unique references are admitted;
key IDs are bounded to 64 bytes and reference keys to 256 bytes. Secret values must be canonical
43-byte URL-safe unpadded base64 and decode to exactly 32 bytes.

Actual values are resolved asynchronously inside the sealed request before token parsing or source
scan. The local codec and decoded key map are dropped after the incoming token is opened and the
outgoing cursor is sealed. Resolver causes, reference keys, and key material are not exposed.

## Deliberate limits

This slice does not add:

- multi-page iteration or cross-page accumulation;
- persisted continuation or restart state;
- background scanning or scheduling;
- stale Index-only or orphan-link discovery;
- finding resolve/ignore lifecycle;
- targeted/full/shadow repair;
- retained authorization, secret-resolution, rotation, expiry, cryptographic, PostgreSQL, GraphQL,
  workflow, or CI evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-drift-source-page-graphql-transport.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, cryptographic integration, PostgreSQL or GraphQL
scenarios, workflows, or CI were executed by the implementation agent.
