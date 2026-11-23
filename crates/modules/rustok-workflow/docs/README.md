# `rustok-workflow` Documentation

## Active cron admission and deletion safety

Draft Workflow definitions may contain incomplete cron expressions. Activating
a cron, changing an already active cron expression, or restoring an active
definition now validates the expression with the same parser the scheduler
uses (maximum 512 bytes). Invalid active definitions return
`InvalidTriggerConfig` without a partial workflow update.

Workflow deletion now returns `409 workflow_deletion_blocked` while any
running or reconciliation-required execution exists, or when a cron firing is
pending, running or awaiting reconciliation. This prevents a parent-row
cascade from erasing unresolved owner receipts before the side effects are
accounted for. Once those states become terminal, the existing hard-delete
behavior and its historical-data retention implications still apply; no
new automatic history purge is introduced here.

## Workflow step deadlines and retry semantics

The engine now enforces optional `timeout_ms` for every attempt of a step.
Valid supplied values are between 1 ms and 1 hour; the absent value leaves
the executor's own limits in effect. Invalid values are rejected on creation,
update and version restore. For a pure step, an exhausted timeout ends in
`timed_out` (or may be skipped/retried by the declared error policy).
On retry success, `stop_with` ends the chain just as it does on the first
attempt. Unknown skipped steps are represented as `skipped` in receipts.

A timeout or error from an external step does not prove cancellation of remote
side effects; the step and execution are `reconciliation_required` instead of
`failed`. Even an `InvalidStepConfig` returned by a pluggable external step
cannot prove the failure occurred before a remote send. No automatic retry or
skip occurs for these ambiguous effects.

## Durable cron scheduling and recovery (proposed ADR, 2026-11-23)

Workflow owns `workflow_cron_cursors`, `workflow_cron_firings`, and immutable
`workflow_cron_reconciliation_decisions`. The migration is additive and must
be applied before the host starts the cron scheduler; missing schema fails
bootstrap. The cursor and at most 32 due slots advance in one transaction
under a compare-and-set generation, with unique
`(workflow_id, schedule_revision, scheduled_at)` admission. A bounded active
workflow page is scanned on each tick; the revision changes on workflow edits,
including mutations of its step graph. The first cursor is bounded by a 24-hour
initial catch-up horizon; persistent cursors catch up later downtime over
multiple ticks. A queue row is claimed by one replica with a conditional
pending-to-running write, and its UUID becomes the Workflow execution's stable
`trigger_event_id`.

Completed and failed execution receipts are replayed without executing
steps twice. An existing running or `reconciliation_required` receipt is
*never* interpreted as safe to replay. External HTTP/notification/script
errors enter reconciliation rather than performing blind `OnError::Retry`
or `OnError::Skip`. A validation-looking error from a pluggable external
executor does not prove the effect was never sent. The event listener now awaits its execution instead of
acknowledging after a detached spawn.

The tenant-scoped REST GET `/api/workflows/cron/reconciliation` and GraphQL
`workflowCronReconciliation` require `workflow_executions:list`.
The REST POST `/api/workflows/cron/reconciliation/{firing_id}` and GraphQL
`reconcileWorkflowCronFiring` require `workflows:manage` plus a human actor
and explicit, bounded evidence. `confirm_terminal` requires an existing
terminal execution. `requeue_confirmed_unstarted` is allowed only when no
execution journal exists and the operator explicitly attests that the prior
worker is stopped/fenced. The decision and evidence are committed in the same
transaction as the firing status; a stale attempt cannot commit afterward.

This contract does not make arbitrary external providers exactly-once, and
an attestation is not a technical proof that a still-running remote side
effect was cancelled. Review any old `running` execution before retry; do
not reset it based solely on time. The migration and tests have not been run.
Architecture is tracked in [the proposed ADR](../../../../DECISIONS/2026-11-23-workflow-cron-durable-scheduling-and-recovery.md).

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
  delivery of a *terminal* execution returns the existing result without
  re-running steps; an unresolved running or reconciliation-required journal
  fails closed and keeps its durable event source unacknowledged.

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
