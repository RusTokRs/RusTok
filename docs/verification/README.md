# Планы верификации

Этот раздел собирает планы верификации по основным контурам платформы и фиксирует минимальный локальный путь проверки для модульной системы.

## Назначение

- хранить планы верификации в одном месте;
- отделять периодическую верификацию от live/remediation documentation;
- давать единый вход для точечных и широких прогонов;
- фиксировать обязательные quality gates для платформенных модулей.

Планы исполнения и backlog по исправлениям не должны жить в этом разделе как бесконечный список задач. Здесь остаются только правила проверки, целевые команды и ссылки на профильные планы.

## Основные документы

- [Сводный план верификации](./PLATFORM_VERIFICATION_PLAN.md)
- [Верификация foundation-слоя](./platform-foundation-verification-plan.md)
- [Верификация API-поверхностей](./platform-api-surfaces-verification-plan.md)
- [Верификация frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md)
- [Верификация качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md)
  (включая Docs quality gates baseline по DOC-07)
- [Верификация целостности ядра](./platform-core-integrity-verification-plan.md)
- [Верификация RBAC, сервера и runtime-модулей](./rbac-server-modules-verification-plan.md)
- [Верификация Leptos-библиотек](./leptos-libraries-verification-plan.md)

## Минимальный путь проверки для платформенных модулей

Для scoped модулей платформы канонический локальный путь такой:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

`module validate` проверяет контракт модуля и локальные docs, а `module test` строит точечный test/check plan для самого модуля и его UI-пакетов.

Если меняется composition contract всей платформы, дополнительно нужен:

```powershell
cargo xtask validate-manifest
```


## Reference artifacts pipeline (DOC-09 / B11)

Для phase 1 по DOC-09 используем единый локальный скрипт экспорта reference-артефактов:

```bash
scripts/verify/export-reference-artifacts.sh artifacts/reference
```

Что делает скрипт:

- генерирует rustdoc для `rustok-server` и `rustok-workflow` (если не задан `SKIP_RUSTDOC=1`);
- сохраняет OpenAPI (`/api/openapi.json`, `/api/openapi.yaml`);
- сохраняет GraphQL introspection snapshot из `/api/graphql`;
- пишет `manifest.txt` с timestamp/base_url.

Переменные окружения:

- `RUSTOK_BASE_URL` — базовый URL сервера (по умолчанию `http://127.0.0.1:5150`);
- `SKIP_RUSTDOC=1` — пропустить `cargo doc` и сделать только API exports.

Минимальный verification-набор для PR (B11):

```bash
cargo xtask --help
scripts/verify/export-reference-artifacts.sh artifacts/reference
rg -n "openapi|graphql-introspection|manifest.txt" artifacts/reference -S
```

## Windows hybrid path

На текущем Windows-окружении обязательный локальный путь верификации не должен зависеть от Bash как hard prerequisite.

Минимальный Windows-native набор:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm.cmd run verify:storefront:routes
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

Дополнительно:

- Python-dependent проверки запускаются через установленный Python.
- Bash-only scripts допускаются как legacy/perimeter checks, но не как единственный способ подтвердить модульный контракт на этой машине.

## Что считается обязательным для модульной унификации

При изменении module system или локального контракта модуля нужно проверять не только код, но и документационный слой:

- наличие `README.md`, `docs/README.md`, `docs/implementation-plan.md`;
- согласованность `modules.toml` и `rustok-module.toml`;
- корректность admin/storefront manifest wiring;
- актуальность central docs в `docs/modules/*` и `docs/index.md`.

Support/capability crates могут участвовать в общей документационной унификации, но scoped `module validate` применяется только к slug из `modules.toml`.

## Как пользоваться набором планов

1. Начинать со [сводного плана верификации](./PLATFORM_VERIFICATION_PLAN.md), если нужен широкий прогон.
2. Переходить в профильный план, если меняется конкретный контур: foundation, API, frontend, RBAC, UI libraries.
3. Для точечной работы по модулю сначала выполнять `cargo xtask module validate <slug>`, а не полный workspace-wide прогон.
4. Нерешённые блокеры фиксировать в профильном плане или в локальных docs соответствующего компонента, а не превращать `docs/verification/README.md` в backlog.

## Регламент обновления

При изменении архитектуры, API, UI-контрактов, module system, observability или quality gates:

1. Обновить локальные docs затронутого `apps/*` или `crates/*`.
2. Обновить профильный план верификации в этой папке, если изменился сам порядок проверки.
3. Обновить связанные central docs в `docs/modules/*`, `docs/architecture/*` и `docs/index.md`.
4. Если меняется acceptance-контракт модуля, синхронно обновить [контракт manifest-слоя](../modules/manifest.md).

## Статусы

- `Не начато`
- `В процессе`
- `Завершено`
- `Заблокировано`

> Статус документа: актуальный. Для модульной системы этот README должен оставаться синхронизированным с `cargo xtask module validate`, `cargo xtask module test` и central docs в `docs/modules/*`.
