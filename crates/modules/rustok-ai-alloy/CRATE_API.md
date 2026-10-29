# rustok-ai-alloy / CRATE_API

## Public surface

### Constants & Enums
- `ALLOY_CODE_TASK_SLUG`: task identifier for Alloy code orchestration.
- `ALLOY_CODE_TOOL_NAME`: tool binding identifier for script execution.
- `AlloyOperation`: typed operations (`ListScripts`, `GetScript`, `ValidateScript`, `RunScript`).

### Descriptors & Configuration
- `AlloyCodeAgentDescriptor`: definition of Alloy code-agent roles (`planner`, `implementer`, `reviewer`, `verifier`).
- `AlloySwarmStageDescriptor`: stage dependencies and execution parameters.
- `AlloySwarmWorkflowDescriptor`: multi-stage code development workflow specification.

### Policies & Registrations
- `register_alloy_ai_vertical_handlers`: registers vertical handlers into the AI runtime.
- `alloy_code_agents`: returns available code agent descriptors.
- `alloy_swarm_workflows`: returns registered swarm workflows.
- `alloy_stage_execution`: binds stage execution parameters to operations.
- `alloy_script_execution_policy`: validates permissions and execution boundaries.
- `validate_runtime_payload`: validates payload structure before task dispatch.

## Platform Capability Bindings
- **AI Vertical Handler**: Exposes the `alloy_code` AI domain vertical and script operations.
- **Workflow & Stage Orchestration**: Defines stage dependencies and approval requirements.
- **Domain Decoupling**: Strictly acts as an AI adapter without implementing Alloy storage or transports directly.

## Domain Collaborations
- **`rustok-ai`**: Consumes agent descriptors, workflows, and operations for AI orchestration.
- **`alloy`**: Underpins script storage, validation, and execution engines.

## Minimum Contract Set

### Input DTOs/Commands
- `AlloyOperation`: selected operation enum for script operations.
- `AlloyCodeAgentDescriptor`: agent configuration inputs.
- `AlloySwarmStageDescriptor`: stage definition inputs.

### Domain Invariants
- **Non-Swallowed Errors**: Script validation and execution errors must be reported with specific failure details.
- **Operation Scoping**: Only authorized operations in the typed `AlloyOperation` catalog may be executed.
- **Approval Gating**: Stages flagged with `requires_approval = true` require explicit human review before execution.

### Events / Outbox Side Effects
- Execution requests trigger state machine transitions and audit events through the AI runtime orchestrator.

### Errors / Failure Codes
- Validation failures produce structured JSON parse or schema error messages.
