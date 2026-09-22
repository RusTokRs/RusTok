# ADR: Platform contracts for agent workflow configuration and scheduling

- Date: 2026-07-13
- Status: Proposed

## Context

`rustok-ai` persists tenant-scoped agent principals, model assignments, and
workflow stages. It can validate an agent principal against a known descriptor
and execute a claimed stage through the canonical AI task-run path. Two
platform capabilities are still required for production operation:

1. operator forms need a tenant-scoped catalog of existing RBAC roles and
   permission descriptors; and
2. a durable scheduler host needs to discover tenants, acquire module work,
   and provide a trusted initiating-subject context to a capability worker.

Neither concern belongs in an AI module, Alloy, or `apps/server`-specific
code. Letting the AI admin form accept raw permission strings would create a
second RBAC vocabulary. Letting the AI crate create an unscoped background task
would lose tenant lifecycle, trusted identity, and host ownership guarantees.

## Decision

The platform owner will publish two generic foundation contracts.

`TenantRbacCatalog` must offer tenant-scoped, read-only role and permission
descriptors, including stable slug, display metadata, and assignment validity.
It lives in the RBAC/API foundation, not in an AI package. Consumers may select
only records returned by the catalog; they must not submit arbitrary role or
permission strings.

`ModuleWorkScheduler` must allow a capability to register a typed worker under
the existing generic runtime-extension mechanism. Each invocation receives a
tenant id, durable lease context, cancellation signal, and a trusted initiating
subject resolver. The host runs the generic scheduler; it does not import AI
stage types, provider types, or Alloy operations.

`rustok-ai` will adapt these generic contracts to `AgentPrincipal` and
`execute_agent_workflow_stage`. It remains responsible for descriptor checks,
agent/initiator permission intersection, model assignment validation, and
workflow state transitions.

## Consequences

- Leptos and Next agent forms can use role/permission dropdowns with native and
  GraphQL parity once `TenantRbacCatalog` is available.
- The scheduler can execute AI stages without an `apps/server` AI import or
  hidden host-owned runtime construction.
- No raw permission fallback, package-local role catalog, or AI-specific host
  worker is permitted.
