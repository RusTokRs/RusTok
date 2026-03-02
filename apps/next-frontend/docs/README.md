# Документация apps/next-frontend

`apps/next-frontend` — Next.js storefront в параллельной схеме фронтендов RusToK.

## Цель

Приложение повторяет ключевые архитектурные принципы админок:

- FSD-ориентированная структура слоёв (`app`, `modules`, `shared`);
- единый UI-контракт через internal UI workspace (`UI/next`);
- паритет сетевых и auth-контрактов через самописные пакеты `leptos-*`.

## Что перенесено из подхода админок

### 1) Общие frontend-библиотеки

В storefront подключены те же внутренние пакеты, что используются для паритета в админках:

- `leptos-graphql/next` — единый GraphQL контракт (`/api/graphql`, `Authorization`, `X-Tenant-Slug`);
- `leptos-auth/next` — единый формат клиентской auth-сессии и типизация ошибок;
- `leptos-hook-form`, `leptos-zod`, `leptos-zustand` — слой расширения для форм/валидации/состояния.

Для прикладного кода витрины создана FSD-обёртка в `src/shared/lib/`:

- `src/shared/lib/graphql.ts` — `storefrontGraphql(...)` + реэкспорт базовых GraphQL-типов и констант;
- `src/shared/lib/auth.ts` — реэкспорт auth-типов/хелперов (`getClientAuth`, `mapAuthError`, ключи cookie/token).

### 2) FSD-выравнивание

В `tsconfig.json` добавлен alias `@/shared/*` для явного доступа к shared-слою.

Это позволяет переносить в storefront те же паттерны, которые уже применены в next-admin:

- бизнес-логика интеграций — в `shared/lib/*`;
- UI-агрегация — через `modules/*`;
- маршрутизация и сборка экранов — в `app/*`.

### 3) UI-паритет

Для визуальных компонентов продолжается использование shadcn/tailwind-токенов и контрактов,
согласованных с `UI/docs/api-contracts.md`.

### 4) Уточнение package-контрактов

Внутренние npm-пакеты `packages/leptos-*` приведены к явным package manifests (`package.json` с export `./next`).
Это делает зависимости в `apps/next-frontend/package.json` прозрачными и воспроизводимыми для `npm install`.

## Практические правила для дальнейшего переноса

1. Не писать новый ad-hoc fetch-клиент — использовать `@/shared/lib/graphql`.
2. Не дублировать клиентскую auth-логику — использовать `@/shared/lib/auth`.
3. Новые кросс-приложенческие UI-элементы сначала добавлять в `UI/next/components`, затем подключать в витрине.
4. Для изменений UI-контрактов синхронно обновлять:
   - `UI/docs/api-contracts.md`;
   - `docs/UI/storefront.md`;
   - эту страницу.

## Связанные документы

- `/docs/UI/storefront.md`
- `/docs/UI/fsd-restructuring-plan.md`
- `/docs/index.md`
