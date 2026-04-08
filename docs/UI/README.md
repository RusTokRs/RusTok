# Документация UI

Этот раздел описывает frontend-приложения RusToK и общие правила интеграции UI-поверхностей.

## Ландшафт UI

В платформе поддерживаются четыре host-приложения:

- `apps/admin` — основной Leptos admin host;
- `apps/storefront` — основной Leptos storefront host;
- `apps/next-admin` — параллельный Next.js admin host;
- `apps/next-frontend` — параллельный Next.js storefront host.

Leptos hosts являются основным runtime-путём для platform-owned UI внутри Rust workspace. Next.js hosts идут параллельным headless-путём и должны сохранять parity по transport, auth, i18n и module contracts.

## Базовый UI contract

- Host-приложения композируют UI-поверхности, но не забирают модульный business UI в свой код.
- Если модуль поставляет UI, эта поверхность остаётся module-owned независимо от статуса `Core` или `Optional`.
- Manifest-driven wiring для publishable UI идёт через `modules.toml` и `rustok-module.toml`.
- Leptos hosts обязаны использовать host-provided `UiRouteContext`, включая effective locale и module route base.
- Module-owned UI пакеты не должны вводить собственную locale negotiation цепочку поверх host/runtime contract.

## Transport и runtime contract

- Для Leptos hosts GraphQL и native `#[server]` functions сосуществуют параллельно; добавление `#[server]` не заменяет `/api/graphql`.
- Backend source of truth для UI hosts — `apps/server`.
- Contract parity между Leptos и Next.js оценивается на уровне маршрутов, auth, locale, module wiring и transport surface, а не на уровне буквального совпадения внутренней реализации.

## Разделы документации

- [Контракт storefront](./storefront.md)
- [Архитектура GraphQL](./graphql-architecture.md)
- [Быстрый старт Admin ↔ Server](./admin-server-connection-quickstart.md)
- [Каталог Rust UI-компонентов](./rust-ui-component-catalog.md)
- [Трек rich-text и визуального page builder](../modules/tiptap-page-builder-implementation-plan.md)

## Документация приложений

- [Leptos Admin](../../apps/admin/docs/README.md)
- [Leptos Storefront](../../apps/storefront/docs/README.md)
- [Next.js Admin](../../apps/next-admin/docs/README.md)
- [Next.js Storefront](../../apps/next-frontend/docs/README.md)

## Поддержка актуальности

При изменении frontend-архитектуры, маршрутизации, UI contracts или backend integration:

1. Обновляйте локальные docs в `apps/*`.
2. Обновляйте соответствующий документ в `docs/UI/`.
3. Сверяйте ссылки в [карте документации](../index.md).
