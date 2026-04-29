# План верификации платформы: frontend-поверхности

- **Статус:** актуальный детальный чеклист
- **Контур:** Leptos hosts, Next.js hosts, module-owned UI packages, shared UI libraries
- **Companion-план:** [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md)

---

## Актуальный scoped contract

План верификации frontend-поверхностей опирается на current-state UI model:

- UI остаётся module-owned
- hosts только монтируют surfaces
- internal Leptos data layer использует `#[server]`
- GraphQL остаётся параллельным transport contract
- effective locale приходит из host/runtime layer

## Фаза 1. Leptos hosts

### 1.1 `apps/admin`

**Файлы:**
- `apps/admin/src/`
- `apps/admin/docs/README.md`

- [ ] `apps/admin` остаётся host application, а не owner module UI.
- [ ] Module routing и registry отражают текущий manifest-driven contract.
- [ ] `#[server]` path и GraphQL path сосуществуют без дрейфа контрактов.
- [ ] Effective locale прокидывается через host/runtime context.

### 1.2 `apps/storefront`

**Файлы:**
- `apps/storefront/src/`
- `apps/storefront/docs/README.md`

- [ ] `apps/storefront` остаётся host application для module-owned storefront surfaces.
- [ ] Routing, locale path и host wiring совпадают с `docs/UI/storefront.md`.
- [ ] Нет app-local business logic, подменяющей ownership module packages.

## Фаза 2. Next.js hosts

### 2.1 `apps/next-admin`

- [ ] Next admin host монтирует module-owned или capability-owned surfaces без дрейфа ownership.
- [ ] Locale/runtime contract совпадает с общим i18n policy.
- [ ] Frontend build/type/lint path остаётся воспроизводимым.

### 2.2 `apps/next-frontend`

- [ ] Next storefront host использует host/runtime locale contract.
- [ ] Storefront routing согласован с общим route contract.
- [ ] Host-only код не дублирует module-owned domain logic.

## Фаза 3. Module-owned UI packages

### 3.1 Leptos UI packages

- [ ] `admin/` и `storefront/` sub-crates согласованы с `rustok-module.toml`.
- [ ] UI package docs согласованы с local docs owning module.
- [ ] Package не вводит собственный locale/auth contract.
- [ ] Package не переносит в себя ownership доменной логики.

### 3.2 Capability-owned UI

- [ ] Capability-owned UI packages не выдаются за UI-поверхности платформенных модулей.
- [ ] Их runtime/docs contract остаётся согласованным с host layer.

## Фаза 4. Shared UI libraries

### 4.1 Reusable UI/tooling layer

- [ ] Shared Leptos/UI libraries используются как reusable building blocks, а не как скрытый host/business layer.
- [ ] Library contracts не конфликтуют с host locale/runtime policy.

## Фаза 5. i18n и route checks

### 5.1 Обязательные targeted gates

- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm.cmd run verify:storefront:routes`

Если менялся host wiring или UI contract, эти проверки считаются обязательными.

## Фаза 6. Точечные локальные проверки

### 6.1 Минимум

- [ ] targeted `cargo check` / `cargo test` для затронутых Leptos packages
- [ ] targeted `npm run lint` / `npm run typecheck` для затронутого Next host
- [ ] targeted build/smoke, если менялся runtime wiring

## Open blockers

- [ ] Runtime-only blockers фиксировать отдельно и кратко, не превращая этот документ в endless backlog.
- [ ] При drift между host docs и module docs сначала чинить local docs owning component.

## Связанные документы

- [UI README](../UI/README.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Storefront](../UI/storefront.md)
- [Архитектура i18n](../architecture/i18n.md)
- [Главный README по верификации](./README.md)
