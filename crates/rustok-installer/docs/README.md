# Documentation `rustok-installer`

`rustok-installer` is a support crate for the RusToK hybrid installer. It is not
a platform module and does not participate in tenant-level enable/disable.

## Purpose

The crate defines the common installer contract reused by:

- CLI `rustok-cli install ...`;
- HTTP surface `/api/install/*`;
- first-run web wizard;
- dev wrappers such as `cargo xtask install-dev`.

## v1 Boundaries

- PostgreSQL is the default production DB and the only production-ready engine.
- SQLite is allowed only for `local`, `demo` and `test` scenarios.
- Module selection controls tenant enablement and build/profile intent, but not
  physical exclusion of module-owned schema from the global `Migrator`.
- `SeedProfile` owns its canonical default module set; host installers may only
  apply explicit enable/disable overrides from the install plan.
- `SeedExecutionRequest` composes tenant, identity, role and module owner ports;
  it has no server model or database adapter dependency and is exposed by the
  default `seed-runtime` feature.
- Install-plan, state, receipt, preflight, deployment, secret, and executor
  contracts remain available without default features so the web wizard can
  share their exact types without linking the native seed runtime.
- Rollback after schema application must not promise a universal reverse
  migration; production restore relies on backup/snapshot.

## Installation states

Main happy path:

```text
Draft
-> PreflightPassed
-> ConfigPrepared
-> DatabaseReady
-> SchemaApplied
-> SeedApplied
-> AdminProvisioned
-> Verified
-> Completed
```

Error/operational states:

```text
Failed
RolledBackFreshInstall
RestoreRequired
```

## Current adapters and target topology

The server CLI parser was removed with the Axum cutover. `apps/server` hosts a
thin HTTP adapter; `rustok-cli install plan|preflight|apply|status` and
`rustok-cli seed apply` use the shared typed executor and SeaORM adapters. The
standalone apply adapter opens the target database itself, so a requested
database may be created before a CLI runtime database exists.

An apply operation resolves local secret refs `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` and `dotenv:<VAR>`. External
backends such as `vault:*`, `kubernetes:*` and cloud secret managers remain
contract-level refs for `plan`/`preflight` and fail-fast on `apply` until an
external resolver is connected.

The HTTP adapter publishes a thin wizard surface:
`GET /api/install/status`, `POST /api/install/plan`,
`POST /api/install/preflight`, `POST /api/install/apply`,
`GET /api/install/jobs/{job_id}`, and
`GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` starts a
background job; the UI must not duplicate migration, seed, or admin logic.

The topology contract distinguishes a one-role `monolith` from a distributed
deployment descriptor. Trusted CLI and HTTP hosts bind the selected
distribution revision/hash before preflight and apply; a wizard never supplies
that identity. `InstallDeploymentPort` and deterministic
`InstallRoleDeploymentRequest` values define the neutral role hand-off: a host
adapter must build, publish, wait for, and idempotently return an active release
for the same composition, role, and surfaces. A distributed role is
single-purpose (`api`, `admin_ssr`, `storefront_ssr`, `worker`, or `registry`)
and may not claim another role's surface. The Axum server supplies the first
adapter when `rustok.build.enabled=true`; it applies schema, tenant seed, and
admin provisioning once, then records an active release receipt for each role.
Standalone CLI preflight remains unavailable for distributed apply until it is
configured with its own deployment adapter. See the
[implementation plan](implementation-plan.md) for ownership and rollout.

## Related documents

- [Hybrid installer ADR](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md)
- [Installer topology composition identity ADR](../../../DECISIONS/2026-07-12-installer-topology-composition-identity.md)
- [Module architecture](../../../docs/architecture/modules.md)
- [Platform database schema](../../../docs/architecture/database.md)
- [Installer implementation plan](implementation-plan.md)
