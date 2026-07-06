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
- REST/webhook handlers on narrow `WorkflowHttpRuntime` with an explicit DB handle; the current Loco `AppContext` remains only in the route-state adapter until the full Axum cutover;
- workflow storage: definitions, versions, steps, executions and step executions;
- transport surfaces: GraphQL, REST/webhook ingress and module-owned admin UI package;
- step taxonomy (`action`, `emit_event`, `condition`, `delay`, `http`, `alloy_script`, `notify`);
- tenant isolation, RBAC and execution audit for the workflow domain.

## Integration

- uses platform `EventBus` / `EventTransport` contracts from the foundation layer and does not own transport delivery;
- may use `alloy` as a capability for individual workflow steps without a hard registry dependency;
- `apps/server` for workflow remains a composition root / shim layer, not the owner of transport business logic;
- event-driven trigger handling is published through `WorkflowModule::register_event_listeners(...)`, and `WorkflowCronScheduler` remains a separate host background runtime and is not considered an `event_listener`;
- workflow-generated events are published through the outbox path, not through a separate internal loop.
- event-triggered execution is idempotent by `(workflow_id, trigger_event_id)`: repeated
  delivery, including delivery after a handler restart, returns the existing
  execution and does not re-run steps; the guarantee is anchored by a unique index in the database.

## Verification

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- targeted tests for trigger matching, step execution, tenant isolation and transport/UI contracts

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
