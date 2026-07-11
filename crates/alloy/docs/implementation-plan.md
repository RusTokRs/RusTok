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

The accepted target boundary makes Alloy the authoring and evolution capability
over the neutral `rustok-sandbox` foundation. Rhai sources, revisions, draft
review, repair and release lineage remain Alloy-owned. Generic executor policy,
capability enforcement and sandbox outcomes move to the shared foundation so
Alloy drafts and installed marketplace artifacts use the same isolation model.

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

1. **Adopt the neutral sandbox foundation.** Move the generic Rhai engine limits
   and outcome mapping behind `rustok-sandbox`. Keep Alloy-specific entity and
   service bridges as capability adapters.
   **Depends on:** shared execution subject, policy, broker and executor contracts.
   **Done when:** Alloy draft/manual/hook execution uses the common sandbox and
   Alloy no longer owns a parallel production executor policy.

2. **Publish and evolve Rhai module artifacts.** Package reviewed Rhai sources
   with immutable descriptors and lineage, publish them through the marketplace,
   and import or fork a release into a new Alloy revision for further work.
   **Depends on:** `rustok-modules` publication and installation operations.
   **Done when:** edits produce a new semantic version and digest while previous
   installed releases remain reproducible.

3. **Promote static runtime locks into executable integration evidence.** Run
   router, schema, pagination, scheduler, hook, and sandbox scenarios through
   host-composed runtime fixtures where they provide stronger proof.
   **Depends on:** stable host test fixtures and runtime composition.
   **Done when:** executable checks cover the existing contract matrix without
   reintroducing server-owned Alloy controllers or runtime state.

4. **Continue the MCP/Admin Alloy draft-review surface.** Add operator-facing
   draft review only through the capability's published permission, runtime, and
   transport contracts.
   **Depends on:** approved MCP/Admin product requirements.
   **Done when:** drafts, review actions, audit history, and error handling have
   capability-owned semantics and a defined host composition boundary.

5. **Maintain operational runtime hardening.** Update sandbox, scheduler,
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

1. Keep authoring policy, source revisions, draft review and Alloy-specific bridges in this capability; keep generic sandbox policy in `rustok-sandbox`.
2. Update the root README, local docs, manifest, and host composition docs with
   a capability contract change.
3. Update `docs/modules/registry.md` with an FFA/FBA or module boundary change.
