# rustok-workflow / CRATE_API

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


## Durable cron owner contract (proposed, 2026-11-23)

- Three Workflow-owned tables: `workflow_cron_cursors` (revision + CAS
  watermark), `workflow_cron_firings` (unique scheduled slot, one current
  attempt), `workflow_cron_reconciliation_decisions` (immutable human
  decision/evidence).
- `WorkflowCronScheduler::validate_persistence` rejects missing migration.
  `start_with_stop` claims and supervises bounded due work without replaying
  expired attempts merely because their lease aged.
- WorkflowEngine uses firing UUID as the canonical trigger identity. Replays
  of terminal event/cron executions reuse journal identity; incomplete or
  uncertain executions return `ExecutionOutcomeUncertain`.
- For possible external effects, a failure without a proven pre-effect
  rejection records `reconciliation_required`. Generic retry and skip do
  not authorize external re-dispatch.
- Tenant-scoped REST and GraphQL review/decision surfaces share permission
  checks and one owner reconciliation transaction. Requeuing requires
  `prior_worker_fenced` attestation, evidence and absence of an execution
  record; confirmed terminal decisions require terminal journal evidence.
- The decision remains **Proposed**, and no compilation, tests or migration
  deployment evidence has been obtained.

## Cron lifecycle ownership (2026-11-23)

`WorkflowCronScheduler::start_with_stop` binds the Workflow owner's cron
scheduler to a host-provided watch stop receiver. The scheduler tracks its
spawned execution tasks, refuses new triggers on stop (including after async
step loading), and joins accepted executions on cooperative shutdown. A forced
host abort or process crash still interrupts those tasks and is **not**
reconciled as a durable execution completion. The legacy `start` method also
retains child-task ownership. No cross-replica or missed-firing guarantees are
added by this local lifecycle fix.

## Public surface

### Public modules

- `controllers`, `dto`, `entities`, `error`, `graphql`, `migration`, `migrations`, `ports`, `services`, `steps`, `templates`, `module`

## Platform Capability Bindings

- **Core Module Registration**: Exposes `WorkflowModule` implementing `RusToKModule` (slug `"workflow"`) and `MigrationSource`.
- **Workflow Services and Engines**:
  - `WorkflowService`: CRUD and orchestration use-cases for visual workflows and execution runs.
  - `WorkflowEngine`: Linear step execution engine and step registry runtime dispatch.
  - `WorkflowTriggerHandler`: Event-driven trigger listener subscribing to domain events.
  - `WorkflowCronScheduler`: Schedule-driven trigger runner.
- **Workflow Steps and Integrations**:
  - `AlloyScriptStep` / `ScriptRunner`: Sandboxed scripting step runner.
  - `NotifyStep` / `NotificationSender`: Notification dispatch step.
  - `BUILTIN_TEMPLATES`: Curated reusable workflow templates.

## Domain Collaborations

- **`rustok-events` / `rustok-outbox`**: Trigger source for event-driven workflows and destination for emitted events.
- **`rustok-notifications`**: Target domain for notification steps.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateWorkflowInput`, `UpdateWorkflowInput`, `WorkflowFilter`: Workflow authoring and query inputs.
- `CreateWorkflowStepInput`, `UpdateWorkflowStepInput`: Step configuration inputs.
- `ExecuteWorkflowInput`: Manual and webhook workflow trigger payloads.

### Domain Invariants

- All write/read operations are tenant-scoped.
- Workflow execution path is strictly separated from the authoring path.
- Step execution goes through a typed step runtime (`WorkflowStep` contract).
- Trigger paths (manual/webhook/event/cron) converge into a single execution orchestration.
- Execution and configuration errors must be returned as typed domain errors without silent fallback.

### Events / Outbox Side Effects

- Workflows may emit domain events through `emit_event` steps via `TransactionalEventBus`.
- Workflow lifecycle events (started, completed, failed) publish execution telemetry.

### Errors / Failure Codes

- `WorkflowError` variants: `Validation`, `NotFound`, `Execution`, `Database`, `StepFailed`, `Internal`.
- All errors preserve structured context and map cleanly to GraphQL/REST error representations.

## Execution Model

- `WorkflowService` is responsible for CRUD and orchestration use-cases.
- `WorkflowEngine` executes steps and manages step registry/runtime dispatch.
- `WorkflowTriggerHandler` integrates the event-driven trigger path.
- `WorkflowCronScheduler` covers the schedule-driven trigger path.

## Extensibility Contract

Custom steps are connected via the `WorkflowStep` trait and registration in the engine runtime (`with_step(...)`).
Requirements for steps:
- Deterministic behavior given the same input context;
- Correct error handling via `WorkflowResult`;
- No tenant-boundary bypass;
- No hidden side effects outside the step contract.
