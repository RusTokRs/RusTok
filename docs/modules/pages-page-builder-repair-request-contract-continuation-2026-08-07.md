# Pages / Page Builder Repair Request Contract Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-request-contract-harness-source-ready / execution-pending
Scope: request-level GraphQL/HTTP authorization and static-error evidence for explicit artifact rebuild and activation

## Rechecked source state

Current `main` already contains:

- explicit append-only immutable artifact rebuild;
- explicit rebuilt-artifact activation;
- separate bounded GraphQL/HTTP/OpenAPI transports;
- a generated GraphQL SDL/OpenAPI contract harness.

The next gap was request-level evidence for the actual adapter fences and public error bodies.

## Corrected authorization model

Marker:

```text
explicit-artifact-repair-pages-manage-all-none-actualized
```

The earlier continuation wording expected an `owner-scoped pages:manage` request scenario. That scenario is not representable by the current permission bridge.

`security_context_from_access_token` infers a `SecurityContext` from the request-effective permission snapshot. For `Resource::Pages` + `Action::Manage`, the current RBAC scope function resolves:

```text
pages:manage present  -> PermissionScope::All
pages:manage absent   -> PermissionScope::None
```

There is no current `PermissionScope::Own` branch for Pages Manage. `Own` is reserved by the current RBAC implementation for narrower resource/action cases such as customer Orders and selected Comments writes.

Therefore request evidence must not claim that a currently representable owner-scoped Pages Manage grant passes the adapter and is rejected later by the owner command.

The owner services still require `PermissionScope::All`. That check remains useful defense in depth for direct/internal callers and for any future authorization model that could introduce a narrower Pages Manage scope.

## Request contract harness

Marker:

```text
explicit-artifact-repair-request-contract-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_request_contract.rs` exercises real GraphQL requests through the merged `PagesQuery`/`PagesMutation` schema and real Axum requests through `rustok_pages::http::axum_router`.

The harness covers both rebuild and activation.

### GraphQL

For each command it proves, when executed by the maintainer:

- an optional `tenantId` different from the current request tenant returns `PERMISSION_DENIED` with the static tenant-fence message;
- absence of effective `pages:manage` returns `PERMISSION_DENIED` with the static permission message;
- current-tenant effective `pages:manage` passes the adapter and owner `PermissionScope::All` check, then a nil page id is rejected as `PAGE_ARTIFACT_REPAIR_INVALID_INPUT` before any owner DB write;
- the Pages module enablement check is real, backed by a minimal `tenant_modules` SQLite fixture.

### HTTP

For each repair route it proves, when executed by the maintainer:

- authenticated-tenant/current-tenant mismatch returns HTTP `403` plus static `PAGES_PERMISSION_DENIED` body;
- absence of effective `pages:manage` returns HTTP `403` plus the static permission body;
- current-tenant effective `pages:manage` reaches owner validation, where a nil page id returns HTTP `400` plus static `PAGE_ARTIFACT_REPAIR_INVALID_INPUT` body.

The HTTP harness uses the real Pages Axum router, `TenantContextExtension` and `AuthContextExtension`. It does not add a fake transport-only authorization seam.

## Bounded execution boundary

The harness intentionally stops at owner input validation for the successful-authorization case. Its database contains only the minimal GraphQL `tenant_modules` fixture and does not run Pages migrations or seed artifact/provenance/binding tables.

That keeps this slice focused on request fences and public error contracts. Full rebuild/activation behavior remains owned by the existing database regressions and later accepted PostgreSQL/lifecycle evidence.

No automatic audit-to-rebuild or rebuild-to-activation behavior is introduced.

## Evidence

Source evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-request-contract-source.json
```

Status remains:

```text
pages_explicit_artifact_repair_request_contract_source_unvalidated
```

Execution is empty and every validation flag remains false until maintainer execution.

Source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
```

The guard is intentionally not run in this slice.

## Updated parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Generated GraphQL/OpenAPI repair contract | Source-ready | Maintainer execution pending |
| GraphQL current-tenant request fence | Harness source-ready | Request execution pending |
| GraphQL effective `pages:manage` fence | Harness source-ready | Request execution pending |
| HTTP tenant/auth mismatch fence | Harness source-ready | Request execution pending |
| HTTP effective `pages:manage` fence | Harness source-ready | Request execution pending |
| Static GraphQL validation response | Harness source-ready | Response evidence pending |
| Static HTTP validation response | Harness source-ready | Response evidence pending |
| Pages Manage `All`/`None` permission semantics | Source actualized + harness-ready | Execution pending |
| Owner `PermissionScope::All` defense-in-depth guard | Source-ready | Direct-owner evidence remains optional/pending |
| Rebuild DB/reproduction behavior | Source-ready | SQLite/PostgreSQL evidence pending |
| Activation DB/lifecycle/cache behavior | Source-ready | SQLite/PostgreSQL/lifecycle evidence pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run `explicit_artifact_repair_transport_contract` and `explicit_artifact_repair_request_contract`; retain generated schema/OpenAPI and request-response evidence.
2. Run both repair transport source guards plus existing rebuild/activation source guards.
3. Retain rebuild exact replay/conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction evidence.
4. Retain activation stale-version/current-artifact/reused-rebuild/invalid-target/unpublished rejection evidence.
5. Add accepted PostgreSQL coverage for rebuild/activation receipt constraints and transaction rollback behavior.
6. Observe one successful activation lifecycle pair and route/page/artifact cache-generation rotation only after commit.
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

Tests, source verifiers, Cargo commands, formatting, GraphQL requests, HTTP requests, Pages database scenarios, lifecycle/cache observation, workflows and CI were intentionally not run.
