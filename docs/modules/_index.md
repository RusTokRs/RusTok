# Module documentation index

Per-module documentation lives inside each crate at `crates/<name>/docs/README.md`.
This page is the navigation index for all module-level docs.

## Navigation rule

Documentation for each module is kept **inside the module's crate** (`crates/<name>/docs/`),
not duplicated in `docs/modules/`. Links below point directly to those locations.

## Core & Infrastructure modules

| Module | Docs | Implementation plan |
|--------|------|-------------------|
| `rustok-core` | [docs](../../crates/rustok-core/docs/README.md) | [plan](../../crates/rustok-core/docs/implementation-plan.md) |
| `rustok-events` | [docs](../../crates/rustok-events/docs/README.md) | [plan](../../crates/rustok-events/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/rustok-outbox/docs/README.md) | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) |
| `rustok-test-utils` | [docs](../../crates/rustok-test-utils/docs/README.md) | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) |
| `rustok-telemetry` | [docs](../../crates/rustok-telemetry/docs/README.md) | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/rustok-tenant/docs/README.md) | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/rustok-rbac/docs/README.md) | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) |
| `rustok-iggy` | [docs](../../crates/rustok-iggy/docs/README.md) | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) |
| `rustok-iggy-connector` | [docs](../../crates/rustok-iggy-connector/docs/README.md) | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) |
| `rustok-mcp` | [docs](../../crates/rustok-mcp/docs/README.md) | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) |

## Domain modules

| Module | Docs | Implementation plan |
|--------|------|-------------------|
| `rustok-content` | [docs](../../crates/rustok-content/docs/README.md) | [plan](../../crates/rustok-content/docs/implementation-plan.md) |
| `rustok-commerce` | [docs](../../crates/rustok-commerce/docs/README.md) | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-blog` | [docs](../../crates/rustok-blog/docs/README.md) | [plan](../../crates/rustok-blog/docs/implementation-plan.md) |
| `rustok-forum` | [docs](../../crates/rustok-forum/docs/README.md) | [plan](../../crates/rustok-forum/docs/implementation-plan.md) |
| `rustok-pages` | [docs](../../crates/rustok-pages/docs/README.md) | [plan](../../crates/rustok-pages/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/rustok-index/docs/README.md) | [plan](../../crates/rustok-index/docs/implementation-plan.md) |
| `alloy-scripting` | [docs](../../crates/alloy-scripting/docs/README.md) | [plan](../../crates/alloy-scripting/docs/implementation-plan.md) |
| `rustok-workflow` | [docs](../../crates/rustok-workflow/docs/README.md) | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) |

## Module template

When creating a new module, copy the `_template` folder and fill in all sections.

> [!IMPORTANT]
> Если новый модуль публикует или обрабатывает `DomainEvent`, в его `crates/<name>/docs/README.md`
> обязательно добавить секцию `Event contracts` со ссылкой на
> `docs/architecture/event-flow-contract.md`, и обновить `docs/index.md`/`docs/modules/registry.md` при добавлении нового модуля.


```
docs/modules/_template/
  _index.md    — entry point with purpose and key flows
  api.md       — GraphQL/REST contracts
  commands.md  — write-side commands
  queries.md   — read-side queries
  events.md    — published domain events
  domain.md    — entity model
  storage.md   — tables and indexes
  testing.md   — test strategy
  workflows.md — key business workflows
```

## Related documents

- [Module overview](./overview.md) — which modules are registered and their kinds
- [Module & application registry](./registry.md) — full component directory with dependencies
- [Module manifest](./manifest.md) — modules.toml format and rebuild lifecycle
- [Tiptap/Page Builder implementation plan](./tiptap-page-builder-implementation-plan.md) — phased rollout plan with completion marks
- [Flex spec](./flex.md) — Flex module concept
