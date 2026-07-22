---
id: doc://docs/guides/scheduler.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Scheduled Operations

RusToK does not run a scheduler inside `apps/server`. Schedule typed
`rustok-cli` commands with the deployment platform's
cron or job runner. Each invocation must receive `RUSTOK_DATABASE_URL` (or
`DATABASE_URL`) and any command-specific settings through
`RUSTOK_SETTINGS_JSON`.

## Current operations

| Operation | Command | Suggested cadence | Owner |
|---|---|---|---|
| Expired session cleanup | `rustok-cli auth sessions-cleanup` | hourly | `rustok-auth` |
| Build queue execution | `rustok-cli core rebuild` | deployment-defined polling cadence | `rustok-build` |
| Media storage reconciliation | `rustok-cli media reconcile --limit <count>` | deployment-defined | `rustok-media` |

Use `rustok-cli list --json` to obtain the currently enabled command inventory.
Module-owned operations must declare their provider through `[provides.cli]`;
platform-owned operations belong in `rustok-cli-platform`.

## Example deployment cron

```cron
0 * * * * /usr/local/bin/rustok-cli auth sessions-cleanup
*/5 * * * * /usr/local/bin/rustok-cli core rebuild
```

The process runner should record exit status and standard output. The commands
are idempotent where their owner contract requires retry-safe operation.

## Runtime schedulers

`WorkflowCronScheduler` and the Alloy scheduler are capability-owned runtime
services. They are started through their respective host runtime composition;
they are not deployment cron jobs and are not configured in this guide.

## Related Documents

- [Axum Runtime and Operations CLI Boundary](../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md)
- [rustok-media Documentation](../../crates/rustok-media/docs/README.md)
- [Observability](./observability-quickstart.md)
