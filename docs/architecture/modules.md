# Архитектура модулей

Этот документ фиксирует архитектурный взгляд на модульную платформу RusToK:
что считается платформенным модулем, где проходит граница между module crate,
support/capability crate и host application, и где находится источник истины
для runtime и контракта документации.

## Базовая модель

В RusToK платформенные модули делятся только на две runtime-категории:

- `Core`
- `Optional`

Источник истины по составу и зависимостям платформенных модулей: `modules.toml`.

Это означает:

- платформенный модуль определяется не названием crate, а присутствием slug в
  `modules.toml`;
- runtime taxonomy определяется через `Core` и `Optional`, а не через произвольные
  локальные категории;
- support/shared/capability crate-ы могут участвовать в composition, но не
  становятся от этого автоматически tenant-toggled modules.

## Источники истины

### Runtime

- composition root: `modules.toml`
- runtime registration: `apps/server/src/modules/mod.rs`
- manifest/runtime validation: `apps/server/src/modules/manifest.rs`
- базовые модульные контракты: `crates/rustok-core/src/module.rs`

### Документация

- root `README.md` компонента на английском фиксирует публичный контракт:
  `Purpose`, `Responsibilities`, `Entry points`, `Interactions`
- локальный `docs/README.md` на русском фиксирует живой runtime/module/app-контракт
- локальный `docs/implementation-plan.md` на русском фиксирует живой план развития
- central docs в `docs/modules/*` дают карту и навигацию, но не заменяют локальные
  docs компонента

При изменении ownership, runtime-контракта или module-boundaries сначала
обновляются локальные docs компонента, затем central docs.

## Типы компонентов

### Платформенные модули

Платформенный модуль:

- объявлен в `modules.toml`
- имеет `rustok-module.toml`, если это `source = "path"`
- проходит scoped validation через `cargo xtask module validate <slug>`
- участвует в runtime/module lifecycle как `Core` или `Optional`

Текущий scope платформенных модулей смотрите в:

- [обзоре модульной платформы](../modules/overview.md)
- [реестре модулей и приложений](../modules/registry.md)

### Shared / support crate-ы

Эти crate-ы дают foundation или shared-контракты для платформенных модулей и host-layer:

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

Они могут иметь собственные `README.md` и local docs, но не обязаны иметь slug в
`modules.toml`.

### Capability crate-ы

Capability crate-ы добавляют отдельные runtime-capabilities, но не считаются
tenant-toggled платформенными модулями:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-iggy`
- `rustok-iggy-connector`
- `rustok-telemetry`

Их роль описывается как support/capability-слой, а не как `Core`/`Optional`
module-category.

### Хост-приложения

Хост-приложения собирают runtime и монтируют surfaces модулей:

- `apps/server` — composition root и основной runtime host
- `apps/admin` — Leptos admin host
- `apps/storefront` — Leptos storefront host
- `apps/next-admin` — Next.js admin host
- `apps/next-frontend` — Next.js storefront host

Host application не должен становиться canonical owner module-owned domain-логики
или UI-поверхности.

## Политика UI-композиции

Если модуль поставляет UI, этот UI остаётся module-owned:

- Leptos surfaces публикуются через `admin/` и `storefront/` sub-crates
- host applications только монтируют эти surfaces через manifest-driven wiring
- internal Leptos data layer по умолчанию строится на `#[server]` functions
- GraphQL остаётся параллельным transport-контрактом и не удаляется
- locale берётся из host-provided контракта, а не из package-local fallback chain

Само наличие папки `admin/` или `storefront/` не считается доказательством
интеграции. Канонический источник истины здесь — `rustok-module.toml`.

## Outbox и capability-слои

Несколько компонентов важно читать правильно:

- `rustok-outbox` — это `Core` platform module, а не просто support crate
- `rustok-core` и `rustok-events` — shared contract crates, а не tenant-toggled modules
- `rustok-installer` — support crate для installer-core contracts, а не
  tenant-toggled module и не module lifecycle registry entry
- `alloy`, `rustok-ai`, `rustok-mcp`, `flex` — capability layers, а не `Core/Optional`
  modules

