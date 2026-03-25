# Документация `rustok-commerce`

В этой папке хранится документация модуля `crates/rustok-commerce`.

## Документы

- [План реализации](./implementation-plan.md) — подробный план миграции `rustok-commerce`
  на Medusa-подобную архитектуру; переносит готовый исследовательский план в docs репозитория,
  фиксирует backlog противоречий и содержит официальные ссылки Medusa v2 для API-сверки

## Статус распила

- `rustok-product`, `rustok-pricing` и `rustok-inventory` уже выделены в отдельные crates и platform modules.
- `rustok-commerce` теперь играет роль `Ecommerce` umbrella/root module для всего ecommerce family и держит
  transport/API surface, orchestration, state-machine заказа и legacy-части, которые еще не вынесены в отдельные модули.
- Общие DTO, entities, error surface и search helpers вынесены в `rustok-commerce-foundation`.

## Статус адаптеров

- GraphQL и REST адаптеры commerce теперь живут внутри `crates/rustok-commerce`
  (`src/graphql/*`, `src/controllers/*`).
- `apps/server` больше не содержит бизнес-логики commerce-адаптеров и использует только
  thin shim/re-export слой для маршрутов, OpenAPI и GraphQL schema composition.
- Общие transport-контракты (`AuthContext`, `TenantContext`, `RequestContext`,
  `require_module_enabled`, locale/pagination helper-ы) модуль получает из `rustok-api`.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

