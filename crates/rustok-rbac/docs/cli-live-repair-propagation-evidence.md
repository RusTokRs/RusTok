# Live CLI system-role repair propagation evidence

Status: `source_ready_unvalidated`

This packet continues the active `cycle-001` `core/rbac` verification item. It
adds source coverage for the remaining P0 requirement to exercise canonical CLI
system-role repair while multiple server replicas remain live. It does not
advance the platform verification cursor or complete RBAC.

## Topology

The ignored integration test creates:

- one isolated PostgreSQL database migrated by the workspace migrator;
- two independent long-lived observer processes;
- one independent short-lived CLI process.

Each observer owns a separate process-local permission cache, server runtime
context, cache listener and durable-generation watchdog. Redis configuration is explicitly removed from every child process. The previously added Redis harness
covers transport delivery and restart; this packet isolates CLI propagation so
the database generation watchdog is the only cross-process recovery path.

The parent records observer process identifiers before and after recovery.
Restarting an observer is not an accepted recovery path.

## Drift fixture

Two active users receive the canonical Manager role. The fixture adds one extra
`settings:manage` permission relation to that built-in role. The canonical cache
listener completes its synchronous initial durable-generation recovery at
generation zero before each observer warms an allowed decision.

The repair must remove the extra relation. The authoritative decision for both
users then becomes deny.

## Canonical CLI path

The third process invokes:

```text
rustok-cli rbac repair-system-roles --apply --tenant-id <tenant UUID>
```

It uses `rustok_cli::run_with_runtime`, the selected distribution registry and
the registered `rustok-rbac-cli` command provider. The runtime contains only the PostgreSQL connection and settings object. It does not receive a server cache service, process-local invalidation bus or Redis handle.

The command result must report:

- exit code zero;
- applied mode;
- at least one removed role-permission relation;
- exactly two affected users;
- durable generation one;
- no runtime restart requirement.

The harness deliberately does not call the owner repair function, generation
allocator or server committed-repair facade directly.

## Live replica recovery

The CLI adapter repairs role definitions and reserves the durable invalidation
generation in the same database transaction. It does not publish a local or
Redis invalidation. Therefore each observer must independently detect generation
advance through the canonical five-second watchdog.

For both live replicas, the packet requires:

- the same process identifier before and after repair;
- durable and applied generation one;
- at least one `generation_advanced` recovery;
- at least one `generation_advanced` full clear;
- recovery within seven seconds;
- cached and authoritative final decisions both deny;
- Redis remains unconfigured;
- cache-listener and watchdog tasks remain active.

This verifies the source shape for CLI repair propagation without server restart,
Redis delivery or same-process cache sharing.

## Forbidden shortcuts

The integration harness does not:

- call `apply_system_role_repair_in_transaction` directly;
- call `reserve_permission_invalidation_generation` directly;
- call `RbacService::repair_system_roles_committed`;
- manually clear user or global permission caches;
- update `rbac_invalidation_state` with test SQL;
- publish a synthetic invalidation message;
- configure Redis as an alternate recovery path;
- model replicas as two contexts in one process;
- terminate and replace observers after repair.

## Evidence boundary

No Rust test, source verifier, formatting, Cargo check, PostgreSQL execution,
subprocess execution, workflow or CI check was run in the connector-only work
unit that added this packet. Redis was neither configured nor executed by this
source harness.

The packet remains `source_ready_unvalidated`. It does not close:

- same-revision compile, lint, module validate or module test gates;
- retained execution of the PostgreSQL concurrency harness;
- retained execution of the incident trace packet;
- retained execution of the Redis available, outage and restart harnesses;
- live negative HTTP, GraphQL, WebSocket and native transport requests;
- the complete `core/rbac` verification item.

## Targeted execution

```bash
cargo test -p rustok-cli \
  --test rbac_live_repair_propagation \
  live_cli_system_role_repair_reaches_two_running_replicas_without_restart \
  -- --ignored --nocapture
node scripts/verify/verify-rbac-cli-live-repair-propagation-source.mjs
```

Required environment:

```text
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://...
```
