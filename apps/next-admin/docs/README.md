# Next Admin Docs

## Назначение

Локальная документация для `apps/next-admin`.

## Состав

- [Implementation Plan](./implementation-plan.md)
- [Navigation RBAC](./nav-rbac.md)
- [Clerk setup](./clerk_setup.md)
- [Themes](./themes.md)

## Current runtime contract

- `apps/next-admin` использует canonical FSD-слои `app`, `shared`, `entities`, `features`, `widgets`.
- Shared UI contract идёт через [`UI/docs/api-contracts.md`](../../../UI/docs/api-contracts.md) и `@iu/*` wrappers из `UI/next/components`.
- Backend integration идёт через `apps/server` и внутренние transport packages, а не через локальные ad-hoc clients.
- Глобальный поиск по админке встроен в `widgets/command-palette`: KBar использует `rustok-search` query `adminGlobalSearch` для quick-open результатов и hand-off в полный search control plane.
- Legacy import paths допустимы только как временный compatibility слой; новый код должен идти через canonical FSD paths.

Открытые доработки и остаточный scope ведутся только в [`implementation-plan.md`](./implementation-plan.md).

## Связанные документы

- [Next Admin README](../README.md)
- [Documentation Map](../../../docs/index.md)
