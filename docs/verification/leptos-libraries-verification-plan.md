# План верификации платформы: Leptos-библиотеки

- **Статус:** актуальный детальный чеклист
- **Контур:** shared Leptos libraries, host integration, module-owned UI packages, reusable UI/tooling layer
- **Companion-план:** [План верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md)

---

## Актуальный контракт Leptos-библиотек

Этот план подтверждает, что библиотечный UI-контур остаётся согласованным
между reusable Leptos crates, host-приложениями и module-owned UI surfaces.

Проверка опирается на current-state contract:

- Leptos hosts монтируют UI surfaces, но не подменяют ownership модулей
- reusable библиотеки остаются общими building blocks, а не скрытым application layer
- internal Leptos data path использует `#[server]` как default internal layer
- GraphQL сохраняется как параллельный transport contract
- effective locale приходит из host/runtime layer, а не из package-local fallback chain

## Контур проверки

### Shared Leptos crates

- [ ] `crates/leptos-auth`
- [ ] `crates/leptos-forms`
- [ ] `crates/leptos-zustand`
- [ ] `crates/leptos-graphql`
- [ ] `crates/leptos-shadcn-pagination`
- [ ] `crates/leptos-ui`
- [ ] `crates/leptos-zod`
- [ ] `crates/leptos-table`
- [ ] `crates/leptos-hook-form`

### Host consumers

- [ ] `apps/admin`
- [ ] `apps/storefront`

## Фаза 1. Публичный контракт библиотек

### 1.1 Root README и локальные docs

- [ ] Каждая библиотека сохраняет актуальный `README.md` с `Purpose`, `Responsibilities`, `Entry points`, `Interactions`.
- [ ] Local docs внутри `crates/leptos-*` не расходятся с фактическим public contract.
- [ ] Библиотека явно фиксирует, где заканчивается reusable layer и начинается host/module-owned logic.

### 1.2 Ownership boundary

- [ ] Reusable Leptos crates не маскируются под module-owned UI packages.
- [ ] Библиотеки не вводят собственный auth/locale/runtime contract поверх host policy.
- [ ] App-specific сценарии не закрепляются в shared crate как скрытая зависимость на конкретный host.

## Фаза 2. Host integration

### 2.1 `apps/admin`

- [ ] `apps/admin` использует Leptos libraries как building blocks, а не как контейнер для обхода module-owned UI.
- [ ] `UiRouteContext`, effective locale и module route base остаются host-provided.
- [ ] `#[server]` и GraphQL integration не расходятся с текущим UI/runtime contract.

### 2.2 `apps/storefront`

- [ ] `apps/storefront` использует shared Leptos libraries без дублирования storefront-specific business logic в shared crates.
- [ ] Storefront route/locale contract совпадает с `docs/UI/storefront.md` и `docs/architecture/i18n.md`.
- [ ] Library-level abstractions не подменяют module-owned storefront packages.

## Фаза 3. Data layer и transport contract

### 3.1 `#[server]` и GraphQL

- [ ] Leptos libraries не ломают правило: `#[server]` как default internal layer, GraphQL как параллельный contract.
- [ ] Shared crates не зашивают host-specific transport assumptions.
- [ ] Library APIs не создают второй источник истины для fetching/mutations поверх server contract.

### 3.2 i18n и runtime context

- [ ] Shared packages не вводят package-local locale negotiation chain.
- [ ] Locale, tenant и auth context приходят из host/runtime layer.
- [ ] UI libraries не расходятся с manifest/module wiring и host route context.

## Фаза 4. Bypass и drift checks

### 4.1 Bypass patterns

- [ ] В `apps/admin` и `apps/storefront` нет систематических bypass-реализаций поверх shared Leptos contracts.
- [ ] Если bypass временно существует, он зафиксирован локально и не подменяет библиотечный contract.
- [ ] Новый reusable functionality добавляется в shared crate, а не размазывается по host apps.

### 4.2 Документационный drift

- [ ] `docs/UI/README.md`, `docs/UI/graphql-architecture.md`, `docs/UI/storefront.md` согласованы с текущим библиотечным слоем.
- [ ] Local docs приложений и библиотек описывают один и тот же integration contract.

## Точечные локальные проверки

- [ ] targeted `cargo check` / `cargo test` для затронутых `crates/leptos-*`
- [ ] targeted `cargo check` / `cargo test` для `apps/admin` и `apps/storefront`, если менялся host integration path
- [ ] `npm run verify:i18n:ui`, если менялись shared locale/UI contracts
- [ ] `npm run verify:i18n:contract`, если менялся locale/runtime contract
- [ ] targeted UI smoke, если менялся route wiring или shared rendering contract

## Open blockers

- [ ] Не превращать этот документ в weekly-таблицу статусов и backlog обходов.
- [ ] Runtime-only blockers фиксировать кратко и отдельно от library contract.
- [ ] При drift между shared crate и host app сначала чинить owning docs и public contract, а не накапливать исключения.

## Связанные документы

- [Верификация frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md)
- [UI README](../UI/README.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Storefront](../UI/storefront.md)
- [Архитектура i18n](../architecture/i18n.md)
- [Документация Leptos Admin](../../apps/admin/docs/README.md)
- [Документация Leptos Storefront](../../apps/storefront/docs/README.md)
