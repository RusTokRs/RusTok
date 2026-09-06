# Pages Immutable Artifact Integrity Audit Transport

Date: 2026-08-07  
Status: `source-ready / maintainer-validation-pending`

## Purpose

Pages already owns the bounded read-only command:

```text
PageService::audit_immutable_artifact_integrity
```

This slice exposes that exact command through tenant-admin GraphQL and HTTP adapters. The adapters do not reconstruct artifacts, query immutable artifact tables, alter authorization semantics or add a second audit implementation.

## GraphQL

Mutation:

```text
auditPageArtifacts(
  id: UUID!
  input: { maxRecords: Int }
  tenantId: UUID
)
```

The mutation:

- requires the Pages module;
- requires an authenticated actor with effective `pages:manage`;
- rejects a requested tenant different from the current tenant;
- converts only the optional bounded record limit;
- delegates to the owner service;
- returns only the bounded owner result.

The service remains authoritative for tenant-wide `PermissionScope::All`. Under the current request permission bridge, Pages Manage resolves to `All` when effective `pages:manage` is present and to `None` when absent; there is no current Pages Manage `Own` request state. The service-level `All` check remains defense in depth.

## HTTP

Endpoint:

```text
POST /api/admin/pages/{id}/artifacts/audit
```

Request body:

```json
{
  "max_records": 128
}
```

The handler verifies that the authenticated actor and `TenantContext` refer to the same tenant, requires effective `pages:manage`, constructs the canonical security context and delegates to the same owner service.

The route is registered in the Pages Axum router and OpenAPI document. The existing owner DTOs remain the HTTP request and response schemas.

## Bounded response

Both adapters expose only:

- artifact UUID;
- fixed SHA-256 locale hash;
- fixed SHA-256 record-identity hash;
- one stable finding code;
- fixed SHA-256 diagnostic hash;
- bounded counts and truncation flags;
- deterministic audit hash.

The transport does not expose raw locale, stored build/artifact/content/materialization hashes, HTML, CSS, runtime snapshots, materialization identity JSON or internal integrity errors.

GraphQL maps owner counts into GraphQL signed integers only after the owner service has bounded every value to at most 512. HTTP reuses the owner DTO without transformation.

## Error boundary

Public transport errors are static:

```text
PAGE_NOT_FOUND
PAGES_PERMISSION_DENIED
PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT
PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED
```

Database, decoding, integrity and other internal `PagesError` text is never copied into the GraphQL or HTTP response by these adapters.

Invalid retained artifacts are not transport errors. They remain bounded findings inside a successful audit result.

## Preserved boundaries

This slice does not:

- query `page_static_landing_artifacts` from GraphQL or HTTP;
- write immutable artifacts or published bindings;
- emit lifecycle or cache events;
- publish, rollback, repair or rebuild;
- add migrations or persistence;
- add an admin UI;
- change public storefront routes;
- weaken the owner service authorization;
- promote FFA or FBA.

Repair/rebuild remains a separate source cursor. Executed authorization, GraphQL schema, OpenAPI and HTTP evidence remain pending.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-transport.mjs
cargo check -p rustok-pages
```

Retained execution should cover:

- GraphQL and HTTP success with current-tenant effective `pages:manage`;
- missing authentication;
- missing Manage permission;
- current Pages Manage `All`/`None` scope semantics plus the service `All` guard;
- current-tenant mismatch rejection;
- negative, zero and above-limit record inputs;
- page-not-found mapping;
- valid, invalid and truncated bounded results;
- static public errors with no internal error text;
- OpenAPI path and schema generation;
- zero artifact/binding writes and zero emitted events.

No test, verifier, Cargo, formatting, GraphQL, HTTP, database, workflow or CI execution is claimed here.
