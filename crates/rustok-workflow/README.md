# rustok-workflow

Visual workflow automation module for the RusToK platform.

## Purpose

`rustok-workflow` provides an n8n/Directus Flows-style orchestrator that integrates with
the platform's event infrastructure. It allows tenants to define automated workflows
triggered by domain events, schedules, webhooks, or manual actions.

## Responsibilities

- Define and store workflows and their step chains.
- Execute workflow step chains in response to triggers.
- Persist execution history and per-step logs.
- Integrate with `alloy-scripting` for arbitrary Rhai script steps.
- Provide CRUD API and admin UI for workflow management.

## Interactions

| Depends on | Purpose |
|-----------|---------|
| `rustok-core` | `RusToKModule`, RBAC permissions, `EventBus`, `EventTransport` |
| `rustok-events` | `DomainEvent`, `EventEnvelope` contracts |
| `rustok-outbox` | Transactional event publishing inside step execution |

## Entry points

- `WorkflowModule` — registers migrations, slug `workflow`, RBAC permissions.
- `WorkflowService` — CRUD for workflows and steps.
- `WorkflowEngine` — executes a workflow step chain, writes execution logs.
- `WorkflowTriggerHandler` — subscribes to `DomainEvent`s and dispatches matching workflows.
- `WorkflowCronScheduler` — polls cron-triggered workflows on a tick.
- `BUILTIN_TEMPLATES` — built-in marketplace templates.

## Step types

| Step | Description |
|------|-------------|
| `action` | Calls a platform service action |
| `emit_event` | Publishes a `DomainEvent` back to the outbox |
| `condition` | Branches on a JSON pointer equality check |
| `delay` | Deferred execution via scheduled event |
| `http` | Outbound HTTP request (webhook) |
| `alloy_script` | Runs a Rhai script via `alloy-scripting` engine |
| `notify` | Sends a notification (email / Slack / Telegram) |

## Trigger types

`event` · `cron` · `webhook` · `manual`

## Admin UI

Next.js UI package: `crates/rustok-workflow/ui/admin`

Provides: workflows list, workflow form with step editor, execution history, template gallery,
version history, and manual trigger button.

## Documentation

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Architecture](../../docs/architecture/workflow.md)
- [CRATE_API](./CRATE_API.md)
