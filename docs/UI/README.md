# Документация UI

Этот раздел описывает frontend-приложения RusToK и общие правила интеграции UI-поверхностей.

## Текущий ландшафт UI

RusToK поддерживает четыре UI-приложения в двух стеках.

### Основной стек: Leptos

- `apps/admin` — основная Leptos-админка (CSR/WASM). Участвует в pipeline пересборки при install/uninstall модулей.
- `apps/storefront` — основная Leptos-витрина (SSR). Участвует в pipeline пересборки при install/uninstall модулей.

Leptos UI-код модулей живёт в publishable sub-crates рядом с модулем: `admin/` и `storefront/`.

### Экспериментальный headless-стек: Next.js

- `apps/next-admin` — альтернативная Next.js-админка в headless-режиме. Пересборка выполняется вручную.
- `apps/next-frontend` — альтернативная Next.js-витрина в headless-режиме. Пересборка выполняется вручную.

Next.js UI-код модулей живёт в виде отдельных npm-пакетов внутри приложений:

- `apps/next-admin/packages/<module>/`
- `apps/next-frontend/packages/<module>/`

См. [ADR: Dual UI Strategy — Next.js modular packages](../../DECISIONS/2026-03-17-dual-ui-strategy-next-batteries-included.md).

## Базовый UI/FSD-контракт

- `apps/admin` и `apps/next-admin` используют canonical FSD-слои: `app`, `shared`, `entities`, `features`, `widgets`, `pages`.
- Source of truth для shared component API находится в [`UI/docs/api-contracts.md`](../../UI/docs/api-contracts.md).
- `UI/leptos` предоставляет Leptos-native реализацию базовых примитивов, а `UI/next/components` — Next.js wrappers с тем же surface.
- shadcn-compatible CSS variables являются canonical theming contract для обеих админок; `UI/tokens/base.css` добавляет только общие font/spacing/radius tokens.

## Правило ownership UI

- Если модуль поставляет UI, этот UI живёт рядом с модулем и остаётся module-owned независимо от того, является модуль `Core` или `Optional`.
- Host-приложения (`apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`) только композируют module surfaces и не забирают модульный business UI в свой код.
- Core-статус модуля влияет на обязательность runtime wiring и доступность поверхности, но не меняет место хранения UI.

## Документы раздела

- [GraphQL Architecture](./graphql-architecture.md) — клиентские GraphQL-conventions.
- [Admin ↔ Server Connection Quickstart](./admin-server-connection-quickstart.md) — подключение UI к backend и env setup.
- [Storefront](./storefront.md) — storefront-specific заметки.
- [Rust UI Component Catalog](./rust-ui-component-catalog.md) — каталог переиспользуемых компонентов и crates.

## Документация приложений

- [Leptos Admin docs](../../apps/admin/docs/README.md)
- [Leptos Storefront README](../../apps/storefront/docs/README.md)
- [Next.js Admin README](../../apps/next-admin/README.md)
- [Next.js Admin RBAC Navigation](../../apps/next-admin/docs/nav-rbac.md)
- [Next.js Admin Clerk setup](../../apps/next-admin/docs/clerk_setup.md)
- [Next.js Admin Theming](../../apps/next-admin/docs/themes.md)
- [Next.js Storefront docs](../../apps/next-frontend/docs/README.md)

## Поддержка актуальности

При изменении frontend-архитектуры, маршрутизации, UI-контрактов или backend integration:

1. Обновляйте локальные docs в `apps/*`.
2. Обновляйте соответствующий документ в `docs/UI/`.
3. Следите, чтобы [`docs/index.md`](../index.md) продолжал ссылаться на актуальные файлы.
