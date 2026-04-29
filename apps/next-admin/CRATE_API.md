# apps/next-admin / CRATE_API

## Публичные модули

- Next.js admin dashboard (App Router): layout, navigation, RBAC-aware pages, theme/clerk integrations.

## Основные структуры/контракты

- Публичные маршруты `/admin/*`.
- Контракты с backend API/GraphQL.
- Auth provider integration (Clerk).

## События

- Публикует: админские команды в backend через API.
- Потребляет: ответы API и auth session events от Clerk.

## Зависимости от других крейтов/пакетов

- `packages/leptos-*` (TS packages), backend `apps/server` API.

## Частые ошибки ИИ

- Путает источники типов между локальными `types` и generated GraphQL types.
- Ломает RBAC в sidebar/nav при рефакторинге маршрутов.