Это различие важно для registry, lifecycle, RBAC ownership и documentation-flow.

## Install/uninstall и tenant lifecycle

Нужно различать три уровня:

### Platform-level composition

Platform-level composition определяется через `modules.toml` и build/runtime
registration. Здесь решается:

- какие модули входят в сборку
- какие у них dependency edges
- какие path-modules обязаны иметь `rustok-module.toml`
- какой scoped-контракт должен пройти `xtask`

### Schema composition

Schema composition в текущей версии определяется серверным `Migrator` в
`apps/server/migration`, который объединяет platform-core и module-owned
migrations в один глобально отсортированный список. Installer v1 не должен
обещать физическое исключение schema artifacts optional-модулей из БД только
потому, что модуль выключен на уровне tenant.

### Tenant-level enable/disable

Tenant lifecycle применяется только к `Optional` modules и работает поверх уже
собранной platform composition. Он не должен:

- переключать `Core` modules
- превращать capability crate в platform module
- ломать dependency graph, описанный в `modules.toml`
- удалять или скрывать уже применённые module-owned schema artifacts

## Связанные документы

- [Обзор модульной платформы](../modules/overview.md)
- [Реестр модулей и приложений](../modules/registry.md)
- [Индекс документации по модулям](../modules/_index.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
- [Реестр crate-ов модульной платформы](../modules/crates-registry.md)
- [Шаблон документации модуля](../templates/module_contract.md)

## Runtime control plane и lifecycle

Активный runtime-состав модулей хранится в `platform_state`; `modules.toml` используется как bootstrap/dev input.
Изменение состава выполняется как atomic control-plane операция: manifest валидируется по registry, `platform_state`
обновляется через revision/CAS, а build job получает `manifest_ref = platform_state:<revision>` в той же DB transaction.
Manifest hash — SHA-256 от canonical JSON полного snapshot, включая settings/build/source/dependency metadata.

Tenant enable/disable должен проходить через `ModuleLifecycleService::toggle_module_with_actor()`: operation journal
пишется до изменения tenant state, compat `on_enable`/`on_disable` hooks выполняются как pre-hooks, а успешное изменение
state и перевод operation в `committed` фиксируются одним commit. GraphQL и Leptos SSR/admin surfaces
не владеют lifecycle taxonomy или journal metadata: server GraphQL mapper публикует canonical error codes
(`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`) и recovery/journal fields, а Leptos SSR/admin
слои только прокидывают payload без local remap.

Module-owned migrations могут объявлять ordering metadata рядом со своим exporter-ом; server migrator делает
topological sort и считает missing dependency/cycle ошибкой runtime/test contract.

### Текущий contract control-plane/module lifecycle

- Composition update выполняется только через server-owned orchestration: validation, `platform_state` CAS/revision update и build enqueue находятся в одном transaction boundary; `manifest_ref` имеет форму `platform_state:<revision>`, а `manifest_hash` считается через общий SHA-256 canonical snapshot helper.
- Tenant module lifecycle имеет один production entrypoint — `ModuleLifecycleService::toggle_module_with_actor()`. Прямой model-level toggle и admin-side SQL/bypass не являются contract surface.
- Lifecycle journal использует статусы `validated/running/committed/failed`; post-hook failure не откатывает committed tenant state, а создаёт failed operation с recovery metadata (`status`, `issue`, `retryable`, `recommended_action`, `correlation_id`, `requested_by`, `error_message`).
- Recovery выполняется только через canonical GraphQL/service surface: `moduleOperationRecoveryPlan`, `failedModuleOperationRecoveryPlans`, `retryFailedModuleOperationPostHook`, `compensateFailedModuleOperation`. Compensation разрешается только для `post_hook_failed` operations, когда current effective state всё ещё совпадает с committed requested state.
- GraphQL mapper владеет error taxonomy (`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`); Leptos SSR/admin layers обязаны passthrough и не должны remap'ить taxonomy/journal/recovery fields.
- Module migration ordering фиксируется descriptor contract: topological sort, deterministic lexical tie-breaker, hard errors для missing dependency/cycle и отсутствие hardcoded dependency match в migrator core.
