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

## Reference artifacts pipeline in CI (DOC-09 / B12)

Phase 2 для DOC-09 выполняется через CI job `reference-artifacts` в
`.github/workflows/ci.yml`.

Job обязан:

- поднять runtime (`rustok-server`) и дождаться `/api/openapi.json`;
- выполнить `scripts/verify/export-reference-artifacts.sh artifacts/reference`;
- опубликовать `artifacts/reference/**` через `actions/upload-artifact`;
- быть включённым в aggregate gate `ci-success`.

Минимальная проверка B12 в PR:

```bash
rg -n "reference-artifacts|export-reference-artifacts|upload-artifact|ci-success" .github/workflows/ci.yml
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

## Роли `xtask` и `scripts/*` (актуализация 2026-05)

Чтобы не дублировать tooling и не разъезжаться по контрактам:

- `xtask` (Rust) — **каноничный entrypoint** для платформенных и модульных контрактов, которые должны одинаково запускаться на Linux/macOS/Windows.
- `scripts/verify/*.sh` и `scripts/verify/*.mjs` — **периметр и специализированные аудит-проверки**, где важнее быстрый grep/smoke и shell orchestration.
- `scripts/verify/*.ps1` — parity-скрипты для Windows там, где Bash-check обязателен, но должен иметь native fallback.

Практический критерий выбора реализации:

1. **Писать в `xtask` (Rust)**, если:
   - проверка входит в обязательный модульный acceptance path;
   - нужна кроссплатформенность без Bash;
   - есть структурированный парсинг (`modules.toml`, manifests, wiring, registry contracts).
2. **Оставлять в `sh`/`mjs`**, если:
   - это perimeter/security smoke с множеством внешних CLI;
   - проверка носит ad-hoc audit характер и не является модульным gate;
   - критична скорость правок в CI orchestration.
3. **Удалять/схлопывать дубли**, если:
   - один скрипт только проксирует другой без дополнительной логики;
   - команда уже покрыта `cargo xtask ...` и не добавляет отдельный контракт.

## Page Builder FBA verification baseline (Wave 0/Wave 1 gate)

Для трека `page_builder -> pages` обязательный минимальный gate перед продвижением между волнами:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
```

Состав baseline gate:

1. parity provider/consumer contract versions;
2. required fallback/toggle profile structure;
3. toggle profile value consistency (`all_on/publish_off/preview_off/builder_off`).

Этот baseline gate используется как обязательный артефакт для Sprint/Wave evidence в `docs/modules/tiptap-page-builder-implementation-plan.md`.

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

### Принцип для тестов operational scripts

- Тесты в `scripts/tests/*` и `scripts/ci/test_*.py` должны использовать изолированные fixture-каталоги (`mktemp` / `tempfile`) и не зависеть от текущего состояния репозитория.
- Репозиторий может временно содержать drift/legacy данные; это не должно делать script-tests флаки при локальном запуске и в CI.

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
