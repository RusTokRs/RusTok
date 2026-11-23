
## Cron execution task lifecycle (2026-11-23)

The Workflow owner tracks cron-fired execution tasks in its scheduler-owned
`JoinSet`, rather than detaching `tokio::spawn` handles. The host calls
`WorkflowCronScheduler::start_with_stop`: a stop signal prevents admission of
further scheduled executions, including after the workflow/step read awaits,
and the scheduler joins already-started executions before returning. Completed
tasks are reaped and panics observed during subsequent cron ticks. Aborting or
dropping the scheduler's task cancels remaining children instead of leaving
them detached. The existing `start` API still supports callers without a host
stop signal, but it also owns its execution tasks.

This is **process-local shutdown supervision**, not durable schedule delivery.
Multiple hosts can still execute the same cron firing, skipped seconds or
process death can miss firings, and forcibly aborted or crashed executions may
retain `running` rows. Cross-replica claims, recovery, and execution idempotency
need separate Workflow-owned durable design and verification. Tests are
source-only and unrun here.
# `rustok-workflow` Documentation

`rustok-workflow` — visual automation module on the platform queue and
event infrastructure. It orchestrates workflow execution over platform
events and must not become a second event bus or transport runtime.

## Purpose

- publish the canonical workflow runtime contract for triggers, steps and executions;
- keep the workflow engine, execution journal and module-owned transport/UI surfaces inside the module;
- evolve the automation layer over platform events without duplicating event transport.

## Scope

- `WorkflowService`, `WorkflowEngine`, trigger handlers and execution lifecycle;
- REST/webhook handlers on narrow `WorkflowHttpRuntime` with an explicit DB handle, built by module-owned Axum routers from `HostRuntimeContext`;
- module-owned admin native server-function transport on `rustok_api::HostRuntimeContext`, with GraphQL kept as the parallel selected path;
- workflow storage: definitions, versions, steps, executions and step executions;
- transport surfaces: GraphQL, REST/webhook ingress and module-owned admin UI package;
- step taxonomy (`action`, `emit_event`, `condition`, `delay`, `http`, `alloy_script`, `notify`);
- tenant isolation, RBAC and execution audit for the workflow domain.

## Integration

- uses platform `EventBus` / `EventTransport` contracts from the foundation layer and does not own transport delivery;
- may use `alloy` as a capability for individual workflow steps without a hard registry dependency;
- `apps/server` for workflow remains a composition root, not the owner of transport business logic;
- event-driven trigger handling is published through `WorkflowModule::register_event_listeners(...)`, and `WorkflowCronScheduler` remains a separate host background runtime and is not considered an `event_listener`;
- workflow-generated events are published through the outbox path, not through a separate internal loop.
- event-triggered execution is idempotent by `(workflow_id, trigger_event_id)`: repeated
  delivery, including delivery after a handler restart, returns the existing
  execution and does not re-run steps; the guarantee is anchored by a unique index in the database.

## Verification

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- `node scripts/verify/verify-workflow-admin-boundary.mjs`
- targeted tests for trigger matching, step execution, tenant isolation and transport/UI contracts

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Event flow contract](../../../../docs/architecture/event-flow-contract.md)
