# rustok-workflow / CRATE_API

## Lifecycle and admission guards

- Cron syntax is validated with the runtime parser when a Workflow enters
  `active`, when an active cron is modified, and when an active workflow
  version is restored. Draft configurations may be incomplete.
- A destructive `WorkflowService::delete` rejects running/uncertain
  executions or pending/running/unresolved cron firings with
  `WorkflowDeletionBlocked` (`409` via REST). This protects active recovery
  evidence against parent cascade. Completed history remains subject to the
  existing explicit workflow deletion/retention contract.

## Step execution contract

- An explicit `timeout_ms` bounds **each** step execution attempt to 1–3,600,000 ms (one hour). `None` means no engine-level deadline. Owner service validates new/updated/restored step configurations, and the engine applies the bound to first and retry attempts.
- Pure steps have retry/skip/stop semantics; `stop_with` must stop the chain even when returned after a successful retry. Typed `StepTimedOut` produces an overall `timed_out` terminal state when a pure step fails without recovery.
- Potentially external HTTP/notify/script/event steps fail closed on any executor error, including timeout or a misleading validation error. The individual step and parent execution use `reconciliation_required`; a generic retry/skip policy cannot establish whether a remote effect was already accepted.
- Unknown executor types with `on_error=skip` record the step as `skipped`, not `failed`.

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
