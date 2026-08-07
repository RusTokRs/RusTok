# Pages / Page Builder Repair Transport Contract Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-transport-contract-harness-source-ready / execution-pending
Scope: bounded GraphQL/OpenAPI repair contract evidence after explicit rebuild and activation transports

## Rechecked source state

Current `main` already contains the explicit append-only artifact rebuild command, explicit rebuilt-artifact binding activation, and separate tenant-admin GraphQL/HTTP/OpenAPI adapters for both commands.

The adapters preserve current-tenant fencing, effective `pages:manage` prechecks, owner-service tenant-wide authorization, static public error text and bounded result DTOs. Automatic audit-to-rebuild and rebuild-to-activation chaining remains absent.

What remained missing at source level was a runnable contract harness that exercises the generated GraphQL schema and OpenAPI document rather than proving those surfaces only through string inspection of adapter source.

## Contract harness

Marker:

```text
explicit-artifact-repair-transport-contract-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_transport_contract.rs` now builds the real merged `PagesQuery`/`PagesMutation` GraphQL schema and serializes the real Pages OpenAPI document.

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

## Evidence boundary

Source evidence is retained in:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-transport-contract-source.json
```

Its status remains `pages_explicit_artifact_repair_transport_contract_source_unvalidated`; execution is empty and all validation flags remain false.

The fail-closed source guard is:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
```

The guard itself is also intentionally not run in this slice.

## Updated parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Explicit append-only rebuild owner command | Source-ready | DB/runtime evidence pending |
| Explicit rebuilt-artifact activation owner command | Source-ready | DB/lifecycle evidence pending |
| Bounded GraphQL/HTTP/OpenAPI repair transports | Source-ready | Transport execution pending |
| Generated GraphQL schema contract harness | Source-ready | Maintainer execution pending |
| Generated OpenAPI contract harness | Source-ready | Maintainer execution pending |
| Current-tenant and `pages:manage` adapter execution | Source-ready adapters | Request execution pending |
| Tenant-wide versus owner-scoped owner authorization | Source-ready owner checks | Transport execution pending |
| Static public error response execution | Source-ready mapping | Response evidence pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run `explicit_artifact_repair_transport_contract` and retain its GraphQL SDL/OpenAPI result evidence.
2. Run the repair-transport source guard and existing rebuild/activation source guards.
3. Add/execute request-level GraphQL evidence for current-tenant override rejection, missing `pages:manage`, tenant-wide Manage success and owner-scoped Manage rejection before writes.
4. Add/execute request-level HTTP evidence for the same tenant/authorization fences and stable status/code mapping.
5. Retain rebuild exact replay/conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction DB evidence.
6. Retain activation stale-version/current-artifact/reused-rebuild/invalid-target/unpublished rejection evidence.
7. Observe one successful activation lifecycle pair and event-driven cache-generation rotation only after commit.
8. Keep automatic audit-to-repair absent and defer FFA/FBA promotion until transport, DB, lifecycle/cache and tenant evidence are accepted.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
cargo test -p rustok-pages --test explicit_artifact_repair_transport_contract -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, GraphQL mutation execution, HTTP request execution, database scenarios, lifecycle/cache observation, workflows and CI were intentionally not run.
