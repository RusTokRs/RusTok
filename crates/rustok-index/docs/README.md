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
- не должен схлопываться с `rustok-search`: `search` может читать projections, но `index` не становится search module;
- остаётся `Core` module без самостоятельного storefront/admin UX как primary surface.

## Проверка

- `cargo xtask module validate index`
- `cargo xtask module test index`
- targeted tests для ingestion, rebuild, link-aware queries и consistency semantics при изменении контракта

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
