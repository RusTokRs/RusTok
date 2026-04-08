# План верификации платформы: целостность ядра

- **Статус:** актуальный детальный чеклист
- **Контур:** core crate-ы, foundation-контракты, реестр модулей, ядро auth/RBAC/tenant
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

---

## Актуальный scoped contract

План верификации целостности ядра проверяет, что server host и foundation crate-ы
по-прежнему образуют согласованное ядро для всех платформенных модулей.

Сюда входят:

- `apps/server`
- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`
- `rustok-auth`
- `rustok-cache`
- `rustok-email`

## Фаза 1. Foundation contracts

### 1.1 Core crate-ы

- [ ] Foundation crates собираются и не расходятся по публичным контрактам.
- [ ] Shared contracts для module/runtime layer не дублируются локально в host-коде.
- [ ] Event, auth, tenant и RBAC contracts совпадают с central docs и local docs owning crates.

### 1.2 Module registry

- [ ] `ModuleRegistry` и manifest/runtime wiring отражают текущую platform composition.
- [ ] `Core` и `Optional` semantics не размыты.
- [ ] Support/capability crate-ы не выдаются за платформенные модули.

## Фаза 2. Auth / tenant / RBAC ядро

### 2.1 Auth baseline

- [ ] Auth/session contract централизован и не размазан по host-local обходам.
- [ ] Password/session/token flow соответствует текущему auth contract.
- [ ] Email/auth integration не расходится с foundation/runtime layer.

### 2.2 Tenant baseline

- [ ] Tenant resolution остаётся единым host/runtime path.
- [ ] Tenant lifecycle не ломает core module semantics.
- [ ] `tenant_modules` используется только для `Optional` flows и не подменяет platform composition.

### 2.3 RBAC baseline

- [ ] RBAC enforcement path проходит через текущий typed/runtime contract.
- [ ] Host/module code не возвращается к ad-hoc role checks.
- [ ] Permission ownership совпадает с owning modules и local docs.

## Фаза 3. Runtime services

### 3.1 Cache / email / outbox

- [ ] Cache runtime остаётся единым shared path.
- [ ] Email runtime не дублируется в обход platform contract.
- [ ] Outbox/runtime delivery остаётся частью core baseline, а не optional add-on.

## Фаза 4. Точечные локальные проверки

### 4.1 Минимум

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] targeted `cargo test` для foundation/core crates, если менялся contract
- [ ] `cargo xtask validate-manifest`, если менялся central composition contract

## Open blockers

- [ ] Отдельно фиксировать environment/runtime blockers, не засоряя сам checklist историей.
- [ ] При drift сначала обновлять local docs owning component, затем central verification docs.

## Связанные документы

- [Обзор архитектуры платформы](../architecture/overview.md)
- [Архитектура модулей](../architecture/modules.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
- [Главный README по верификации](./README.md)
