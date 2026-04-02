# RusToK Admin (Leptos)

`apps/admin` — Leptos admin RusToK с `csr`, `ssr` и `hydrate` профилями, развиваемая параллельно с `apps/next-admin` для функционального паритета.

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

### Ядро

- `leptos`, `leptos_router` — UI, маршрутизация и dual runtime (`csr` / `ssr` / `hydrate`);
- `tailwindcss` + shadcn token model;

### i18n

- `leptos_i18n` 0.6 — compile-time многоязычность для `csr` / `ssr` / `hydrate` путей через `t_string!()` / `t!()` макросы;
- `leptos_i18n_build` — кодогенерация i18n-модуля из `locales/*.json` через `build.rs`;
- файлы локалей: `locales/en.json`, `locales/ru.json` (вложенный JSON, ~260 ключей).

### Данные и API

- `#[server]` server functions — основной внутренний data-layer для Leptos host/UI;
- `leptos-graphql` — параллельный GraphQL transport/контракты, который не удаляется и остаётся fallback/внешним контрактом;
- `leptos-auth` — auth/session контракты;
- `leptos_query` — кэширование/prefetch запросов.

### Формы и состояние

- `leptos-hook-form`, `leptos-zod` — формы/валидация;
- `leptos-zustand` — управление состоянием;
- `leptos-struct-table`, `leptos-shadcn-pagination` — таблицы/пагинация;
- `leptos-chartistry` — графики на дашборде.

## Взаимодействие

- `apps/server` (`/api/graphql` и `/api/fn/*`) как backend для dual-path data access
- `crates/rustok-rbac` и другие доменные модули через backend
- общий UI контракт с `apps/next-admin` и storefront приложениями
- manifest-driven Leptos module UI через `apps/admin/build.rs`, registry и generic route `/modules/:module_slug`

## Документация

- Локальная: `apps/admin/docs/README.md`
- Платформенная: `docs/UI/README.md`, `docs/UI/rust-ui-component-catalog.md`, `docs/index.md`
