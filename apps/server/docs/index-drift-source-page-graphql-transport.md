# Index drift source-page GraphQL transport

Status: `source_complete_owner_execution_pending`.

## Operation

The root mutation now exposes one bounded source-page operation `diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)`:

```graphql
mutation DiagnoseIndexSourcePage($input: IndexDriftSourcePageDiagnosisInput!) {
  diagnoseIndexSourcePage(input: $input) {
    scannedMutationCount
    candidateCount
    skippedDeleteCount
    nonMissingCount
    missingRecordedCount
    findings {
      findingId
      findingKey
      status
    }
    complete
    continuation
  }
}
```

The operation delegates exactly once to `diagnose_source_page_sealed`. It never calls the historical
raw-cursor page method.

## Input boundary

`IndexDriftSourcePageDiagnosisInput` accepts only:

- `moduleName` as a bounded string;
- `entityName` as a bounded string;
- `schemaVersion` as a bounded positive-integer string;
- `limit` as a bounded string that must parse into `1..=32`;
- one optional opaque `continuation` string bounded to 16 KiB.

Tenant and actor are derived only from authenticated `TenantContext` and `AuthContext`. The input has
no tenant, actor, source name, owner module, raw cursor JSON, entity ID, entity-ID list, batch,
checkpoint, scheduler, lifecycle, or repair field.

The resolver creates the request-bound operator context and checks the current effective
`modules:manage` snapshot before calling `parse_schema`, parsing the limit, or validating the
continuation length. In other words, authorization runs before schema, limit, or continuation parsing.

After bounded parsing, the resolver looks up `IndexDriftSourcePageDiagnosisRuntime` from the frozen
runtime extensions and delegates exactly once to `diagnose_source_page_sealed`.

## Sealed continuation

The resolver treats the continuation only as an opaque string. The service boundary:

1. repeats request-bound authorization;
2. derives canonical tenant/schema/owner/source scope from the frozen source registry;
3. resolves the private deployment keyring;
4. authenticates, decrypts, scope-checks, and expires the token;
5. constructs `IndexSourceScanRequest` only after token admission;
6. diagnoses one page;
7. seals any outgoing raw cursor before returning.

No raw `IndexSourceCursor`, source identity, keyring handle, `SecretRef`, key material, or decoded
cursor state crosses the GraphQL resolver.

## Output boundary

`IndexDriftSourcePageDiagnosisPayload` exposes only:

- five current-page aggregate counters;
- at most 32 bounded finding receipts;
- completion state;
- one optional opaque continuation token.

Finding receipts expose the persisted finding UUID, digest-shaped finding key, and bounded lifecycle
status. They do not expose the source entity UUID, source/index record, fields, links, source name,
owner module, tenant, actor, SQL, database cause, secret reference, or key material.

## Error boundary

The transport exposes fixed GraphQL codes for:

- invalid or expired continuation tokens;
- unavailable continuation configuration or key resolution;
- owner-source dependency failure;
- snapshot-capture and mismatch-recording dependency failure.

Retryability and already-bounded dependency machine codes are retained where available. Resolver
causes, source names, secret references, token contents, raw cursor JSON, SQL, and database causes are
not exposed.

## Deliberate limits

This transport performs one page only. It adds no loop, multi-page accumulation, persisted cursor,
job, scheduler, background task, stale Index-only enumeration, orphan-link discovery, finding
resolve/ignore command, or repair capability.

## Suggested maintainer validation

```bash
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis::tests -- --nocapture
node scripts/verify/verify-index-drift-source-page-graphql-transport.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, GraphQL scenarios, secret-resolution scenarios,
workflows, or CI were executed by the implementation agent.
