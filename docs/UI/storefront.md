# Витрина (Leptos SSR + Next.js)

В RusToK витрина реализована в двух вариантах:
- **Leptos SSR** (`apps/storefront`) — Rust-first SSR приложение на Tailwind.
- **Next.js App Router** (`apps/next-frontend`) — React/Next.js реализация витрины.

Обе реализации выравниваются по тем же принципам, что и параллельные админки:

- FSD-границы между `app / modules / shared`;
- единые внутренние frontend-библиотеки (`leptos-*`) для API/Auth/State-контрактов;
- паритет UI-контрактов через `UI/docs/api-contracts.md`.

## Локальный запуск

```bash
# Leptos SSR
cargo run -p rustok-storefront

# Next.js storefront
cd apps/next-frontend
npm install
npm run dev
```

Leptos SSR сервер слушает `http://localhost:3100`. Next.js приложение — `http://localhost:3000`.

## Контракты паритета фронтендов

### Next.js storefront (`apps/next-frontend`)

- GraphQL-контракт подключается через `leptos-graphql/next`.
- Auth/session контракт подключается через `leptos-auth/next`.
- FSD-gateway для интеграций расположен в `src/shared/lib/`:
  - `shared/lib/graphql.ts` (`storefrontGraphql`, re-export констант и типов)
  - `shared/lib/auth.ts` (re-export auth типов и хелперов)

Такой слой устраняет разрозненные ad-hoc реализации fetch/auth и удерживает витрину
в том же контрактном поле, что и админки.

### Leptos storefront (`apps/storefront`)

- Использует workspace-крейты `leptos-auth`, `leptos-graphql`, `leptos-table`.
- Остаётся Rust-first вариантом витрины с тем же backend-контуром доменных модулей.

## Стили Tailwind

Leptos storefront использует Tailwind-only стили. CSS-пайплайн построен на `tailwind-rs`.
Для оффлайн-сборки или кастомной темы:

```bash
cd apps/storefront
npm install
npm run build:css
```

Собранный файл `apps/storefront/static/app.css` раздаётся SSR-сервером по `/assets/app.css`.

## Локализация

Витрина поддерживает English и Russian. Переключение языка через query-параметр `lang`:

- English: `http://localhost:3100?lang=en`
- Russian: `http://localhost:3100?lang=ru`

Для новых языков расширяйте `locale_strings` в `apps/storefront/src/main.rs`.
