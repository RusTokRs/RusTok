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
| `rustok-channel` | [docs](../../crates/rustok-channel/docs/README.md) | [plan](../../crates/rustok-channel/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/rustok-index/docs/README.md) | [plan](../../crates/rustok-index/docs/implementation-plan.md) |
| `rustok-search` | [docs](../../crates/rustok-search/docs/README.md) | [plan](../../crates/rustok-search/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/rustok-outbox/docs/README.md) | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) |
| `rustok-test-utils` | [docs](../../crates/rustok-test-utils/docs/README.md) | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) |
| `rustok-telemetry` | [docs](../../crates/rustok-telemetry/docs/README.md) | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/rustok-tenant/docs/README.md) | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/rustok-rbac/docs/README.md) | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) |
| `rustok-iggy` | [docs](../../crates/rustok-iggy/docs/README.md) | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) |
| `rustok-iggy-connector` | [docs](../../crates/rustok-iggy-connector/docs/README.md) | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) |
| `rustok-mcp` | [docs](../../crates/rustok-mcp/docs/README.md) | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) |
| `rustok-ai` | [docs](../../crates/rustok-ai/docs/README.md) | [plan](../../crates/rustok-ai/docs/implementation-plan.md) |
| `flex` | [docs](../../crates/flex/docs/README.md) | n/a (Phase 4.5 extraction) |

Core module UI packages:
- `rustok-channel` admin UI: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index` admin UI: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox` admin UI: [README](../../crates/rustok-outbox/admin/README.md)
- `rustok-tenant` admin UI: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac` admin UI: [README](../../crates/rustok-rbac/admin/README.md)

Capability UI packages:
- `rustok-ai` Leptos admin UI: [README](../../crates/rustok-ai/admin/README.md)
- `rustok-ai` Next.js admin UI: `apps/next-admin/packages/rustok-ai/`

Optional module UI packages:
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)

Notes:
- `rustok-content` remains a shared helper/orchestration boundary and does not publish a dedicated operator-facing UI.
- Commerce split crates stay under aggregate `rustok-commerce-admin` in the current wave and do not publish separate admin packages.

## Domain modules

| Module | Docs | Implementation plan |
|--------|------|-------------------|
| `rustok-content` | [docs](../../crates/rustok-content/docs/README.md) | [plan](../../crates/rustok-content/docs/implementation-plan.md) |
| `rustok-cart` | [docs](../../crates/rustok-cart/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-customer` | [docs](../../crates/rustok-customer/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-product` | [docs](../../crates/rustok-product/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-profiles` | [docs](../../crates/rustok-profiles/docs/README.md) | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) |
| `rustok-region` | [docs](../../crates/rustok-region/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-pricing` | [docs](../../crates/rustok-pricing/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-inventory` | [docs](../../crates/rustok-inventory/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-order` | [docs](../../crates/rustok-order/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-payment` | [docs](../../crates/rustok-payment/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-fulfillment` | [docs](../../crates/rustok-fulfillment/docs/README.md) | via [commerce split plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-commerce` | [docs](../../crates/rustok-commerce/docs/README.md) | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-blog` | [docs](../../crates/rustok-blog/docs/README.md) | [plan](../../crates/rustok-blog/docs/implementation-plan.md) |
| `rustok-comments` | [docs](../../crates/rustok-comments/docs/README.md) | [plan](../../crates/rustok-comments/docs/implementation-plan.md) |
| `rustok-forum` | [docs](../../crates/rustok-forum/docs/README.md) | [plan](../../crates/rustok-forum/docs/implementation-plan.md) |
| `rustok-pages` | [docs](../../crates/rustok-pages/docs/README.md) | [plan](../../crates/rustok-pages/docs/implementation-plan.md) |
| `rustok-taxonomy` | [docs](../../crates/rustok-taxonomy/docs/README.md) | [plan](../../crates/rustok-taxonomy/docs/implementation-plan.md) |
| `rustok-media` | [docs](../../crates/rustok-media/docs/README.md) | [plan](../../crates/rustok-media/docs/implementation-plan.md) |
| `rustok-storage` | [docs](../../crates/rustok-storage/docs/README.md) | [plan](../../crates/rustok-storage/docs/implementation-plan.md) |
| `rustok-workflow` | [docs](../../crates/rustok-workflow/docs/README.md) | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) |

## Capabilities

| Capability | Docs | Implementation plan |
|--------|------|-------------------|
| `alloy` | [docs](../../crates/alloy/docs/README.md) | [plan](../../crates/alloy/docs/implementation-plan.md) |
| `rustok-ai` | [docs](../../crates/rustok-ai/docs/README.md) | [plan](../../crates/rustok-ai/docs/implementation-plan.md) |

## Module template

When creating a new module, copy the `_template` folder and fill in all sections.

> [!IMPORTANT]
> Если новый модуль публикует или обрабатывает `DomainEvent`, в его
> `crates/<name>/docs/README.md` обязательно добавляется секция `Event contracts`
> со ссылкой на `docs/architecture/event-flow-contract.md`, а при добавлении
> нового модуля обновляются `docs/index.md` и `docs/modules/registry.md`.


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
- [Rich-text (Tiptap) and GrapesJS Page Builder implementation plan](./tiptap-page-builder-implementation-plan.md) — phased rollout plan with completion marks
- [Flex docs](../../crates/flex/docs/README.md) — Flex module concept


