# RusToK Leptos Storefront

`apps/storefront` — Leptos SSR витрина RusToK (Rust-first вариант storefront).

## Роль в платформе

- SSR storefront для витринных сценариев;
- параллельная реализация к `apps/next-frontend` для технологического паритета;
- проверка Rust UI/SSR пайплайна в единой платформе.

## Архитектурный контур

- entrypoint: `src/main.rs`
- модульные расширения витрины: `src/modules/*` (registry/slots)
- стили: Tailwind + статическая сборка `static/app.css`

## Соглашения об именовании (Naming Conventions)

В проекте приняты следующие соглашения для обеспечения чистоты кода и соблюдения стандартов Rust:

- **Компоненты (функции)**: Все Leptos-компоненты именуются в `snake_case` (например, `storefront_shell`, `product_card`). Использование `PascalCase` для функций-компонентов не рекомендуется.
- **Shared UI**: Общие UI-компоненты в `shared/ui/` (если появятся) имеют префикс `ui_` (например, `ui_button`).
- **Модули**: Компоненты в `src/modules/` также используют `snake_case`.

## Библиотеки

- `leptos`, `leptos_router` — UI и SSR маршрутизация
- `axum`, `tokio` — HTTP сервер
- внутренние crates: `leptos-auth`, `leptos-graphql`, `leptos-table`, `leptos-hook-form`, `leptos-zod`, `leptos-zustand`
- util crates: `leptos-shadcn-pagination`, `leptos_i18n`, `leptos-next-metadata`, `leptos_query`

## Взаимодействие

- `apps/server` (API)
- доменные `crates/rustok-*` через backend
- общий UI-контур с `apps/admin` / `apps/next-frontend`

## Документация

- Платформенный контекст: `docs/UI/storefront.md`
- Общая карта: `docs/index.md`
