# Implementation plan for `rustok-workflow`

## Current state

`rustok-workflow` owns workflow definitions, execution journal, triggers, step
execution, GraphQL/REST/webhook ingress, and the workflow admin package. It is
not a generic scripting or event-transport layer. Webhook ingress is an
owner-owned Axum surface; cron and event-listener execution remain distinct.

The admin package has reached Phase B: a framework-agnostic core, selected
native/GraphQL transport facade, and explicit Leptos UI adapter. Native server
functions and HTTP/webhook handlers use `HostRuntimeContext` and narrow
`WorkflowHttpRuntime`, without Loco dependencies.

The owner-owned overview and template gallery are mounted at `/modules/workflow`.
The legacy `/workflows` root redirects there; only workflow detail editing,
execution history, and version history remain host-composed until their atomic
transfer to this package.

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- FBA provider contract: `WorkflowReadPort` / `workflow.read_projection.v1` in
  `crates/rustok-workflow/contracts/workflow-fba-registry.json`.
- Static, runtime-order, and compile-free evidence:
  `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json`,
  `crates/rustok-workflow/contracts/evidence/workflow-provider-runtime-order-smoke.json`,
  and `crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json`.
- `scripts/verify/verify-workflow-admin-boundary.mjs` and
  `npm run verify:workflow:fba` lock the FFA boundary and FBA provider order.

## Open results

1. **Collect live workflow read-parity evidence.** Execute native
   `list_workflows`, GraphQL selected-path reads, tenant scope, and typed
   `PortError` mapping in a composed backend before FBA promotion.
   **Depends on:** a runtime-composed backend and workflow fixtures.
   **Done when:** native and GraphQL paths provide reproducible parity evidence
   for the published read-projection profiles.

2. **Finish production execution-history hardening.** Extend the existing
   duplicate-delivery protection with real PostgreSQL history coverage and
   operational evidence for journal persistence and trigger replay.
   **Depends on:** PostgreSQL test environment and trigger runtime fixtures.
   **Done when:** duplicate events, tenant isolation, history queries, and
   failure/retry behavior are validated against the production persistence path.

3. **Evolve steps and observability only through workflow ownership.** Complete
   `alloy_script` and `notify` capabilities, add `workflow.execution.*`
   telemetry, and consider DAG/branching only for demonstrated product pressure.
   **Depends on:** capability contracts, event schema, and product requirements.
   **Done when:** every new step has execution, failure, observability, and
   documentation semantics without turning workflow into a generic script host.

## Verification

- `npm run verify:workflow:admin-boundary`
- `npm run verify:workflow:fba`
- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- Targeted trigger, step, execution-journal, tenant-isolation, and admin/runtime
  contract tests.

## Change rules

1. Keep workflow orchestration, ingress, and execution history in this module.
2. Update local docs, `rustok-module.toml`, and event/capability documentation
   with a workflow contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
