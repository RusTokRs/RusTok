# Index drift diagnosis GraphQL transport

Status: `source_complete_owner_execution_pending`.

## Purpose

The server GraphQL schema exposes one bounded mutation over the existing guarded
`IndexDriftDiagnosisOperatorRuntime`:

```graphql
mutation DiagnoseIndexEntity($input: IndexDriftDiagnosisInput!) {
  diagnoseIndexEntity(input: $input) {
    status
    digest
    sourceDigest
    materializedDigest
    findingId
    findingKey
    findingStatus
  }
}
```

The transport diagnoses exactly one entity. It does not expose replay, reconciliation, scanning,
entity discovery, finding lifecycle, or repair authority.

## Input boundary

`IndexDriftDiagnosisInput` contains only:

- `moduleName: String!`;
- `entityName: String!`;
- `schemaVersion: String!`;
- `entityId: String!`;
- `locale: String`.

Tenant and actor identities are never accepted from GraphQL input. They are derived from the
request's authenticated `AuthContext` and `TenantContext`.

All untrusted identity fields intentionally remain strings at the GraphQL boundary. The resolver
therefore checks the current task-local RBAC snapshot and effective `modules:manage` permission
before parsing module/entity identifiers, schema version, UUID, or locale. Invalid or oversized
values cannot be used as an authorization oracle.

After authorization, the transport:

1. bounds every string by a fixed byte limit;
2. builds one canonical `ModuleName`, `EntityName`, positive `SchemaVersion`, non-nil entity UUID,
   and optional canonical `LocaleKey`;
3. derives the key tenant from `TenantContext`;
4. retrieves the already-composed `IndexDriftDiagnosisOperatorRuntime`;
5. delegates once to `diagnose_entity(context, key)`.

The operator repeats the same request-bound authorization before source access, materialized reads,
digest production, or finding persistence. The transport adds no database permission lookup and no
second long-lived authority cache.

## Output boundary

The mutation returns only one typed status and bounded digest/receipt metadata:

- `CONSISTENT` with one SHA-256 digest; or
- `MISMATCH_RECORDED` with source/materialized SHA-256 digests, finding UUID, finding-key digest,
  and receipt status (`CREATED`, `REFRESHED`, `REOPENED`, or `SUPPRESSED`).

It returns no owner payload, indexed payload, fields, links, tenant identity, actor identity, SQL,
database cause, transaction token, registry, snapshot boundary, source watermark, scheduler handle,
or repair capability.

Dependency failures expose only a fixed GraphQL error code, a boolean retryability flag, and the
already-bounded Index dependency code. Raw database or owner-adapter errors are not serialized.

## Deliberate limits

This slice does not add or claim:

- batch diagnosis or caller-selected tenant scope;
- schema/source registry browsing;
- stale/missing entity discovery or orphan-link enumeration;
- finding inspection, resolve, ignore, or reopen commands;
- targeted, full, dry-run, or shadow repair;
- HTTP routes outside GraphQL, CLI, MCP, or native admin commands;
- retained authorization, PostgreSQL, GraphQL execution, or CI evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
node scripts/verify/verify-index-drift-diagnosis-graphql-transport.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.
