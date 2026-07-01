# План реализации `rustok-api`

Статус: shared host/API layer уже служит опорой для `apps/server` и
module-owned transport adapters; главная задача — не дать ему разрастись в
параллельный application layer.

## Execution checkpoint

- Current phase: neutral contract ownership hardening
- Last checkpoint: `Port*`, permission and locale contracts moved into `rustok-api`; API no longer depends on core in any feature, core compatibility exports were deleted, and outbox Loco composition moved to `rustok-outbox::loco`.
- Next step: Keep new module ports on `rustok_api::ports` and reject runtime-specific dependencies in the default contract surface.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-07-01T00:00:00Z

## Область работ

- удерживать `rustok-api` как shared web/API adapter foundation;
- синхронизировать request/auth/tenant/channel/UI host contracts и local docs;
- не допускать втягивания module-specific business logic в shared API layer.

## Текущее состояние

- crate уже предоставляет shared request/auth/tenant/channel contexts и GraphQL helpers;
- `UiRouteContext` и related host contracts уже используются для module-owned UI packages;
- `PortContext`/`PortError` задают shared baseline для transport-agnostic ports, а `PortCallPolicy` фиксирует reusable read/write/event-replay/best-effort enforcement без module-specific logic; `rustok-region`, tenant, channel, product, customer, media, workflow, RBAC, tax, fulfillment, payment, pricing, cart, inventory, comments, search, order, index, email delivery, outbox relay и page-builder publish paths уже потребляют shared policy baseline (`PortCallPolicy::read()` для read projections, `PortCallPolicy::write()` для write control);
- default и `server` feature sets владеют neutral API contracts без зависимости на `rustok-core`; runtime RBAC/security живёт в core, который зависит от API;
- `apps/server` остаётся composition root поверх этого слоя, а не второй параллельный shared API framework;
- transport adapters модулей могут постепенно переезжать на `rustok-api` без дублирования common contracts.

## Этапы

### 1. Contract stability

- [x] закрепить `rustok-api` как shared host/API layer;
- [x] удерживать reusable request/auth/channel/UI contracts вне `rustok-core`;
- [~] удерживать sync между public surface, host wiring и local docs; (started: shared FFA UI input and route-query update contracts)

### 2. Boundary hardening

- [~] продолжать выносить действительно shared transport/UI/port helpers из host/module-specific layers; (continued: neutral port context/error primitives, port call policies, typed error constructors and expanded multi-module read/write-port consumer migration)
- [ ] не втягивать сюда module-owned resolvers и controllers;
- [ ] покрывать новые shared contracts targeted compile/tests при изменении surface.

### 3. Operability

- [~] документировать изменения host/API contracts одновременно с изменением runtime surface; (updated for shared write-policy migration across inventory/comments/fulfillment/order/payment/page-builder and previous read-policy cleanup)
- [~] удерживать local docs и `README.md` синхронизированными; (updated for shared write-policy migration across inventory/comments/fulfillment/order/payment/page-builder and previous read-policy cleanup)
- [ ] обновлять consumer-module docs, если меняются shared transport expectations.

## Проверка

- structural verification для local docs и host/API boundary;
- targeted compile/tests при изменении shared request/auth/channel/GraphQL contracts;
- docs sync для `apps/server` и module-owned transport crates.

## Правила обновления

1. При изменении shared host/API contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении consumer expectations обновлять связанные host/module docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
