# Verification: локальный контур платформы

Этот раздел фиксирует минимальный reproducible verification path для RusTok. Он разделяет обязательные локальные проверки, Windows-friendly entrypoints и legacy perimeter scripts.

## Обязательные prerequisites

- `cargo`
- `node`
- `python` или `py` для архитектурного guard

`bash` и Git Bash не являются обязательными для старта работ по module standard. Они нужны только для legacy perimeter scripts, которые ещё не перенесены в Node, PowerShell или `cargo xtask`.

## Обязательный модульный verification path

### 1. Scoped contract audit

```powershell
cargo xtask module validate
```

Проверка должна fail-fast, если path-модуль:

- не имеет `rustok-module.toml`;
- не имеет `docs/README.md` или `docs/implementation-plan.md`;
- имеет drift по зависимостям между `modules.toml` и локальным manifest;
- имеет broken admin/storefront wiring;
- не отражён корректными ссылками в `docs/modules/_index.md`.

### 2. Таргетированные module tests

```powershell
cargo xtask module test <slug>
```

Эта команда объединяет локальную manifest/documentation validation и compile smoke для owning crate и module-owned UI crates.

### 3. UI/i18n и storefront route gates

```powershell
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:storefront:routes
```

### 4. Architecture guard

После установки Python:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

PowerShell-wrapper ищет `py` или `python` в `PATH` и запускает `scripts/architecture_dependency_guard.py`.

## Дополнительные smoke-пути

- `powershell -File scripts/verify/verify-deployment-profiles.ps1`

Deployment-profile smoke нужен, когда меняются host wiring, runtime profiles или verification entrypoints, связанные с deployment profile matrix.

## Legacy perimeter scripts

Shell-скрипты в `scripts/verify/*.sh` остаются дополнительным periodic perimeter check. Они не считаются hard prerequisite для локального module audit на Windows-машине без Git Bash.

Используйте их отдельно, если:

- проверяете CI parity;
- хотите расширенный аудит вне минимального module standard path;
- на машине уже установлен Git Bash.

## Связанные документы

- [План унификации module-system](../modules/module-system-plan.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
- [Сводный verification plan](./PLATFORM_VERIFICATION_PLAN.md)
