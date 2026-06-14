# Документация `rustok-pages`

`rustok-pages` — доменный модуль страниц, menus и visual page-builder flows.
Модуль уже работает на pages-owned storage и должен оставаться владельцем page,
block и menu contracts.

## Назначение

- публиковать канонический pages runtime contract для page/body/block/menu surfaces;
- держать module-owned transport adapters и UI packages внутри модуля;
- развивать pages как channel-aware модуль без возврата к shared node storage.

## Зона ответственности

- `PageService`, `BlockService`, `MenuService` и page visibility semantics;
- module-owned storage для pages, page bodies, blocks и menus;
- GraphQL/REST adapters и Leptos admin/storefront packages;
- canonical write-path для visual builder через `body.format = "grapesjs_v1"`;
- typed relation `page_channel_visibility` для publication-level visibility.

## Интеграция

- использует `rustok-content` только для shared rich-text helpers, а не как storage backend;
- зависит от capability-модуля `rustok-page-builder` для FBA builder-contract (`preview/tree/properties/publish`) и соответствующих degraded/toggle профилей;
- использует `rustok-channel` для module-level и publication-level visibility contract;
- host applications подключают pages UI через manifest-driven generated wiring;
- `rustok-pages/admin` уже встраивает owner-side page SEO panel через `rustok-seo-admin-support`
  и shared capability contract модуля `rustok-seo`;
- block endpoints остаются migration-compatible surface и не должны неявно синтезировать `body`; legacy `blocks` считаются read/bridge совместимостью для visual-builder rollout: import/create сохраняется, но `grapesjs_v1` body writes не удаляют blocks и не расширяют block write surface;
- FBA rollout policy для builder capability layer хранится в `rustok-module.toml`: `control_plane_builder_wave_audit`, before/after snapshots, keep/rollback decision, owner sign-off, SLO rollback triggers и pilot smoke `preview -> properties -> publish(dry)`.

## Проверка

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:pages:legacy-bridge`
- targeted tests для page/block/menu flows, grapesjs body contract и channel visibility semantics

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
