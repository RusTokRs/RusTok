# Explicit Artifact Repair Transports

Date: 2026-08-07  
Status: `source-ready / maintainer-validation-pending`

## Purpose

Pages already owns two separate tenant-admin commands:

1. `rebuild_immutable_artifact` appends one exact immutable rebuild candidate without changing public output;
2. `replace_rebuilt_artifact_binding` explicitly activates one rebuild receipt for one locale.

This slice adds bounded GraphQL, HTTP and OpenAPI adapters for those existing owner commands. It does not merge audit, rebuild and activation into one automatic repair action.

## GraphQL

`PagesMutation` now mounts:

```text
rebuildPageArtifact
activateRebuiltPageArtifact
```

Both mutations:

- require the Pages module to be enabled;
- require an effective `pages:manage` permission before delegation;
- require the authenticated actor and any optional `tenantId` override to match the current tenant;
- derive the owner `SecurityContext` from the authenticated access token;
- delegate exactly once to the corresponding `PageService` command;
- return only bounded receipt fields;
- map every owner failure to a static public code and static message.

The owner commands remain authoritative and independently require tenant-wide `PermissionScope::All`. In the current request permission model Pages Manage does not have an `Own` scope: effective `pages:manage` resolves to `All`, while its absence resolves to `None`. Request evidence therefore covers Manage present/absent rather than inventing an owner-scoped Pages Manage grant. The owner `All` check remains defense in depth for direct/internal callers and any future authorization model that could introduce a narrower Pages Manage scope.

## HTTP

The Pages Axum router now mounts:

```text
POST /api/admin/pages/{id}/artifacts/rebuild
POST /api/admin/pages/{id}/artifacts/activate
```

The rebuild route accepts `RebuildPageArtifactInput`. The activation route accepts `ReplacePageArtifactBindingInput`. Both inputs remain explicit and idempotent; no adapter selects a provenance source, rebuild receipt, current artifact or version on behalf of the caller.

The routes require current-tenant identity plus effective `pages:manage`, then delegate to the owner service. HTTP status and error code mapping is static:

- `400` for malformed command input or rejected reviewed-runtime input;
- `403` for permission or tenant failure;
- `404` for a missing page;
- `409` for stale state, source/target, reproduction, reuse or idempotency conflicts;
- `500` for stored receipt or persistence integrity failures.

No `PagesError` text is copied into GraphQL or HTTP responses.

## Bounded result shape

The transport rebuild receipt exposes only:

- rebuild operation and page ids;
- locale;
- source and rebuilt artifact ids;
- verified artifact and materialization hashes;
- replay flag and timestamp.

The transport activation receipt exposes only:

- activation operation and page ids;
- resulting page version and locale;
- selected rebuild operation id;
- previous and replacement artifact ids;
- verified replacement artifact and materialization hashes;
- replay flag and timestamp.

Transport results deliberately omit:

- provenance source id;
- source publish operation id;
- artifact storage instance key;
- idempotency keys;
- sanitized project data;
- reviewed runtime context;
- materialization identity JSON;
- runtime snapshots;
- document HTML, body HTML or CSS;
- internal diagnostics.

## OpenAPI

`PagesApiDoc` registers both HTTP paths and the explicit owner inputs plus bounded public result schemas.

## Preserved owner boundary

The GraphQL and HTTP modules do not import Pages entities and do not query or mutate artifact, provenance, receipt, binding or page tables. They do not compile Page Builder data, emit lifecycle events or touch cache generations.

All rebuild integrity, activation fencing, transactionality, version changes, lifecycle events and idempotent receipts remain owned by `PageService`.

## Deliberately absent

This slice does not add:

- automatic audit-to-rebuild behavior;
- automatic rebuild-to-activation behavior;
- a combined repair endpoint;
- source or rebuild discovery/list endpoints;
- raw provenance/runtime inspection endpoints;
- admin UI or worker scheduling;
- database schema changes;
- FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
cargo test -p rustok-pages --test explicit_artifact_repair_transport_contract -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_repair_request_contract -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
cargo check -p rustok-pages --all-targets
```

Generated schema/OpenAPI execution, request-level tenant/permission/static-error evidence and full database/runtime scenarios remain pending.
