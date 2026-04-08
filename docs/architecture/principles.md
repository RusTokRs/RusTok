# Архитектурные принципы

> Статус: guidance по текущему состоянию

Это короткий набор правил, по которым удерживается архитектура RusToK.

## 1. RusToK — modular monolith

RusToK строится как modular monolith, а не как набор независимых сервисов.

Следствия:

- composition root находится в `apps/server`
- платформенные модули живут в одном runtime
- границы между модулями проходят по контрактам, а не по процессам

## 2. Для платформенных модулей есть только `Core` и `Optional`

Platform module определяется через `modules.toml` и может относиться только к:

- `Core`
- `Optional`

`Core` modules всегда участвуют в runtime.  
`Optional` modules участвуют в build/runtime composition и могут управляться на
tenant level.

Support/capability crate-ы не образуют третью taxonomy платформенных модулей.

## 3. Role, taxonomy и crate-packaging нельзя смешивать

Нужно различать три оси:

- архитектурная роль: module / shared library / capability crate / host
- runtime taxonomy: `Core` / `Optional`
- техническая упаковка: `crate`

Из этого следует:

- `crate != platform module`
- `ModuleRegistry != архитектурная taxonomy`
- bootstrap wiring != ownership доменной логики

## 4. Источник истины для platform composition — `modules.toml`

Состав платформенных модулей, dependency graph и composition-контракт определяются
через `modules.toml`.

Для path-модуля это должно быть согласовано с:

- `rustok-module.toml`
- runtime registration
- локальные docs
- verification-flow через `xtask`

## 5. Источник истины для документации живёт в компоненте

Для каждого first-party компонента:

- root `README.md` на английском фиксирует публичный контракт
- `docs/README.md` на русском фиксирует живой runtime/app/module-контракт
- `docs/implementation-plan.md` на русском фиксирует живой план развития

Central docs в `docs/` дают карту и навигацию, но не должны подменять локальные
docs компонента.

## 6. Server — host, а не свалка доменной логики

`apps/server` владеет:

- transport layer
- runtime wiring
- auth/session integration
- RBAC enforcement path
- operational endpoints

`apps/server` не должен становиться местом для накопления module-owned domain
логики, если у этой логики уже есть owning crate.

## 7. Write-side correctness важнее convenience

Write-side операции должны быть:

- транзакционными
- tenant-safe
- RBAC-aware
- согласованными с event-контрактом

Межмодульные события публикуются через transactional path там, где нужна
атомарность write + event persistence.

## 8. Read-side отделён от write-side

RusToK удерживает разделение:

- write-side для доменных изменений
- read-side для projections, индексов и быстрых query paths

Это позволяет:

- не тащить тяжёлые join-paths в storefront/read flows
- строить downstream consumers независимо
- развивать индексацию и projections отдельно от write-side моделей

## 9. UI остаётся module-owned

Если модуль поставляет UI:

- Leptos surfaces публикуются через `admin/` и `storefront/` sub-crates
- host applications только монтируют surfaces через manifest-driven wiring
- internal Leptos data layer по умолчанию использует `#[server]` functions
- GraphQL остаётся параллельным transport-контрактом
- locale выбирается host/runtime layer, а не package-local fallback chain

## 10. Capability crate-ы не подменяют module taxonomy

Capability/support crate-ы вроде:

- `alloy`
- `rustok-mcp`
- `rustok-ai`
- `rustok-telemetry`
- `flex`

не должны описываться как обычные tenant-toggled платформенные модули, если они не
объявлены как платформенные модули в `modules.toml`.

И обратное тоже верно: если компонент объявлен как платформенный модуль, он обязан
жить в taxonomy `Core/Optional`.

## 11. Документация должна отражать код

Если код и docs расходятся, приоритет у текущего кода, а документация должна
быть синхронно обновлена.

Особенно это касается:

- module taxonomy
- event flow
- API surface
- host wiring
- tenant и RBAC boundaries

## 12. Boundary-change требует синхронного обновления

При изменении архитектурных границ нужно обновлять одновременно:

1. локальные docs затронутого компонента
2. central docs в `docs/`
3. `docs/index.md`
4. docs верификации, если меняется verification-контракт
5. ADR, если изменение нетривиально

## Связанные документы

- [Обзор архитектуры платформы](./overview.md)
- [Архитектура модулей](./modules.md)
- [Диаграммы платформы](./diagram.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
