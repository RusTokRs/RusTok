# @rustok/workflow-admin — Canonical API Specification

## 1. Package Overview

`@rustok/workflow-admin` provides the module-owned Next.js administration interface for the RusToK Workflows platform. It manages event-driven workflow definitions, linear/branching step pipelines, real-time and historical execution monitoring, starter template instantiation, natural language workflow generation, and version rollbacks.

- **Package Name**: `@rustok/workflow-admin`
- **Location**: `apps/next-admin/packages/workflow`
- **Owner**: `rustok-workflow` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Exported Components & Pages

- `WorkflowsPage` (default export & alias `WorkflowsAdminPage`): Server component listing tenant workflows with failure counters, lifecycle statuses, and template gallery.
- `WorkflowDetailPage`: Component rendering complete workflow definition, execution telemetry, and version revision history.
- `WorkflowFormPage`: Form component for creating or editing workflow metadata and triggers.
- `WorkflowStepEditor`: Interactive step reordering, step creation (`ACTION`, `CONDITION`, `DELAY`, etc.), and step deletion.
- `ExecutionHistory`: Table of recent executions with granular step-by-step inputs, outputs, errors, and timing.
- `TemplateGallery`: Pre-built workflow templates that can be instantiated into active workflows.
- `VersionHistory`: Immutable revision history with one-click restore capabilities.

### Navigation

- `workflowNavItems: NavItem[]`: Declares `/dashboard/workflows` and `/dashboard/workflows/new` under the `modulePlugins` navigation group with shortcuts (`w, l` and `w, n`).

### API & Data Fetching Operations

- `WORKFLOWS_QUERY`: Retrieves tenant workflow summaries.
- `WORKFLOW_QUERY`: Retrieves single workflow definition and its ordered steps.
- `WORKFLOW_EXECUTIONS_QUERY`: Retrieves workflow executions with step execution details.
- `CREATE_WORKFLOW_MUTATION`: Mutation creating a new workflow definition.
- `UPDATE_WORKFLOW_MUTATION`: Mutation updating workflow metadata and triggers.
- `DELETE_WORKFLOW_MUTATION`: Mutation deleting a workflow.
- `ACTIVATE_WORKFLOW_MUTATION`: Transitions workflow to `ACTIVE` status.
- `PAUSE_WORKFLOW_MUTATION`: Transitions workflow to `PAUSED` status.
- `TRIGGER_WORKFLOW_MUTATION`: Manually triggers workflow execution with payload.
- `ADD_STEP_MUTATION`: Appends an execution step to a workflow pipeline.
- `UPDATE_STEP_MUTATION`: Updates configuration or error policy of an execution step.
- `DELETE_STEP_MUTATION`: Removes an execution step.
- `WORKFLOW_TEMPLATES_QUERY`: Lists pre-built workflow templates.
- `CREATE_FROM_TEMPLATE_MUTATION`: Clones a template into a tenant workflow.
- `GENERATE_WORKFLOW_MUTATION`: Generates workflow JSON definition via AI description.
- `WORKFLOW_VERSIONS_QUERY`: Lists version snapshots.
- `RESTORE_VERSION_MUTATION`: Restores a workflow to a specific version number.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in the Next.js admin registry via `registerAdminModule({ id: 'workflow', name: 'Workflows', navItems: workflowNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-workflow` async-graphql schema via `graphqlRequest`.

## 4. Invariants & Security

- **Fail-Closed Input Validation**: Non-empty IDs and workflow names are enforced before initiating network calls.
- **Graceful Module Degradation**: When the backend workflow module is disabled for a tenant or unavailable, `WorkflowsPage` gracefully renders `ModuleUnavailable` instead of crashing.
- **Version Integrity**: Rollback mutations validate positive version numbers (`version > 0`).
- **Tenant Isolation**: All operations forward tenant and authorization tokens via request headers.

## 5. Verification & Testing

- **Contract Tests**: `packages/workflow/src/workflow-admin.test.mjs` executed via `npm --prefix apps/next-admin run test:workflow`.
- **Verified Coverage**:
  - `workflowNavItems` routing, sub-items, and shortcuts (`w, l` and `w, n`).
  - GraphQL documents (queries, mutations).
  - Validation guards on `createWorkflow`, `updateWorkflow`, `deleteWorkflow`, `activateWorkflow`, `pauseWorkflow`, `triggerWorkflow`.
  - Validation guards on step management (`addWorkflowStep`, `updateWorkflowStep`, `deleteWorkflowStep`).
  - Validation guards on templates and version rollbacks (`createWorkflowFromTemplate`, `restoreWorkflowVersion`).
