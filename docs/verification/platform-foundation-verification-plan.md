# План верификации платформы: foundation

- **Статус:** актуальный детальный чеклист
- **Контур:** workspace baseline, foundation crates, module composition, auth/RBAC/tenant foundation
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

---

## Актуальный scoped contract

План foundation-верификации подтверждает, что platform baseline остаётся
согласованным на трёх уровнях:

- workspace и host/runtime foundation
- module composition contract
- minimum по docs/manifests/verification для scoped-модулей

Для path-modules current-state minimum:

- root `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`
- `rustok-module.toml`

Канонические локальные команды:

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`
- `cargo xtask validate-manifest`

## Windows-hybrid path

На Windows обязательный локальный verification-path не зависит от Bash как hard
prerequisite.

Минимальный baseline:

- Cargo/xtask для module/runtime contract
- Node/npm для UI/i18n/routes gates
- Python для architecture guard
- Git Bash только для legacy perimeter checks, если они нужны отдельно

## Фаза 1. Workspace baseline

### 1.1 Сборка и базовая согласованность

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo fmt --all -- --check`
- [ ] targeted `cargo test`, если менялся foundation/runtime contract

### 1.2 Tooling и prerequisites

- [ ] Локальная среда поддерживает минимальный Windows-hybrid verification-path.
- [ ] Environment blockers фиксируются отдельно и не подменяют сам contract.

## Фаза 2. Module composition contract

### 2.1 `modules.toml` и runtime registry

- [ ] `modules.toml` отражает реальный platform scope.
- [ ] `ModuleRegistry` и manifest/runtime wiring совпадают с composition contract.
- [ ] `Core` и `Optional` semantics не размыты.
- [ ] Support/capability crate-ы не выдаются за платформенные модули.

### 2.2 Scoped module contract

- [ ] Path-modules имеют `rustok-module.toml`.
- [ ] Root `README.md`, `docs/README.md`, `docs/implementation-plan.md` присутствуют и соответствуют current docs-standard.
- [ ] Module dependencies и wiring согласованы между кодом, manifest и local docs.

## Фаза 3. Foundation crates

### 3.1 Shared contracts

- [ ] `rustok-core`, `rustok-api`, `rustok-events`, `rustok-storage`, `rustok-test-utils` образуют согласованный foundation layer.
- [ ] Shared contracts не дублируются локально в host/module code.
- [ ] Central docs совпадают с текущими foundation boundaries.

### 3.2 Core платформенные модули

- [ ] `auth`, `cache`, `channel`, `email`, `index`, `search`, `outbox`, `tenant`, `rbac` остаются согласованными с runtime baseline.
- [ ] `rustok-outbox` остаётся `Core` module, а не optional/support add-on.

## Фаза 4. Auth / tenant / RBAC foundation

### 4.1 Auth

- [ ] Auth/session/token contract остаётся централизованным.
- [ ] Host-local обходы не подменяют foundation auth flow.

### 4.2 Tenant

- [ ] Tenant resolution и tenant lifecycle соответствуют current runtime contract.
- [ ] `tenant_modules` используется только для `Optional` flows и не подменяет platform composition.

### 4.3 RBAC

- [ ] Typed permission/runtime contract остаётся единым.
- [ ] Нет возврата к ad-hoc role checks в host/module code.

## Фаза 5. Точечные локальные проверки

### 5.1 Минимум

- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>` для затронутых модулей
- [ ] targeted `cargo xtask module test <slug>` для затронутых модулей
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, если менялся architecture/runtime contract

## Open blockers

- [ ] Не превращать этот документ в исторический журнал инцидентов.
- [ ] Runtime/environment blockers фиксировать кратко и отдельно.

## Связанные документы

- [Контракт `rustok-module.toml`](../modules/manifest.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Архитектура модулей](../architecture/modules.md)
- [Главный README по верификации](./README.md)
