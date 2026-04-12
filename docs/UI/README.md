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
- Для module-owned admin UI selection state тоже host-owned: typed `snake_case` query keys живут в URL,
  локальный editor/detail state только гидратится из них, а отсутствие валидного key ведёт к empty state.
- Для module-owned Leptos storefront UI query/state plumbing тоже должно идти через общий слой:
  `leptos-ui-routing` переиспользуется и в admin, и в storefront, а прямой package-local доступ
  к `UiRouteContext.query_value(...)` не считается каноническим паттерном.

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
4. Для module-owned admin UI дополнительно обновляйте route-selection contract и parity notes в
   host docs, если меняется query schema, selection behavior или helper layer.
5. Для module-owned storefront UI так же обновляйте routing/query parity notes, если меняется
   reuse слоя `leptos-ui-routing`, host query semantics или storefront route/query contract.
