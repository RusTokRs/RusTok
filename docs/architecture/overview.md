# Обзор архитектуры платформы

RusToK развивается как modular monolith с явным composition root в
`apps/server`, platform modules, host applications и отдельным слоем
shared/support/capability crates.

Этот документ даёт верхнеуровневую карту архитектуры. Детальные правила для
module contract, registry и local docs вынесены в `docs/modules/*`.

## Основные слои

### 1. Хост-приложения

- `apps/server` — основной runtime host, HTTP/GraphQL entry point и composition root
- `apps/admin` — Leptos admin host
- `apps/storefront` — Leptos storefront host
- `apps/next-admin` — Next.js admin host
- `apps/next-frontend` — Next.js storefront host

Хост-приложения собирают runtime, монтируют module-owned surfaces и не должны
забирать ownership доменной логики модуля.

### 2. Платформенные модули

Platform module определяется через `modules.toml` и относится только к одной из
двух runtime-категорий:

- `Core`
- `Optional`

Платформенные модули публикуют собственные runtime-контракты, transport-surfaces,
RBAC ownership и локальную документацию. Для path-модулей обязательны:

- `rustok-module.toml`
- root `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`

### 3. Shared / support crate-ы

Shared crates дают foundation contracts и reusable infrastructure:

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

Они могут быть критичны для runtime, но сами по себе не становятся platform
modules без slug в `modules.toml`.

### 4. Capability crate-ы

Capability crate-ы дают отдельные runtime-capabilities и интеграционные слои:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-telemetry`
- `rustok-iggy`
- `rustok-iggy-connector`

Они участвуют в composition, но не считаются tenant-toggled `Core/Optional`
modules.

## Runtime-композиция

Верхний runtime contract собирается так:

1. `modules.toml` определяет platform composition и dependency graph.
2. `apps/server/src/modules/mod.rs` строит runtime registry.
3. `apps/server/src/modules/manifest.rs` валидирует manifest/runtime contract.
4. `apps/server` и другие hosts монтируют surfaces через manifest-driven wiring.
5. shared/capability crates подключаются как support layers, а не как отдельная
   module taxonomy.

## Источники истины

### Runtime

- `modules.toml`
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`
- `crates/rustok-core/src/module.rs`

### Документация

- root `README.md` на английском фиксирует публичный contract компонента
- `docs/README.md` на русском фиксирует живой runtime/app/module contract
- `docs/implementation-plan.md` на русском фиксирует живой план развития
- central docs в `docs/` связывают карту платформы и не должны дублировать
  локальные docs построчно

## UI и transport-политика

- module-owned UI остаётся у самого модуля
- Leptos surfaces публикуются через `admin/` и `storefront/` sub-crates
- internal Leptos data layer по умолчанию использует `#[server]` functions
- GraphQL остаётся параллельным transport contract
- host applications только монтируют surfaces и routes
- locale выбирается host/runtime layer и передаётся в UI package как effective locale

## Поток событий и read-model

Базовая write/read схема платформы:

1. request приходит в host/runtime layer
2. tenant/auth/RBAC policy применяется до вызова доменной логики
3. модуль выполняет write-side операцию
4. межмодульные события публикуются через transactional outbox
5. read-side и индексация обновляются через event-driven flow
6. UI и API читают согласованные read models и transport surfaces

`rustok-outbox` при этом считается `Core` platform module, а не просто support crate.

## Tenant lifecycle

Tenant-level enable/disable относится только к `Optional` modules и работает
поверх уже собранной platform composition.

Это не должно:

- выключать `Core` modules
- превращать capability crate в platform module
- обходить dependency graph из `modules.toml`

## Критерии готовности для архитектурных изменений

Изменение считается доведённым, если:

1. runtime contract отражён в коде и manifest wiring;
2. локальные docs затронутых компонентов обновлены;
3. central docs в `docs/modules/*`, `docs/architecture/*` и `docs/index.md`
   синхронизированы;
4. при необходимости решение зафиксировано в ADR.

## Связанные документы

- [Архитектура модулей](./modules.md)
- [Диаграмма платформы](./diagram.md)
- [Принципы архитектуры](./principles.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Реестр модулей и приложений](../modules/registry.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
