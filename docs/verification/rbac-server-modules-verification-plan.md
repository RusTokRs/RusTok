# План верификации платформы: RBAC, сервер и runtime-модули

- **Статус:** актуальный детальный чеклист
- **Контур:** server authorization path, typed permissions, runtime module contract, capability boundaries
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

---

## Актуальный контракт RBAC и серверного доступа

Этот план подтверждает, что live authorization contract остаётся согласованным
между `apps/server`, `rustok-rbac`, foundation crates, runtime modules и
capability surfaces.

Источники истины для RBAC/server verification:

- код `apps/server`
- typed permission vocabulary из `rustok-core`
- runtime module contracts из `modules.toml`, `rustok-module.toml` и `RusToKModule`
- локальные docs затронутых модулей и capability crates

## Фаза 1. Server authorization path

### 1.1 Entry points

- [ ] GraphQL, REST, `#[server]` и operational endpoints проходят через актуальный auth/RBAC path.
- [ ] `SecurityContext` строится из resolved permissions и tenant/user context, а не из role shortcuts.
- [ ] Server extractors, guards и service entry points не разводят параллельные authorization rules.

### 1.2 Antipattern checks

- [ ] В live server path нет ad-hoc проверок вида `UserRole::*` вместо typed permissions.
- [ ] `infer_user_role_from_permissions()` не подменяет фактическую авторизацию.
- [ ] Host-level обходы не дублируют `RbacService` и permission-aware guards.

## Фаза 2. Typed permission vocabulary

### 2.1 Foundation contract

- [ ] `Permission`, `Resource`, `Action` из `rustok-core` остаются единым источником permission vocabulary.
- [ ] Server-side authorization не уходит в stringly-typed permissions или локальные role aliases.
- [ ] Local docs и central docs не расходятся с текущим permission model.

### 2.2 Module ownership

- [ ] Runtime modules с RBAC-managed functionality публикуют актуальный permission surface.
- [ ] Ownership permissions для `auth`, `tenant`, `rbac`, `content`, `commerce`, `blog`, `forum`, `pages`, `media`, `workflow` совпадают между кодом, manifest и docs.
- [ ] Dependency edges вроде `blog -> content`, `forum -> content`, `pages -> content` не скрывают неописанные authorization expectations.

## Фаза 3. Runtime modules и capability boundaries

### 3.1 Runtime module contract

- [ ] `modules.toml`, runtime registry и `RusToKModule::permissions()` согласованы.
- [ ] Runtime modules не теряют `README.md` / `docs/README.md` / `docs/implementation-plan.md` contract.
- [ ] `outbox` остаётся `Core` module и не смешивается с tenant-toggled capability semantics.

### 3.2 Capability surfaces

- [ ] `alloy`, `flex`, `rustok-mcp` и другие capability crates не маскируются под runtime modules.
- [ ] Capability docs явно описывают свои authorization boundaries и зависимости от server/runtime contract.
- [ ] Capability paths не используют `tenant_modules` как замену явной permission model, если это не часть documented runtime contract.

## Фаза 4. Documentation sync

### 4.1 Central docs

- [ ] `docs/modules/registry.md`, `docs/modules/crates-registry.md`, `docs/architecture/api.md`, `docs/architecture/modules.md` отражают текущую RBAC/server картину.
- [ ] Verification docs остаются checklist-слоем и не превращаются в архив расследований.

### 4.2 Local docs

- [ ] Затронутые runtime modules и capability crates синхронизируют `README.md`, `docs/README.md`, `docs/implementation-plan.md`.
- [ ] Раздел `## Interactions` в root `README.md` не расходится с server authorization path и runtime dependencies.

## Точечные локальные проверки

- [ ] targeted `cargo xtask module validate <slug>` для модулей, затрагивающих auth/RBAC/server boundaries
- [ ] targeted `cargo xtask module test <slug>` для затронутых модулей
- [ ] targeted `cargo test -p rustok-server --lib`, если менялся server authorization path
- [ ] targeted grep/rg по `apps/server/src` на role shortcuts и локальные authorization bypass patterns
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, если менялись dependency boundaries или server/module ownership

## Stop-the-line conditions

- [ ] Live server path авторизует по role shortcuts вместо explicit permissions.
- [ ] Runtime module с RBAC-managed behavior не публикует актуальный permission surface.
- [ ] Capability crate внедряется в server/runtime path без ясного authorization contract.
- [ ] Docs утверждают одну permission/dependency картину, а код реализует другую.

## Связанные документы

- [Главный README по верификации](./README.md)
- [Верификация целостности ядра](./platform-core-integrity-verification-plan.md)
- [Верификация API-поверхностей](./platform-api-surfaces-verification-plan.md)
- [Архитектура API](../architecture/api.md)
- [Архитектура модулей](../architecture/modules.md)
- [Реестр модулей и приложений](../modules/registry.md)
