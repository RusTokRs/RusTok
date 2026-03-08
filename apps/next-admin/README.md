# RusToK Next Admin

`apps/next-admin` — основная Next.js админка RusToK, развиваемая параллельно с `apps/admin` (Leptos) с общими UI/API контрактами.

## Роль в платформе

- интерфейс управления контентом, пользователями, каталогом и модулями;
- реализация Next.js App Router варианта админки;
- референс для FSD-структуры в React/Next фронтендах RusToK.

## FSD ориентир

Текущее направление структуры:

- `src/app/*` — app-layer (роутинг, layouts, route-level orchestration);
- `src/features/*` — пользовательские сценарии и feature-блоки;
- `src/widgets/*` / `src/components/*` — композиционные UI-блоки;
- `src/shared/*` — общие утилиты, API-клиенты, типы, UI primitives.

> При рефакторинге приоритет: выносить интеграционный код в `shared`, бизнес-сценарии в `features`, экранную композицию в `app`/`widgets`.

## Соглашения об именовании (Naming Conventions)

В проекте соблюдаются стандартные соглашения React/Next.js для обеспечения чистоты и переносимости кода:

- **Компоненты**: Используется `PascalCase` (например, `Dashboard`, `UserDetails`, `Button`).
- **Shared UI**: Общие компоненты из `shared/ui` или `components/ui` именуются без префиксов согласно традициям shadcn/ui.
- **Файлы**: Имена файлов компонентов также следуют `PascalCase` или `kebab-case` в зависимости от уровня (например, `UserCard.tsx` или `user-card.tsx`).

## Библиотеки и контракты

### Базовый стек

- `next`, `react`, `typescript`
- `tailwindcss` + shadcn/ui (Radix primitives)

### i18n

- `next-intl` 4.0 — многоязычность;
  - серверные компоненты: `getTranslations('namespace')`;
  - клиентские компоненты: `useTranslations('namespace')`;
  - `NextIntlClientProvider` в root layout;
  - определение локали: cookie `rustok-admin-locale` (без URL-роутинга);
- файлы локалей: `messages/en.json`, `messages/ru.json` (вложенный JSON, ~260 ключей).

### Данные и API

- GraphQL: внутренний пакет `leptos-graphql/next` (единые endpoint/header контракты)
- Auth: внутренний пакет `leptos-auth/next`
- Таблицы: `@tanstack/react-table` + внутренние типы/обёртки при необходимости

### Формы и состояние

- `react-hook-form`, `zod`, `zustand`
- внутренние пакеты для паритета контрактов: `leptos-hook-form/next`, `leptos-zod/next`, `leptos-zustand/next`

## Взаимодействие

- `apps/server` (GraphQL/HTTP API)
- доменные модули `crates/rustok-*` через backend
- shared UI workspace `UI/next` для паритета компонентов с другими фронтендами

## Документация

- Локальные docs: `apps/next-admin/docs/*`
- Платформенные UI docs: `docs/UI/*`
- Карта документации: `docs/index.md`

