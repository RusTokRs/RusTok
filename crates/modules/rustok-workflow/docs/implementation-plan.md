# Implementation plan for `rustok-workflow`

### Workflow durable cron admission / outcome review refinement (2026-11-23)

Cron now atomically checks the current firing attempt and locked workflow
definition, then inserts its execution journal in the same transaction before
any external step begins. This closes the operator-requeue race where an older
worker could still insert a journal after a review found none. Long-running
attempts heartbeat only while the same token is current; the engine rechecks
the token before further steps and finalizes its status under a firing-row
write fence so a late worker cannot overwrite operator recovery.

Operator `confirm_external_completed` / `confirm_external_failed` can
resolve the full overall workflow execution outcome only with human
`workflows:manage` authority, bounded evidence and explicit confirmation
that the former worker is stopped. The operator must account for **all**
remaining steps and possible remote side effects, not just one HTTP response.
`confirm_terminal` remains a reconciliation of a previously terminal
journal, while `requeue_confirmed_unstarted` refuses any firing with an
execution journal. All updates and immutable review rows commit together.
This is not provider-level exactly-once. Migrator composition already includes
Workflow-owned migrations through the canonical ModuleMigrationSource.

Source regressions now cover competing claimers, immutable audit, stale token,
unstarted requeue, terminal review, external failure resolution, HTTP
no-blind-retry and schedule revision on step edits. None were run by request.


## 2026-11-23 — durable cron admission and uncertainty handling (unverified)

Implemented in source: additive cursor/firing/reconciliation schema, bounded
catch-up and compare-and-set cursor advancement, per-firing claim token,
stable UUID trigger identity and engine's terminal-record idempotency guard.
Aged claims are quarantined rather than replayed. An operator may confirm a
terminal execution, or requeue an unstarted firing only with explicit fenced
worker attestation and transactionally stored human evidence. REST/GraphQL
share permissioned owner commands. Workflow step edits now advance schedule
revisions within their transaction; provider-effect errors never run generic
blind retry/skip. Event listener acknowledgement follows the synchronous
engine result rather than fire-and-forget `spawn`.

Before production integration: approve the proposed ADR, apply and review
migrations for SQLite/PostgreSQL/MySQL, run scoped crate/server checks, exercise
two independent database connections in parallel, process-crash windows,
migration rollback and REST/GraphQL authorization. The first 24 hours of
catch-up are an explicit cutover horizon for workflows lacking a cursor.
Older, already-persisted cursors continue catching up across subsequent ticks.
Long-lived preexisting event executions and external provider uncertainty
remain review-only: never erase their current receipts or auto-replay a
possible side effect. No commands or checks have been run by the agent.

## Current state

`rustok-workflow` owns workflow definitions, execution journal, triggers, step
execution, GraphQL/REST/webhook ingress, and the workflow admin package. It is
not a generic scripting or event-transport layer. Webhook ingress is an
owner-owned Axum surface; cron and event-listener execution remain distinct.

The admin package has reached Phase B: a framework-agnostic core, selected
native/GraphQL transport facade, and explicit Leptos UI adapter. Native server
functions and HTTP/webhook handlers use `HostRuntimeContext` and narrow
`WorkflowHttpRuntime`, without host-framework dependencies.

The owner-owned overview and template gallery are mounted at `/modules/workflow`.
The legacy `/workflows` root redirects there; only workflow detail editing,
execution history, and version history remain host-composed until their atomic
transfer to this package.

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- The compatible provider-registry baseline is validated through
  `crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json` before
  Workflow-specific semantic checks run.
- FBA provider contract: `WorkflowReadPort` / `workflow.read_projection.v1` in
  `crates/modules/rustok-workflow/contracts/workflow-fba-registry.json`.
- Static, runtime-order, and compile-free evidence:
  `crates/modules/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json`,
  `crates/modules/rustok-workflow/contracts/evidence/workflow-provider-runtime-order-smoke.json`,
  and `crates/modules/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json`.
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
