# Документация `rustok-index`

`rustok-index` — core-модуль платформы для централизованного index/read-model
слоя. Его задача — не продуктовый search UX, а денормализованное хранение,
ingestion и cross-module query substrate.

## Назначение

- публиковать канонический index/read-model contract для платформы;
- держать ingestion, rebuild и consistency semantics внутри модуля;
- давать host и другим модулям стабильный internal query substrate для cross-module reads.

## Зона ответственности

- index storage и денормализованные projection records;
- ingestion lifecycle: bootstrap, incremental sync, rebuild и drift control;
- link-aware filtering и cross-module query substrate;
- operator-facing health/rebuild controls для index state;
- отсутствие product-facing search ranking и full-text UX semantics.

## Интеграция

- зависит от `rustok-core` и стабильных integration contracts модулей-источников;
- может использоваться `apps/server` и другими platform consumers как internal query/read-model layer;
- FBA owner ports (`IndexReadModelPort`, `IndexRebuildPort`) используют shared `rustok_api::PortContext`/`PortError` и `PortCallPolicy` вместо package-local deadline/error shims;
- adapter-side FBA guardrails включают validation helpers для read/list/rebuild requests, tenant-scope guard `ensure_index_document_tenant_scope` и typed degraded-mode error `index.rebuild_disabled`;
- не должен схлопываться с `rustok-search`: `search` может читать projections, но `index` не становится search module;
- event-driven consumers модуля публикуются через `IndexModule::register_event_listeners(...)` и собираются сервером из `ModuleRegistry`, а не через отдельный host-owned dispatcher path;
- текущие module-owned consumers включают `content_indexer`, `product_indexer` и `flex_indexer` для standalone Flex read-model slice `index_flex_entries`;
- остаётся `Core` module без самостоятельного storefront UX как primary surface; operator-facing admin overview живёт в `rustok-index-admin` и оформлен как FFA `core` + native-only `transport` + `ui/leptos` adapter.

## Проверка

- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`
- targeted tests для ingestion, rebuild, link-aware queries и consistency semantics при изменении контракта

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
