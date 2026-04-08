# Документация `apps/server`

Локальная документация для главного backend host-приложения RusToK. Этот файл фиксирует только живой composition/runtime contract; детальные runbook, framework notes и rollout-планы вынесены в профильные документы внутри этой папки и в central docs.

## Назначение

`apps/server` является главным backend composition root. Приложение:

- собирает platform modules, shared foundation crates и host-level capabilities в единый runtime;
- публикует HTTP, GraphQL, Leptos `#[server]`, health, metrics и related control-plane surfaces;
- остаётся thin transport/wiring слоем там, где доменная логика уже вынесена в модульные crates.

## Обязательный platform baseline

Для `apps/server` обязательный baseline состоит из двух слоёв.

Platform `Core` modules:

- `rustok-auth`
- `rustok-cache`
- `rustok-channel`
- `rustok-email`
- `rustok-index`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`

Shared foundation / support crates:

- `rustok-core`
- `rustok-events`
- `rustok-telemetry`
- `rustok-api`

Логика tenant-toggle относится только к `Optional` modules. `Core` modules не должны трактоваться как отключаемые host-конфигурацией.

## Runtime surface

- `/api/graphql` и `/api/fn/*` являются параллельными transport-слоями; Leptos server functions не заменяют GraphQL API.
- Health/observability surface публикуется через `/health*` и `/metrics`.
- Module/runtime wiring опирается на `modules.toml`, `rustok-module.toml` и generated host integration.
- `apps/server` может работать как `full` host или как `registry_only`, но `host_mode` не заменяет deployment profile и не меняет build/deploy semantics.
- Для registry/governance surfaces именно сервер остаётся каноническим валидатором lifecycle policy, `reason` / `reason_code` contract и allowed action set; thin clients могут делать preflight, но не определяют policy локально.
- `GET /v2/catalog/publish/{request_id}` остаётся machine-readable operator status contract: без `x-rustok-actor` он возвращает status-driven superset `governanceActions`, а при наличии actor header режет только request-level действия до реально разрешённых для этого actor.

## Границы ответственности

`apps/server` отвечает за:

- transport adapters, middleware, request/runtime context и host wiring;
- общий GraphQL schema surface и Leptos server-function entrypoints;
- health/runtime guardrails, build/release orchestration и operator control-plane endpoints;
- RBAC enforcement, auth/session integration и host-level observability.

`apps/server` не должен:

- дублировать module-owned domain services, storage и permission logic;
- подменять модульные interaction contracts собственными ad hoc соглашениями;
- ломать dual-path contract между GraphQL и `#[server]`, если добавляется новый internal path.

## Health и runtime guardrails

- [health.md](./health.md) является каноническим документом для readiness, runtime guardrails, `registry_only` smoke и rollout evidence.
- `apps/server` обязан явно различать `DeploymentProfile` и `settings.rustok.runtime.host_mode`.
- Для reduced hosts health/runtime surface должен описывать фактически поднятый runtime, а не full monolith по умолчанию.

## Verification

Минимальный локальный verification path для изменений в `apps/server`:

- точечные `cargo check` и `cargo test` по затронутым crates и transport slices;
- `cargo xtask module validate <slug>` для модулей, чей host wiring или manifest contract изменился;
- targeted contract checks для GraphQL, REST, server functions и health/runtime surface;
- отдельная проверка health/runtime paths, если затронуты deployment profile, `host_mode` или remote executor/runtime guardrails.

## Связанные документы

- [Health и runtime guardrails](./health.md)
- [Стек библиотек](./library-stack.md)
- [Контракт транспорта событий](./event-transport.md)
- [План верификации ядра](./CORE_VERIFICATION_PLAN.md)
- [Loco integration](./loco-core-integration-plan.md)
- [Контракт event flow](../../../docs/architecture/event-flow-contract.md)
- [Контракты manifest-слоя](../../../docs/modules/manifest.md)
- [Карта документации](../../../docs/index.md)
