# Документация `rustok-api`

`rustok-api` — shared web/API adapter layer платформы. Он держит общие
request/auth/tenant/channel/GraphQL контракты, которые нужны host-слою и
модульным transport adapters, но не должны жить в `rustok-core`.

## Назначение

- публиковать канонический shared host/API contract;
- держать reusable request, auth, tenant, channel и GraphQL-facing primitives вне `apps/server`;
- давать модульным crates общий transport-adapter foundation без дублирования web-layer contracts;
- публиковать нейтральные port primitives (`PortContext`, `PortError`, `PortCallPolicy`) для новых transport-agnostic ports.

## Зона ответственности

- request context types, transport-agnostic port context/error/policy primitives и auth/tenant/channel host contracts;
- `UiRouteContext`, `UiRouteQueryUpdate`, `normalize_ui_text`, `parse_ui_csv` и прочие module-agnostic UI host contracts;
- GraphQL helper types и error helpers shared across modules;
- reusable read/write/event-replay/best-effort port enforcement без module-specific business logic;
- request-level locale/tenant/channel resolution primitives, не принадлежащие domain crates;
- отсутствие module-specific resolvers, controllers и business logic.

## Интеграция

- используется `apps/server` как shared composition/root API layer;
- модульные crates могут зависеть от `rustok-api`, когда их GraphQL/REST adapters живут внутри самих модулей;
- зависит от `rustok-core` для shared security/permission primitives;
- не должен дублироваться ни в `apps/server`, ни в per-module helper crates.

## Проверка

- structural verification: local docs и root `README.md` должны оставаться синхронизированными;
- targeted compile/tests выполняются при изменении shared request/auth/channel/GraphQL/UI contracts;
- изменения host/API layer должны сопровождаться синхронизацией consumer docs.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
