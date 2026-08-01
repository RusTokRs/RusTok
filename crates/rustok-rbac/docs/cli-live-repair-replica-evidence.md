# RBAC live CLI repair replica recovery evidence

## Status

`source_ready_unvalidated`

This source packet continues `cycle-001` at `core/rbac`. It does not advance or complete the component.

## Purpose

The operational command `rbac repair-system-roles --apply` repairs built-in role definitions and reserves a durable invalidation generation in the same database transaction. Live server replicas must observe that generation and discard stale authorization snapshots without requiring a process restart.

The harness in `crates/rustok-rbac/cli/tests/live_repair_replica_recovery.rs` exercises the production CLI command provider with two independent live observer processes and one independent CLI process against one isolated PostgreSQL database.

## Scenario

1. Create one tenant and one active user.
2. Assign the canonical Manager system role through `RbacRoleAssignmentDbWriter`.
3. Add an extra `settings:manage` role-permission link so both authoritative and cached decisions allow the permission before repair.
4. Start two independent observer processes.
5. In each observer, start the production durable-generation watchdog and warm the stale allow through `RbacService::has_permission`.
6. Start a third process with role `cli`.
7. Construct `RuntimeComposition` from the isolated database and invoke the production `rustok_rbac_cli::command_provider` with:

   ```text
   namespace=rbac
   command=repair-system-roles
   tenant_id=<fixture tenant>
   apply=true
   ```

8. Require a successful command outcome, at least one repair change, durable generation `1`, and `runtime_restart_required_if_applied=false`.
9. Require the stale Manager link to be removed.
10. Require both already-running observers to clear their process-local permission snapshots through the production watchdog and converge to the authoritative deny within eight seconds.

## Ownership boundary

The test invokes the CLI adapter rather than importing the owner repair functions. The CLI implementation remains responsible for:

- opening the transaction;
- applying the owner repair plan;
- reserving the durable generation only when users are affected;
- committing repair plus generation once;
- returning the committed generation in the command result.

The server observers remain responsible only for reading the durable generation and clearing their own process-local permission cache. They do not repair role definitions or acknowledge the generation on behalf of another replica.

## Forbidden shortcuts

The harness must not:

- call `apply_system_role_repair_in_transaction` or `plan_system_role_repair` directly;
- call `reserve_permission_invalidation_generation` directly;
- call any manual RBAC cache-clear helper;
- publish a synthetic local or Redis invalidation;
- run both observers in one process;
- replace the CLI provider with a test-only repair implementation;
- use SQLite.

Redis is intentionally absent. Merged PR #2856 owns Redis available/outage/restart evidence, while merged PR #2853 owns intentionally missed-publication watchdog evidence. This packet isolates the production CLI repair adapter and proves by construction that its committed generation is sufficient for two live replicas to recover without restart.

## Evidence boundary

This packet adds source coverage only. It does not claim that Rust tests, source verifiers, formatting, compilation, PostgreSQL, subprocesses, workflows, or CI were executed.

It does not prove:

- retained runtime evidence from an actual execution;
- Redis delivery or restart behavior;
- CLI shell/parser behavior outside the registered command provider;
- the complete exact-revision RBAC compile and module gates;
- the complete multi-replica P0 gate.

`core/rbac` therefore remains `in_progress`.

## Targeted commands

```bash
cargo test -p rustok-rbac-cli \
  --test live_repair_replica_recovery \
  cli_system_role_repair_recovers_two_live_replicas_without_restart \
  -- --ignored --nocapture
node scripts/verify/verify-rbac-cli-live-repair-source.mjs
```

Run the remaining exact-revision RBAC commands from the canonical implementation plan before promoting this packet to retained evidence.