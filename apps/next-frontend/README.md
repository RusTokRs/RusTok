# RusToK Next Storefront

`apps/next-frontend` — Next.js витрина RusToK, синхронизированная по архитектурным принципам с админками.

## Роль в платформе

- клиентская storefront-витрина (каталог, промо, контентные блоки);
- React/Next.js реализация параллельно с `apps/storefront` (Leptos SSR);
- площадка для переиспользования общих frontend-контрактов (`leptos-*`, UI workspace).

## FSD ориентир

- `src/app/*` — маршруты и композиция страниц;
- `src/modules/*` — подключаемые модульные секции витрины;
- `src/shared/*` — shared-level интеграции и утилиты;
- `src/components/*` — UI primitives/локальные компоненты.

Ключевой принцип: не дублировать transport/auth код по страницам — интеграции держать в `src/shared/lib/*`.

## Соглашения об именовании (Naming Conventions)

В проекте соблюдаются стандартные соглашения React/Next.js для обеспечения чистоты и переносимости кода:

- **Компоненты**: Используется `PascalCase` (например, `StorefrontShell`, `ProductCard`, `Button`).
- **Shared UI**: Общие компоненты из `src/shared/ui` или `src/components/ui` именуются без префиксов согласно традициям shadcn/ui.
- **Файлы**: Имена файлов компонентов также следуют `PascalCase` или `kebab-case` в зависимости от уровня (например, `StorefrontShell.tsx` или `product-card.tsx`).

## Библиотеки и контракты

### Базовый стек

- `next`, `react`, `typescript`
- `tailwindcss` + shadcn/ui
- `next-intl` (локализация)

### Внутренние пакеты (паритет с админками)

- `leptos-graphql/next` — GraphQL endpoint + tenant/auth headers;
- `leptos-auth/next` — клиентский auth/session контракт;
- `leptos-hook-form/next`, `leptos-zod/next`, `leptos-zustand/next`, `leptos-table/next` — shared типы и расширение контрактов.

### FSD gateway в приложении

- `src/shared/lib/graphql.ts` — `storefrontGraphql(...)` и реэкспорт GraphQL контрактов;
- `src/shared/lib/auth.ts` — реэкспорт auth-хелперов/типов.

## Взаимодействие

- `apps/server` (витринный API)
- `crates/rustok-commerce`, `crates/rustok-content` через backend
- `UI/next` и `UI/docs/api-contracts.md` для UI-паритета

## Документация

- Локальная: `apps/next-frontend/docs/README.md`
- Центральная: `docs/UI/storefront.md`, `docs/index.md`
