# admin docs

В этой папке хранится документация модуля `apps/admin`.

## Зафиксированный стек интеграции

- UI/state: `leptos`, `leptos_router`, `Resource`/actions.
- GraphQL transport: внутренний crate `crates/leptos-graphql` (тонкий слой).
- HTTP: `reqwest`.
- Typed GraphQL (опционально): `graphql-client` на уровне приложений.
- CSS/стили: Tailwind CSS v3 (стандартный CLI) + shadcn-совместимые CSS-переменные.

Цель: использовать battle-tested библиотеки и минимальный внутренний glue-код.

### Соглашения об именовании

Для соблюдения стандартов Rust и обеспечения чистоты кода, все компоненты Leptos в `apps/admin` именуются в `snake_case`. Общие UI-компоненты (`shared/ui/`) имеют префикс `ui_` (например, `ui_button`, `ui_input`).

## UI и стилизация

`apps/admin` использует **shadcn/ui порт-классов** подход:

- Tailwind CSS v3 собирается через `npx tailwindcss` (стандартный CLI, не `tailwind-rs`).
- `apps/admin/tailwind.config.js` определяет цвета через shadcn CSS-переменные (`background`, `primary`, `destructive`, `border`, `sidebar-*` и т.д.).
- `apps/admin/input.css` — CSS entry point: импортирует `UI/tokens/base.css` и определяет полный shadcn-совместимый набор CSS custom properties для light и dark режимов.
- Компоненты из `UI/leptos/src/` (через `crates/leptos-ui`) реализованы как прямой порт Tailwind-классов из shadcn/ui — визуально идентичны компонентам `apps/next-admin`.

### Принципы стилизации

1. Никаких хардкодированных цветовых классов (`bg-slate-*`, `bg-white`, `text-gray-*`).
2. Только семантические CSS-переменные через Tailwind-утилиты (`bg-card`, `text-foreground`, `border-border`, `text-muted-foreground`, `bg-primary`, `text-destructive` и т.д.).
3. Dark mode — через `.dark` класс на root элементе (class strategy).
4. Для компонентов sidebar — отдельный набор `sidebar-*` переменных.

### Сборка CSS

```toml
# Trunk.toml — автоматически вызывается при trunk build/watch
[[hooks]]
stage = "build"
command = "npx"
command_arguments = ["tailwindcss", "-i", "input.css", "-o", "dist/output.css", "--minify"]
```

## Текущее состояние improvement plan

В рамках `docs/admin-review-improvement-plan.md` уже реализованы ключевые элементы **Phase 1-3**:

- Auth session хранит `refresh_token` и `expires_at`, `AuthProvider` выполняет периодическую проверку и обновление токена.
- Dashboard получает `dashboardStats` через GraphQL `Resource` + `Suspense` fallback.
- Users использует серверную фильтрацию с debounce-поиском.
- FSD-структура полностью реализована: `app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`.
- Tailwind/shadcn миграция завершена: все страницы и компоненты используют семантические CSS-переменные.

Открытые этапы (Phase 4+) остаются в плане как backlog для итеративной доработки.

## Связанные документы

- [`docs/UI/fsd-restructuring-plan.md`](../../../docs/UI/fsd-restructuring-plan.md) — план FSD-реструктуризации
- [`docs/UI/rust-ui-component-catalog.md`](../../../docs/UI/rust-ui-component-catalog.md) — каталог UI-компонентов
- [`UI/leptos/README.md`](../../../UI/leptos/README.md) — документация iu-leptos
- [`UI/docs/api-contracts.md`](../../../UI/docs/api-contracts.md) — API-контракты компонентов

