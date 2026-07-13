# ADR: Agent principals and owner-owned workflows

- Date: 2026-07-13
- Status: Accepted

## Context

AI workloads need to act on the same domain operations as human users, while
remaining constrained by tenancy, RBAC, tool policy, and approvals. A provider
model is an execution engine, not an authorization subject or a product
responsibility. Code-oriented workflows also need domain knowledge that the
generic AI runtime must not own.

## Decision

`rustok-ai` owns generic `AgentPrincipal`, descriptor catalog, workflow stage,
and permission-intersection contracts. Every run has an initiating subject and
an agent principal. The effective permissions are the intersection of both
principals and must satisfy the descriptor's required permissions. An agent
cannot elevate its initiator.

Owner adapters publish their descriptors without importing the generic runtime.
`rustok-ai-alloy` owns code-agent descriptors, allowed Alloy operations, and
the `alloy_change_review` workflow. `rustok-ai` maps those owner descriptors
into its generic catalog. The mapping direction prevents a dependency cycle and
keeps `alloy` free of model, provider, orchestration, and secret concerns.

The initial Alloy workflow has planner, implementer, reviewer, and verifier
stages. It is a declarative policy contract only: it does not introduce an
unreviewed patch-apply operation or a bypass around Alloy's allowed operations.

## Consequences

- Product modules can publish their own domain-agent descriptors through the
  same owner-to-runtime mapping pattern.
- Model/provider selection is attached to an agent invocation by the generic
  runtime and can vary per stage without changing the agent's RBAC identity.
- Persisted agent principals, model assignments, workflow runs, and public
  catalog UI are follow-up work; this ADR does not grant a host application
  ownership of any AI surface.
- `apps/server` remains a platform composition concern and receives no
  AI-specific imports or module-owned construction.
