# RusToK Next Admin

`apps/next-admin` — Next.js админка RusToK в headless-режиме, экспериментальная альтернатива `apps/admin` (Leptos).

> [!IMPORTANT]
> **Next.js не участвует в авто-деплое модулей.** При установке/удалении модуля через marketplace приложение пересобирается **вручную**.
> Авто-деплой (BuildExecutor) работает только для примарного Leptos-стека (`apps/admin`, `apps/storefront`).

## Роль в платформе

- headless-альтернатива для JS-разработчиков, предпочитающих React-экосистему;
- реализация Next.js App Router варианта админки;
- референс для FSD-структуры в React/Next фронтендах RusToK.

> **Примарный стек** для авто-деплоя модулей: `apps/admin` (Leptos). Сей Next.js-вариант — экспериментальный, управляется независимо.

## FSD ориентир

Текущее направление структуры:

- `src/app/*` — app-layer (роутинг, layouts, route-level orchestration);
- `packages/*` — **модульные UI-пакеты** (бизнес-сценарии модулей @rustok/*-admin);
- `src/widgets/*` / `src/components/*` — композиционные UI-блоки приложения;
- `src/shared/*` — общие утилиты, API-клиенты, типы, UI primitives.

> Приоритет: модульный код живет в `packages/`, интеграционный код приложения — в `src/app` и `src/shared`.

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

## Управление модулями

Установка нового модуля через marketplace автоматически пересобирает только Leptos-стек (`apps/admin`, `apps/storefront`).
Для `apps/next-admin` требуется ручная интеграция:

1. Добавить UI-модуль как npm-пакет в `apps/next-admin/packages/<slug>/`.
2. Пакет должен иметь имя `@rustok/<slug>-admin`.
3. Убедиться, что корневой `package.json` явно зависит от этого пакета (`file:` или `workspace:*`, в зависимости от текущей настройки host).
4. Запустить `npm install && npm run build`.

Текущий capability-пример этого паттерна:

- `apps/next-admin/packages/rustok-ai/` → `@rustok/ai-admin`
- host routes `/dashboard/ai` и `/dashboard/ai/diagnostics` только монтируют пакет и не забирают AI business UI в `src/app`

## Документация

- Локальные docs: `apps/next-admin/docs/*`
- Платформенные UI docs: `docs/UI/*`
- Карта документации: `docs/index.md`
- ADR: [`DECISIONS/2026-03-17-dual-ui-strategy-next-batteries-included.md`](../../DECISIONS/2026-03-17-dual-ui-strategy-next-batteries-included.md)
