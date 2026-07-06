# Documentation `rustok-installer`

`rustok-installer` is a support crate for the RusToK hybrid installer. It is not
a platform module and does not participate in tenant-level enable/disable.

## Purpose

The crate defines the common installer contract that should be reused by:

- CLI `rustok-server install ...`;
- HTTP surface `/api/install/*`;
- first-run web wizard;
- dev wrappers like `cargo xtask install-dev`.

## v1 Boundaries

- PostgreSQL is the default production DB and the only production-ready engine.
- SQLite is allowed only for `local`, `demo` and `test` scenarios.
- Module selection in v1 controls tenant enablement and build/profile intent, but not
  physical exclusion of module-owned schema from the global `Migrator`.
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

## Current CLI adapter

`apps/server` already connects an initial CLI surface:

- `rustok-server install preflight ...` builds an install plan and returns
  a `PreflightReport` without connecting to the DB.
- `rustok-server install plan ...` prints a redacted snapshot install plan.
- `rustok-server install apply ...` performs preflight, checks target DB,
  with `--create-database` creates PostgreSQL database/role via admin URL,
  runs server `Migrator::up`, applies tenant/module seed, creates or
  synchronizes superadmin, executes verify/finalize, writes `Preflight` /
  `Config` / `Database` / `Migrate` / `Seed` / `Admin` / `Verify` / `Finalize`
  receipts into `install_step_receipts` and transitions the session to `completed`.

`apply` resolves local secret refs `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` and `dotenv:<VAR>`. External
backends like `vault:*`, `kubernetes:*` and cloud secret managers remain
contract-level refs for `plan`/`preflight` and fail-fast on `apply` until an
external resolver is connected.

The HTTP adapter in `apps/server` publishes a thin surface for the Leptos wizard:
`GET /api/install/status`, `POST /api/install/plan`,
`POST /api/install/preflight`, `POST /api/install/apply`,
`GET /api/install/jobs/{job_id}` and
`GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` starts a
background job and calls the same server-side `apply_plan` pipeline as the CLI;
the UI must not duplicate migration/seed/admin logic.

## Related documents

- [Hybrid installer ADR](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md)
- [Module architecture](../../../docs/architecture/modules.md)
- [Platform database schema](../../../docs/architecture/database.md)
