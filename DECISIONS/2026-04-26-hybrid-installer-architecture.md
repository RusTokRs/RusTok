# Hybrid RusTok installer

- Date: 2026-04-26
- Status: Accepted

## Context

RusTok already has dev bootstrap via `cargo xtask install-dev` and a Docker launcher
`scripts/dev-start.sh`, but this is not a production-grade installer. The current mechanisms
are partially idempotent, but lack a common state model, receipts, resumable
logic, install lock, strict secret policy, and a dedicated operator UX.

The platform `Migrator` in `rustok-migrations` collects platform-core and
module-owned migrations into a single globally sorted list. Tenant-level
enable/disable works on top of an already assembled platform composition. Therefore,
the installer must not promise that selecting optional modules in v1 physically excludes
their schema artifacts from the database.

## Decision

RusTok uses a hybrid installer model:

1. `crates/rustok-installer` becomes a shared installer-core and the source of
   truth for install plan, state machine, preflight policy, secret references,
   receipts, and checksum/idempotency contract.
2. CLI `rustok-server install ...` will be the canonical operator interface for
   automation, CI/CD, and production runs.
3. The web wizard will be a thin facade on top of the same installer-core and HTTP adapter,
   not a separate implementation of bootstrap logic.
4. `cargo xtask install-dev` and `scripts/dev-start.sh` are preserved as backward
   compatible dev entrypoints, but must delegate installation logic to the
   installer-core as the CLI adapter is phased in.
5. The installer explicitly distinguishes between build composition, schema composition, and tenant
   enablement.
6. PostgreSQL is the first-class production database. SQLite is allowed only for
   local/demo/test scenarios. The production installer does not use a silent SQLite
   fallback.
7. Rollback after schema application is not treated as a universal reverse
   migration. For production recovery, backup/snapshot restore is used.

## Consequences

- The installer foundation is a support crate, not a platform module.
- The first version can manage tenant enablement and deployment profile intent,
  but does not remove module-owned schema for disabled modules.
- Secrets must be passed via `env`, `mounted-file`, or
  `external-secret`; `dotenv-file` remains a local/dev mode.
- Server startup guardrails such as sample-secret checks must become part of the
  installer preflight/finalize, rather than existing only as a runtime abort.
- For the web wizard, a setup token, install lock, rate limiting,
  CSRF/origin checks, and disabling setup routes after `Completed` are required.
- The Leptos admin mounts the wizard at `/install`: it forms an `InstallPlan`,
  performs preflight, starts a background apply job, and displays persisted
  receipts. The CLI remains the canonical interface for automation and production
  runbooks.
