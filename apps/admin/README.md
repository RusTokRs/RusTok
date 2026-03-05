# RusToK Admin (Leptos)

`apps/admin` — Leptos CSR админка RusToK, развиваемая параллельно с `apps/next-admin` для функционального паритета.

## Роль в платформе

- управление контентом, пользователями, безопасностью и настройками;
- Rust/Leptos реализация admin-панели;
- эталон для внутреннего UI и crate-контрактов в Rust фронтенде.

## FSD структура

Приложение следует FSD-слоям:

- `shared/` — инфраструктурные утилиты, API, UI primitives.

## Соглашения об именовании (Naming Conventions)

В проекте приняты следующие соглашения для обеспечения чистоты кода и соблюдения стандартов Rust:

- **Компоненты (функции)**: Все Leptos-компоненты именуются в `snake_case` (например, `dashboard`, `user_details`). Использование `PascalCase` для функций-компонентов не рекомендуется.
- **Shared UI**: Общие UI-компоненты в `shared/ui/` имеют префикс `ui_` для предотвращения конфликтов со стандартными HTML-тегами (например, `ui_button`, `ui_input`).
- **Бизнес-компоненты**: Компоненты в слоях `features/` и `widgets/` именуются описательно в `snake_case` (например, `modules_list`, `stats_card`).

## Библиотеки и контракты

- `leptos`, `leptos_router` — UI и маршрутизация;
- `tailwindcss` + shadcn token model;
- `leptos-graphql` — GraphQL transport/контракты;
- `leptos-auth` — auth/session контракты;
- `leptos-hook-form`, `leptos-zod`, `leptos-table`, `leptos-zustand`, `leptos-shadcn-pagination` — формы/валидация/таблицы/состояние.

## Взаимодействие

- `apps/server` (HTTP/GraphQL API)
- `crates/rustok-rbac` и другие доменные модули через backend
- общий UI контракт с `apps/next-admin` и storefront приложениями

## Документация

- Локальная: `apps/admin/docs/README.md`
- Платформенная: `docs/UI/fsd-restructuring-plan.md`, `docs/UI/rust-ui-component-catalog.md`, `docs/index.md`
