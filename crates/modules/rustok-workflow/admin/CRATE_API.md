# rustok-workflow-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for workflow administration, execution tracing, and templates.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-workflow` for workflow engine orchestration, definitions, and executions.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::leptos::WorkflowAdmin`: Root admin page component for workflow operations.

## Minimum Contract Set

### Input DTOs/Commands
- Workflow execution and template filter parameters.
- Workflow row view-models, execution statuses, and template metadata.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Native server functions consume host-provided `HostRuntimeContext`.
- Status and template presentations are pure and framework-agnostic.

### Events / Outbox Side Effects
- Admin UI adapter package; workflow mutations delegate execution to `rustok-workflow`.

### Errors / Failure Codes
- Transport and execution errors are preserved and surfaced through typed UI signals.
