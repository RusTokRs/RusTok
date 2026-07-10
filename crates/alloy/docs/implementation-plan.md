# Implementation plan for `alloy`

## Current state

`alloy` is the capability runtime for scripts, scheduling, hooks, execution
history, and automation permissions. It is not a tenant business module. The
crate owns GraphQL and HTTP surfaces, while the host composes
`SharedAlloyRuntime` and the narrow `AlloyHttpRuntime` through neutral runtime
contexts.

Sandbox limits, timeout/resource policy, scheduler semantics, execution audit
history, deterministic pagination, and hook/bridge behavior are locked in the
runtime contract. The crate does not depend on Loco.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Capability runtime contract:
  `crates/alloy/contracts/alloy-runtime-contract.json` and
  `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`.
- `scripts/verify/verify-alloy-runtime-contract.mjs` /
  `npm run verify:alloy:runtime-contract` lock static runtime, transport, and
  documentation evidence.

## Open results

1. **Promote static runtime locks into executable integration evidence.** Run
   router, schema, pagination, scheduler, hook, and sandbox scenarios through
   host-composed runtime fixtures where they provide stronger proof.
   **Depends on:** stable host test fixtures and runtime composition.
   **Done when:** executable checks cover the existing contract matrix without
   reintroducing server-owned Alloy controllers or runtime state.

2. **Continue the MCP/Admin Alloy draft-review surface.** Add operator-facing
   draft review only through the capability's published permission, runtime, and
   transport contracts.
   **Depends on:** approved MCP/Admin product requirements.
   **Done when:** drafts, review actions, audit history, and error handling have
   capability-owned semantics and a defined host composition boundary.

3. **Maintain operational runtime hardening.** Update sandbox, scheduler,
   execution-history, hook-debugging, and incident guidance with any capability
   behavior change.
   **Depends on:** the changed runtime contract.
   **Done when:** limits and failures are observable, auditable, and recoverable
   without leaking script policy into the server host.

## Verification

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- `npm run verify:alloy:runtime-contract`
- Targeted script execution, scheduling, tenant isolation, bridge, router, and
  schema integration tests.

## Change rules

1. Keep script policy, runtime, and execution history in this capability.
2. Update the root README, local docs, manifest, and host composition docs with
   a capability contract change.
3. Update `docs/modules/registry.md` with an FFA/FBA or module boundary change.
