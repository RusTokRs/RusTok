# Документация Next Admin

Локальная документация для `apps/next-admin`.

## Состав

- [Implementation Plan](./implementation-plan.md)
- [Navigation RBAC](./nav-rbac.md)
- [Clerk setup](./clerk_setup.md)
- [Themes](./themes.md)

## Текущий runtime contract

- `apps/next-admin` использует canonical FSD-слои `app`, `shared`, `entities`, `features`, `widgets`.
- Shared UI contract идёт через [`UI/docs/api-contracts.md`](../../../UI/docs/api-contracts.md) и `@iu/*` wrappers из `UI/next/components`.
- Backend integration идёт через `apps/server` и внутренние transport packages, а не через локальные ad-hoc clients.
- Глобальный поиск по админке встроен в `widgets/command-palette`: KBar использует `rustok-search` query `adminGlobalSearch` для quick-open результатов и hand-off в полный search control plane.
- Legacy import paths допустимы только как временный compatibility layer; новый код должен идти через canonical FSD paths.

## Правило ownership UI

- Если модуль поставляет admin UI, он остаётся module-owned package рядом с модулем независимо от `Core`/`Optional`.
- `apps/next-admin` выступает host/composition root и не забирает модульный business UI в свой код.
- Core-модули с UI подчиняются тому же правилу, что и optional-модули: host монтирует surface, но не становится владельцем модульного UI.
- Capability-пакеты подчиняются тому же правилу: `rustok-ai` поставляется как `apps/next-admin/packages/rustok-ai` и монтируется host'ом под `/dashboard/ai`, а не реализуется в `apps/next-admin/src/app` как ad-hoc feature.

Открытые доработки и остаточный scope ведутся только в [`implementation-plan.md`](./implementation-plan.md).

## Связанные документы

- [Next Admin README](../README.md)
- [Карта документации](../../../docs/index.md)
