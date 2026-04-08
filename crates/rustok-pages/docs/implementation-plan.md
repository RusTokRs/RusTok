# План реализации `rustok-pages`

Статус: pages-owned storage и visual builder contract уже зафиксированы; модуль
удерживается в режиме steady-state hardening и rollout polish.

## Область работ

- удерживать `rustok-pages` как владельца page, block и menu runtime contract;
- синхронизировать visual builder semantics, visibility rules и local docs;
- не допускать возврата page read/write paths на shared storage.

## Текущее состояние

- pages, page bodies, blocks и menus уже работают на module-owned persistence;
- GraphQL/REST adapters и Leptos admin/storefront packages уже живут внутри модуля;
- `grapesjs_v1` зафиксирован как canonical visual page-builder write-path;
- visibility contract уже использует typed relation `page_channel_visibility`.

## Этапы

### 1. Contract stability

- [x] закрыть storage split для pages, blocks и menus;
- [x] зафиксировать builder contract `markdown | rt_json_v1 | grapesjs_v1`;
- [x] удерживать compatibility surface для legacy block-driven pages;
- [ ] удерживать sync между runtime contracts, UI packages и module metadata.

### 2. Product hardening

- [ ] удерживать GraphQL и REST surfaces синхронизированными при изменении page builder flows;
- [ ] развивать page/menu observability и write-path metrics при реальном operational pressure;
- [ ] документировать policy для authenticated/admin bypass и stricter visibility invariants, если она меняется.

### 3. Operability

- [ ] покрывать page/block/menu lifecycle targeted integration tests;
- [ ] документировать новые runtime guarantees одновременно с изменением visual builder и visibility contract;
- [ ] синхронизировать local docs, README и central references при изменении module boundary.

## Проверка

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- targeted tests для CRUD, blocks reorder, menus, builder round-trip и channel visibility

## Правила обновления

1. При изменении pages runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, UI wiring или visibility semantics синхронизировать `rustok-module.toml`.
4. При изменении shared rich-text expectations обновлять также связанные docs в `rustok-content`.
