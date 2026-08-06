# Index drift source-page diagnosis

Status: `sealed_internal_source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes one internal `IndexDriftSourcePageDiagnosisRuntime` after the immutable Index
source registry and guarded exact-entity diagnosis runtime are composed.

The runtime advances diagnosis from caller-known exact entities to one bounded owner-source page
without introducing a job, loop, scheduler, checkpoint store, or public discovery transport. It now
retains both the historical raw internal method and one sealed method suitable for a future bounded
transport.

## Request boundaries

`diagnose_source_page(context, schema, cursor, limit)` remains an internal-only compatibility method.
It accepts one optional server-held `IndexSourceCursor` and one page limit in `1..=32`.

`diagnose_source_page_sealed(context, schema, continuation, limit)` accepts:

- one request-bound `IndexReconciliationOperatorContext`;
- one typed `SchemaRef`;
- one optional opaque continuation string;
- one page limit in `1..=32`.

Tenant and actor identities come only from the operator context. Both paths require the current
request-local effective `modules:manage` snapshot. The sealed path authorizes and validates the page
limit before parsing the untrusted continuation token.

For the sealed path, the runtime then:

1. derives canonical tenant/schema/owner/source scope from the frozen
   `SharedIndexSourceRegistry`;
2. resolves the deployment-owned key references into one short-lived codec;
3. authenticates, decrypts, scope-checks, and expires the incoming token;
4. constructs `IndexSourceScanRequest` only after token admission;
5. performs exactly one validated `IndexSource::scan` call;
6. diagnoses the page once;
7. seals any returned raw cursor before returning the service result.

The raw cursor is never returned by `diagnose_source_page_sealed`.

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
8. returns only current-page counters and bounded missing-finding receipts.

The diagnosis operator repeats request-bound authorization for every candidate before source load,
materialized reads, typed-state classification, digest calculation, or finding persistence.

## Sealed output boundary

`IndexDriftSourcePageDiagnosisSealedOutcome` contains only:

- scanned mutation count;
- source-present candidate count;
- skipped retained-delete count;
- non-missing candidate count;
- missing-recorded count;
- bounded finding receipts only for missing entities;
- one optional `IndexSourceContinuationToken`.

It contains no raw `IndexSourceCursor`, source entity ID, source record, indexed record, field, link,
tenant ID, actor ID, database error, SQL, registry handle, snapshot state, key material, or secret
reference.

The sealed method and outcome are not attached to GraphQL, HTTP, CLI, MCP, or native admin by this
slice.

## Deployment-owned keyring

Configuration is read only from `RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON`. Example:

```json
{
  "active_key_id": "current",
  "lifetime_seconds": 300,
  "keys": {
    "current": {
      "resolver": "env",
      "key": "RUSTOK_INDEX_SOURCE_CONTINUATION_KEY_CURRENT"
    },
    "previous": {
      "resolver": "mounted_file",
      "key": "index/continuation/previous"
    }
  }
}
```

This slice supports deployment-owned `env` and `mounted_file` references. Mounted files require
`RUSTOK_INDEX_SOURCE_CONTINUATION_SECRET_MOUNT_ROOT`.

The configuration stores only bounded key IDs and `SecretRef` values. Secret values must be URL-safe
unpadded base64 and decode to exactly 32 bytes. At most 16 keys are admitted. One active key seals
new tokens; retained keys remain decrypt-only for rotation. Lifetime is bounded to 1 through 900
seconds.

Synchronous composition validates configuration shape, key IDs, reference uniqueness, resolver
policy, active-key presence, key count, and lifetime. Because `SecretResolverRegistry` is asynchronous,
actual secret resolution and exact-length validation happen inside the sealed request before token
parsing or source scan. Resolution, decoding, or length failure returns one bounded continuation
configuration error and exposes no resolver cause, reference key, or secret value.

Raw key bytes exist only inside a local `IndexSourceContinuationCodec` for one sealed call. The
keyring is passed privately into `IndexDriftSourcePageDiagnosisRuntime`; it is not inserted as a
separate `ModuleRuntimeExtensions` capability.

## Deliberate limits

This slice does not add or claim:

- a GraphQL or other public source-page transport;
- caller-visible raw `IndexSourceCursor` JSON;
- cloud/Vault/Kubernetes resolver configuration specific to this Index boundary;
- cursor persistence, multi-page accumulation, background iteration, scheduling, or restart state;
- Index-only stale enumeration or orphan-link discovery;
- finding resolve/ignore lifecycle;
- targeted, full, dry-run, or shadow repair;
- retained authorization, cryptographic integration, PostgreSQL, GraphQL, workflow, or CI execution
  evidence.

The next safe slice is one bounded transport over `diagnose_source_page_sealed`. It must preserve
authorization-before-input-parsing, accept no tenant or actor, expose no raw cursor or entity IDs,
delegate once, and return only bounded counters, receipts, and the opaque token.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, cryptographic integration, PostgreSQL or GraphQL
scenarios, workflows, or CI were executed by the implementation agent.