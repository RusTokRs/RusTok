# Документация `rustok-search`

`rustok-search` — dedicated core search module платформы. Локальная документация
модуля должна описывать сам search runtime, а не смешивать его с `rustok-index`
или host-specific UI wiring.

## Назначение

- публиковать канонический search API и runtime contracts;
- держать search document materialization, ranking и query normalization внутри модуля;
- развивать admin/storefront search surfaces поверх общего backend contract.

## Зона ответственности

- `search_documents` и связанные search-owned словари/analytics storage;
- search query parsing, ranking, filter presets, typo tolerance и merchandising rules;
- admin/storefront query surfaces и module-owned UI packages;
- observability, rebuild и diagnostics для search state;
- optional connector model для внешних search engines.

## Интеграция

- остаётся отдельным модулем по отношению к `rustok-index`: `search` отвечает за UX, ranking и engine semantics, а не за shared indexed read-model substrate;
- использует PostgreSQL как baseline engine и может расширяться отдельными connector crates;
- должен держать Leptos и Next UI surfaces на одном backend contract;
- доменные модули поставляют изменения через ingestion path, не зная об активном engine.

## Проверка

- `cargo xtask module validate search`
- `cargo xtask module test search`
- targeted tests для query normalization, ranking profiles, rebuild flows и diagnostics surfaces

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Observability runbook](./observability-runbook.md)
- [ADR: boundary `index != search`](../../../DECISIONS/2026-03-29-index-search-boundary.md)
