# Pages / Page Builder Repair Request Contract Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-request-contract-harness-source-ready / explicit-artifact-repair-postgres-harness-source-ready / execution-pending
Scope: request-level GraphQL/HTTP authorization, static-error evidence and PostgreSQL repair transaction coverage

## Rechecked source state

Current `main` already contains:

- explicit append-only immutable artifact rebuild;
- explicit rebuilt-artifact activation;
- separate bounded GraphQL/HTTP/OpenAPI transports;
- a generated GraphQL SDL/OpenAPI contract harness;
- a request-level GraphQL/HTTP tenant, permission and static-error harness.

This continuation now also links the source-ready PostgreSQL repair packet that exercises the real rebuild/activation owner commands and migration-owned receipt constraints.

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

## PostgreSQL repair packet

Marker:

```text
explicit-artifact-repair-postgres-harness-source-ready
```

The environment-gated harness is:

```text
crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs
```

It creates an isolated PostgreSQL schema, applies the real `OutboxModule` and `PagesModule` migrations and uses the owner services end-to-end after reviewed publication.

At source level it now covers:

- append-only rebuild after mutable-body drift and active-artifact corruption;
- unchanged binding and page version after rebuild;
- exact rebuild replay and idempotency conflict;
- migration-owned rebuild receipt uniqueness with transaction rollback;
- stale-current activation rejection without receipt/binding mutation;
- successful one-locale activation and one page-version increment;
- retained source and replacement artifact rows;
- durable `NodeUpdated` plus `NodePublished` activation envelopes;
- exact activation replay;
- rejection of a second activation for the already-consumed rebuild receipt;
- migration-owned activation receipt uniqueness;
- rollback of a prior page marker and prior outbox write when that receipt constraint rejects the transaction.

The detailed packet is retained in:

```text
docs/modules/pages-page-builder-repair-postgres-continuation-2026-08-07.md
```

Its machine evidence remains unvalidated and no PostgreSQL execution is claimed.

## Bounded execution boundary

The request harness intentionally stops at owner input validation for the successful-authorization case. Its database contains only the minimal GraphQL `tenant_modules` fixture and does not run Pages migrations or seed artifact/provenance/binding tables.

The PostgreSQL harness owns the deeper owner/migration transaction packet and remains separately environment-gated. This keeps transport authorization evidence and database atomicity evidence independently runnable and independently reviewable.

No automatic audit-to-rebuild or rebuild-to-activation behavior is introduced.

## Evidence

Request source evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-request-contract-source.json
```

PostgreSQL source evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-postgres-source.json
```

Statuses remain unvalidated; execution arrays are empty and validation flags remain false until maintainer execution.

Source guards:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
```

Neither guard is run in this source slice.

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
| Owner `PermissionScope::All` defense-in-depth guard | Source-ready | Direct-owner evidence optional/pending |
| Rebuild PostgreSQL real-migration flow | Harness source-ready | PostgreSQL execution pending |
| Rebuild receipt constraint/rollback | Harness source-ready | PostgreSQL execution pending |
| Activation PostgreSQL one-locale flow | Harness source-ready | PostgreSQL execution pending |
| Activation durable lifecycle pair | Harness source-ready | PostgreSQL execution pending |
| Activation receipt/outbox rollback atomicity | Harness source-ready | PostgreSQL execution pending |
| Cache-generation rotation after activation | Source-owned by lifecycle handler | Observation pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run `explicit_artifact_repair_transport_contract`, `explicit_artifact_repair_request_contract` and `explicit_artifact_repair_postgres`; retain generated schema/OpenAPI, request-response and PostgreSQL evidence.
2. Run the three focused source guards plus existing rebuild/activation owner guards.
3. Retain rebuild provenance-corruption and reviewed-runtime-mismatch evidence beyond the replay/conflict packet now present on PostgreSQL.
4. Retain activation stale-version, invalid-target and unpublished rejection evidence beyond the stale-current/reused-rebuild packet now present on PostgreSQL.
5. Feed one committed activation lifecycle pair through the real Pages cache invalidation handler and prove route/page/artifact generation rotation only after commit.
6. Keep automatic audit-to-repair absent and defer FFA/FBA promotion until transport, DB, lifecycle/cache and tenant evidence are accepted.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
cargo test -p rustok-pages --test explicit_artifact_repair_transport_contract -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_repair_request_contract -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, GraphQL/HTTP requests, PostgreSQL scenarios, lifecycle-handler/cache observation, workflows and CI were intentionally not run.
