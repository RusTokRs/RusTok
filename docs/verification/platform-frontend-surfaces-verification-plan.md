# План верификации платформы: frontend-поверхности

- **Статус:** Актуализированный детальный чеклист
- **Контур:** Leptos host-приложения, Next.js host-приложения, UI/libraries/packages
- **Companion-план:** [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md)

---

## Фаза 10: Фронтенды — Leptos

### 10.1 `apps/admin`

**Файлы:**
- `apps/admin/src/app/router.rs`
- `apps/admin/src/app/modules/registry.rs`
- `apps/admin/src/pages/`

- [ ] Leptos Admin остаётся host-приложением с module-owned routing через `/modules/:module_slug` и `/modules/:module_slug/*module_path`.
- [ ] Базовые host-страницы соответствуют коду: dashboard, profile, security, modules, users, apps, workflows.
- [ ] Auth и data-layer integration отражают текущий dual-path: native `#[server]` first для Leptos data access, GraphQL параллельно через `leptos-auth` / `leptos-graphql` и fallback-ветки.
- [ ] Admin module registry и module request context описаны корректно.
- [ ] План не обещает встроенные product/content/blog/pages screens там, где в host есть только generic module-owned surfaces.

### 10.2 `apps/storefront`

**Файлы:**
- `apps/storefront/src/app/mod.rs`
- `apps/storefront/src/modules/registry.rs`
- `apps/storefront/src/pages/home/`

- [ ] Leptos Storefront отражён как SSR host shell с module-owned pages и slot-based composition.
- [ ] В плане отражены `StorefrontSlot`, page registry и enabled-modules gating.
- [ ] Home shell, static locale data и module-owned page rendering задокументированы корректно.
- [ ] Data-layer storefront отражает текущий dual-path: native `#[server]` path и сохранённый GraphQL fallback.
- [ ] План не обещает отдельные catalog/blog/cart screens, если в текущем host-коде они ещё не оформлены как самостоятельные маршруты.

---

## Фаза 11: Фронтенды — Next.js

### 11.1 `apps/next-admin`

**Файлы:**
- `apps/next-admin/package.json`
- `apps/next-admin/src/auth.ts`
- `apps/next-admin/src/app/dashboard/`

- [ ] План отражает актуальный стек: Next.js 16, React 19, NextAuth credentials/session flow, App Router.
- [ ] В плане отражены реальные dashboard-разделы: blog, product, modules, users, workflows и другие существующие страницы из `src/app/dashboard/`.
- [ ] Локальные module-owned UI packages (`@rustok/blog-admin`, `@rustok/workflow-admin`) отражены как текущий механизм модульного UI.
- [ ] Документация не утверждает, что текущая auth-модель построена на Clerk, если runtime-код использует `next-auth`.
- [ ] Lint/build/type safety checks соответствуют реальным npm scripts.

### 11.2 `apps/next-frontend`

**Файлы:**
- `apps/next-frontend/package.json`
- `apps/next-frontend/src/app/`
- `apps/next-frontend/src/shared/`

- [ ] План отражает текущий минимальный Next.js storefront shell с `next-intl`, enabled-modules provider и locale route `[locale]`.
- [ ] План не обещает полноценные catalog/blog/product-detail flows, если они ещё не оформлены в `src/app/`.
- [ ] Lint/typecheck/build checks соответствуют реальным npm scripts.

---

## Фаза 12: UI-библиотеки и shared packages

### 12.1 Rust / Leptos библиотеки (`crates/`)

- [ ] `leptos-auth`
- [ ] `leptos-forms`
- [ ] `leptos-graphql`
- [ ] `leptos-hook-form`
- [ ] `leptos-shadcn-pagination`
- [ ] `leptos-table`
- [ ] `leptos-ui`
- [ ] `leptos-zod`
- [ ] `leptos-zustand`

Для каждой:
- [ ] README/docs совпадают с текущим public API.
- [ ] Реальные потребители в `apps/admin` / `apps/storefront` отражены корректно.
- [ ] Нет обходных app-level реализаций там, где библиотека уже должна быть source of truth.

### 12.2 Internal UI workspace

- [ ] `UI/leptos` отражён как текущий shared design-system/runtime workspace.
- [ ] `docs/UI/README.md`, `graphql-architecture.md`, `storefront.md`, `rust-ui-component-catalog.md` не расходятся с кодом.
- [ ] `docs/UI/graphql-architecture.md` и локальные app/crate docs не утверждают GraphQL-only модель там, где код уже работает через `#[server]` + GraphQL parallel path.
- [ ] Если между Leptos и Next.js есть shared design language, это задокументировано честно, без обещаний parity там, где её ещё нет.

### 12.3 TypeScript packages (`packages/`)

- [ ] `packages/leptos-auth`
- [ ] `packages/leptos-graphql`
- [ ] `packages/leptos-hook-form`
- [ ] `packages/leptos-table`
- [ ] `packages/leptos-zod`
- [ ] `packages/leptos-zustand`

Для каждого:
- [ ] package metadata и build/lint/typecheck expectations актуальны.
- [ ] Реальное использование в `apps/next-*` отражено корректно.
- [ ] План не описывает package surfaces, которых в коде ещё нет.
