# Implementation plan for `rustok-rbac`

## Current state

`rustok-rbac` is the single tenant-policy runtime for permission decisions,
role/permission assignments, and authorization policy. The relation store is
the assignment source of truth; `apps/server` provides adapters and
observability only. No shadow policy engine or presentation-only role inference
may participate in live authorization.

The admin overview is an intentional native-only surface with a module-owned
core, transport facade, and UI adapter. It shows the built-in role catalog and
runtime overview; a GraphQL/REST secondary path requires an approved remote or
headless operator contract.

`RbacRoleAssignmentDbWriter` owns idempotent built-in role, permission, and
relation persistence for bootstrap consumers with an explicit database handle.
Host adapters invalidate process-local authorization caches after it succeeds;
they do not duplicate RBAC relation writes.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `RbacPermissionDecisionPort` /
  `rbac.permission_decision.v1` in
  `crates/rustok-rbac/contracts/rbac-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json`
  and `crates/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-rbac-admin-boundary.mjs` and `npm run verify:rbac:fba`
  lock the native-only boundary, provider metadata, and authorization order.

## Open results

1. **Collect live host evidence for permission decisions.** Exercise
   `RbacPermissionDecisionPort` with tenant scope, claims, deadline, cache, and
   degraded behavior before promoting FBA.
   **Depends on:** composed host execution and representative identities.
   **Done when:** targeted runtime tests prove the module evaluator is the only
   decision engine for allowed and denied requests.

2. **Expand operator role/permission flows through the owner package.** Add
   management actions only with module-owned validation, authorization evidence,
   and a decision on whether a headless transport contract is needed.
   **Depends on:** approved operator requirements and policy mutation contract.
   **Done when:** UI actions use the facade, role changes publish the expected
   integration events, and no host-owned `/roles` surface reappears.

3. **Keep drift prevention and operability current.** Cover new resolver/event
   paths with integration tests and synchronize telemetry, cache, adapter, and
   runbook expectations with each live policy change.
   **Depends on:** the change-owning authorization surface.
   **Done when:** a policy incident can be traced to one evaluator, cache state,
   and recovery procedure.

## Verification

- `npm run verify:rbac:admin-boundary`
- `npm run verify:rbac:fba`
- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- Targeted permission-resolution, authorization-decision, cache, and
  integration-event tests.

## Change rules

1. Keep policy evaluation and assignments in this module.
2. Update local docs, `rustok-module.toml`, and server adapter documentation
   with a public authorization contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
