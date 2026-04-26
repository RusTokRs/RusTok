# rustok-workflow

## Purpose

`rustok-workflow` owns workflow automation and execution history for RusToK.

## Responsibilities

- Provide `WorkflowModule` metadata for the runtime registry.
- Own workflow CRUD, execution engine, schedules, webhooks, and execution history.
- Own workflow GraphQL and REST transport adapters for module-facing APIs.
- Publish the module-owned Leptos admin root page through `crates/rustok-workflow/admin`.
- Publish the typed `workflows:*` and `workflow_executions:*` RBAC surface.

## Interactions

- Depends on `rustok-core` for module contracts, permissions, and shared runtime types.
- Depends on `rustok-api` for shared tenant/auth/request and GraphQL helper contracts.
- Depends on `rustok-tenant` entity contracts for webhook tenant resolution.
- Integrates with Alloy script execution through the `ScriptRunner` abstraction and the
  `alloy_script` step type, without declaring Alloy as a runtime module dependency.
- Exposes its own GraphQL and REST adapters; `apps/server` now acts only as a composition root
  and re-export shim for workflow transport entry points.
- Keeps webhook ingress as a module-owned transport surface via `controllers::webhook_routes`,
  while `WorkflowCronScheduler` remains a separate background runtime path.
- Declares permissions via `rustok-core::Permission`.
- REST and GraphQL adapters enforce permissions from `AuthContext.permissions` before invoking
  workflow services.
- The default platform RBAC role sets grant `workflows:*` and `workflow_executions:*`
  management to `SuperAdmin` and `Admin`; startup superadmin seeding re-syncs those
  role permissions so newly added workflow permissions are available in local debug stacks.

## Entry points

- `WorkflowModule`
- `WorkflowService`
- `WorkflowEngine`
- `WorkflowCronScheduler`
- `WorkflowTriggerHandler`
- `graphql::WorkflowQuery`
- `graphql::WorkflowMutation`
- `controllers::routes`
- `controllers::webhook_routes`
- `admin/WorkflowAdmin` (publishable Leptos admin root page)

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
