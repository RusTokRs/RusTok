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
- [ ] Module-owned Leptos admin packages описаны как часть live contract, а не как app-local исключения host-кода.
- [ ] Admin module registry и module request context описаны корректно.
- [ ] Leptos admin host прокидывает effective locale в `UiRouteContext`, а module-owned admin packages читают язык UI оттуда вместо собственных несогласованных fallback-цепочек.
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
- [ ] Locale-prefixed маршруты `/{locale}` и `/{locale}/modules/{route_segment}` отражены как текущий SSR contract, а legacy `?lang=` не описывается как primary path.
- [ ] `UiRouteContext` и effective locale прокидываются в module-owned storefront packages без собственных несовместимых fallback-цепочек.
- [ ] Module-owned storefront packages не генерируют hardcoded `/modules/...` ссылки внутри своих view-компонентов; для внутренних generic page links используется `UiRouteContext::module_route_base()`.
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
- [ ] Shared GraphQL transport и policy enforcement описаны на уровне host/package contracts, а не как набор app-specific экранных исключений.
- [ ] Документация не утверждает, что текущая auth-модель построена на Clerk, если runtime-код использует `next-auth`.
- [ ] Lint/build/type safety checks соответствуют реальным npm scripts.

### 11.2 `apps/next-frontend`

**Файлы:**
- `apps/next-frontend/package.json`
- `apps/next-frontend/src/app/`
- `apps/next-frontend/src/shared/`

- [ ] План отражает текущий минимальный Next.js storefront shell с `next-intl`, enabled-modules provider и locale route `[locale]`.
- [ ] План не обещает полноценные catalog/blog/product-detail flows, если они ещё не оформлены в `src/app/`.
- [ ] Module-owned Next/storefront packages потребляют тот же locale contract, что и host, без собственной альтернативной цепочки выбора locale.
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
- [ ] GraphQL hardening для sensitive admin operations описан как server-side AST/root-field policy, а `operationName` не описывается как security boundary.
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

---

## Фаза 13: Native i18n contract

> [!NOTE]
> Эта фаза закрыта как migration wave. Остаточный future scope описывается в live docs, прежде всего в `docs/architecture/i18n.md` и `apps/server/docs/loco-core-integration-plan.md`, и не должен переоткрывать package-owned UI migration как pending работу.

- [x] Server runtime locale chain отражён одинаково в host-приложениях и module-owned UI packages: `query -> x-medusa-locale -> cookie -> Accept-Language(q-values) -> tenant.default_locale -> en`.
- [x] `Content-Language` и effective locale описаны как platform contract для SSR/GraphQL/server-function ответов.
- [x] UI bundles parity проверяется машинно через `npm run verify:i18n:ui`; docs не описывают parity как ручную договорённость без проверяемого артефакта.
- [x] Module-owned translation bundles, если они существуют, описываются через manifest-level `[provides.*_ui.i18n]` contract, а не через расплывчатые WIP-формулировки.
- [x] Locale-aware storefront route contract проверяется отдельно через `npm run verify:storefront:routes`, чтобы module-owned Leptos packages не возвращались к hardcoded `/modules/{route_segment}` вместо `UiRouteContext::module_route_base()`.
- [x] Package-owned Leptos admin bundles (`admin/locales/*.json`) используют тот же host locale contract и не обходят `UiRouteContext.locale` локальными источниками языка, включая capability-owned `rustok-ai-admin`.
- [x] В verification отражено текущее покрытие package-owned bundle contract: Leptos admin surfaces `workflow`, `rbac`, `tenant`, `index`, `outbox`, `pages`, `comments`, `channel`, `forum`, `search`, `commerce`, capability-owned `rustok-ai-admin`, и storefront surfaces `blog`, `pages`, `commerce`, `forum`, `search`.
