# rustok-workflow / CRATE_API

## Public Modules

`controllers`, `dto`, `entities`, `error`, `graphql`, `migration`, `migrations`, `ports`, `services`, `steps`, `templates`, `module`.

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
