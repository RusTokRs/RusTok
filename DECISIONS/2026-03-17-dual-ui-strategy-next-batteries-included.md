# UI Strategy: Leptos (Primary) + Next.js (Batteries Included) + Deployment Modes

- Date: 2026-03-17
- Status: Accepted

## Context

RusTok поддерживает два UI стека:
- **Leptos** — primary, компилируется в единый бинарник с Rust backend
- **Next.js** — secondary, для JS-разработчиков, знакомых с React-экосистемой

Изначально Next.js UI для модулей (`@rustok/blog-admin`, `@rustok/workflow-admin`,
`@rustok/blog-frontend`) хранился как отдельные npm-пакеты внутри крейтов
(`crates/rustok-blog/ui/admin/`, `crates/rustok-workflow/ui/admin/`, и т.д.).

## Решение

### 1. Структура модульного крейта — всё в одном

Leptos UI живёт **прямо внутри модульного крейта**, рядом с backend-кодом.
Изоляция через Cargo features — не через отдельные крейты.

```
crates/rustok-blog/
  src/
    lib.rs              ← domain types, DTOs (всегда компилируется)
    services/           ← backend logic   [feature = "server"]
    admin/              ← Leptos admin UI  [feature = "leptos-admin"]
    storefront/         ← Leptos SSR UI    [feature = "leptos-storefront"]
```

```toml
# crates/rustok-blog/Cargo.toml
[features]
server            = ["sea-orm", "tokio", "async-trait"]
leptos-admin      = ["leptos", "leptos-router", "leptos-meta"]
leptos-storefront = ["leptos", "leptos-router", "leptos-meta"]
```

Аналогия: WordPress плагин = одна папка, всё внутри.
Разработчик модуля работает с **одним крейтом**.

### 2. Режимы деплоя

Каждый бинарник собирается с нужным набором features. Конкретная топология
деплоя — решение оператора: monolith, headless, раздельные серверы для API/admin/
нескольких фронтендов, мультитенант, edge — всё это варианты одного принципа.

```bash
cargo build --release --features "server"                              # чистый API
cargo build --release --features "server,leptos-admin"                 # + admin UI
cargo build --release --features "server,leptos-storefront"            # + storefront SSR
cargo build --release --features "server,leptos-admin,leptos-storefront" # всё вместе
```

Marketplace авто-установка работает для любого набора features.

### 3. Next.js — "Batteries Included", ручная сборка

Используется в **headless** режиме как альтернатива Leptos admin/storefront.
Весь Next.js UI живёт прямо в приложениях, без отдельных npm-пакетов:

```
apps/next-admin/src/features/
  ├── blog/       ← blog + forum UI
  └── workflow/   ← workflow UI

apps/next-frontend/src/features/
  └── blog/       ← blog storefront UI
```

Добавить новый модуль в Next.js:
1. Создать `apps/next-admin/src/features/<name>/`
2. Добавить `import '@/features/<name>'` в `src/modules/index.ts`
3. Добавить файловые маршруты в `src/app/dashboard/<name>/`
4. `npm run build`

Авто-установка модулей для Next.js **не предусмотрена** — ручное управление.

## Следствия

### Позитивные

- **Один крейт на модуль** — разработчик не распыляется на 3 репозитория
- **Все режимы деплоя из коробки** — features переключают monolith ↔ headless
- **Чёткое разделение**: Rust/Leptos в `crates/`, Next.js UI в `apps/`
- **WordPress-аналогия работает**: модуль = крейт = плагин

### Негативные

- **Дублирование API-клиентов** между Leptos и Next.js UI
  Митигация: OpenAPI-сгенерированные типы из `packages/rustok-api-client`
- **Большой Cargo.toml** у модулей с UI
  Митигация: workspace features и общие зависимости в workspace Cargo.toml

## Изменения от предыдущей версии этого документа

- Убраны отдельные крейты `crates/leptos-blog-admin/` и `crates/leptos-blog-storefront/`
- Добавлена секция режимов деплоя (monolith / headless / hybrid)
- Уточнено: авто-установка работает для всех режимов, UI-features опциональны
