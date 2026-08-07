# Pages / Page Builder Repair Transport Contract Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-transport-contract-harness-source-ready / request-contract-harness-source-ready / execution-pending
Scope: bounded GraphQL/OpenAPI repair contract evidence after explicit rebuild and activation transports

## Rechecked source state

Current `main` already contains the explicit append-only artifact rebuild command, explicit rebuilt-artifact binding activation, and separate tenant-admin GraphQL/HTTP/OpenAPI adapters for both commands.

The adapters preserve current-tenant fencing, effective `pages:manage` prechecks, owner-service tenant-wide authorization, static public error text and bounded result DTOs. Automatic audit-to-rebuild and rebuild-to-activation chaining remains absent.

A generated contract harness now exercises the real GraphQL schema and OpenAPI document, and a request-level harness is source-ready for the tenant/permission/static-error boundaries.

## Contract harness

Marker:

```text
explicit-artifact-repair-transport-contract-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_transport_contract.rs` builds the real merged `PagesQuery`/`PagesMutation` GraphQL schema and serializes the real Pages OpenAPI document.

The GraphQL contract test requires both explicit mutations:

```text
rebuildPageArtifact
activateRebuiltPageArtifact
```

It checks the generated result object fields and fails if rebuild or activation results expose provenance source id, source publish operation id, storage instance key, idempotency key, runtime payload, materialization identity or runtime snapshots.

The OpenAPI contract test requires both POST paths:

```text
/api/admin/pages/{id}/artifacts/rebuild
/api/admin/pages/{id}/artifacts/activate
```

It verifies registration of the explicit owner input schemas plus the two bounded transport result schemas, and applies the same forbidden-output-field assertions against generated OpenAPI schema properties.

The harness deliberately does not connect to a database, execute either GraphQL mutation, dispatch an HTTP request, invoke rebuild/activation services or observe lifecycle/cache effects. It therefore cannot be used as runtime evidence until the maintainer runs it.

## Request authorization actualization

Marker:

```text
explicit-artifact-repair-pages-manage-all-none-actualized
```

Earlier wording expected a request-level `owner-scoped pages:manage` case. The current request permission bridge does not represent that state for Pages Manage: effective `pages:manage` resolves to `PermissionScope::All`, and absence resolves to `PermissionScope::None`. `PermissionScope::Own` is not a current Pages Manage outcome.

The owner services still require `PermissionScope::All`; that remains a defense-in-depth invariant for direct/internal callers and future authorization-model changes. Request evidence now correctly covers current-tenant mismatch, Manage absent, and Manage present reaching owner validation.

The dedicated continuation is:

```text
docs/modules/pages-page-builder-repair-request-contract-continuation-2026-08-07.md
```

## Evidence boundary

Source evidence for generated schema/OpenAPI remains in:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-transport-contract-source.json
```

Request-level source evidence is retained separately in:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-request-contract-source.json
```

Both remain unvalidated; execution arrays are empty and validation flags remain false.

## Updated parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Explicit append-only rebuild owner command | Source-ready | DB/runtime evidence pending |
| Explicit rebuilt-artifact activation owner command | Source-ready | DB/lifecycle evidence pending |
| Bounded GraphQL/HTTP/OpenAPI repair transports | Source-ready | Transport execution pending |
| Generated GraphQL schema contract harness | Source-ready | Maintainer execution pending |
| Generated OpenAPI contract harness | Source-ready | Maintainer execution pending |
| Current-tenant and `pages:manage` request harness | Source-ready | Request execution pending |
| Pages Manage `All`/`None` scope semantics | Source actualized | Maintainer execution pending |
| Owner `PermissionScope::All` guard | Source-ready | Defense-in-depth evidence pending |
| Static public error response harness | Source-ready | Response evidence pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run `explicit_artifact_repair_transport_contract` and `explicit_artifact_repair_request_contract`; retain generated schema/OpenAPI plus request-response evidence.
2. Run the repair transport/request source guards and existing rebuild/activation source guards.
3. Prove request-level GraphQL and HTTP current-tenant mismatch, missing Manage and Manage-present owner-validation behavior.
4. Retain rebuild exact replay/conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction DB evidence.
5. Retain activation stale-version/current-artifact/reused-rebuild/invalid-target/unpublished rejection evidence.
6. Observe one successful activation lifecycle pair and event-driven cache-generation rotation only after commit.
7. Keep automatic audit-to-repair absent and defer FFA/FBA promotion until transport, DB, lifecycle/cache and tenant evidence are accepted.

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

Tests, source verifiers, Cargo commands, formatting, GraphQL requests, HTTP requests, database scenarios, lifecycle/cache observation, workflows and CI were intentionally not run.
