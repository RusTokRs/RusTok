# План реализации `rustok-content`

Статус: content/domain separation завершён; модуль работает как shared
orchestration и rich-text/locale contract layer.

## Область работ

- удерживать `rustok-content` как shared helper/orchestration модуль, а не product storage owner;
- синхронизировать conversion semantics, canonical URL policy и local docs;
- не допускать возврата domain CRUD обратно в shared storage.

## Текущее состояние

- blog/forum/pages domain CRUD уже вынесены в собственные модули;
- `rustok-content` владеет orchestration service, audit/idempotency state и canonical URL mapping;
- shared locale fallback и rich-text validation уже являются каноническим контрактом для publishable content surfaces;
- module docs и runtime boundary уже отражают post-split роль.

## Этапы

### 1. Contract stability

- [x] закрыть storage split и убрать product-owned transport surfaces из live runtime;
- [x] зафиксировать rich-text, locale fallback и conversion contracts;
- [x] встроить RBAC/idempotency/input-safety в orchestration path;
- [ ] удерживать sync между orchestration contracts, event flows и module metadata.

### 2. Orchestration hardening

- [ ] держать canonical URL и alias semantics атомарными вместе с outbox/reindex flows;
- [ ] расширять conversion coverage только через явные bridge contracts;
- [ ] удерживать rich-text и locale invariants синхронизированными с доменными модулями.

### 3. Operability

- [ ] развивать runbooks и observability для orchestration incidents, partial failures и reindex drift;
- [ ] покрывать новые orchestration guarantees targeted integration tests;
- [ ] документировать изменения conversion policy одновременно с изменением runtime surface.

## Проверка

- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests для orchestration lifecycle, canonical URL policy, fallback chain и sanitize contracts

## Правила обновления

1. При изменении content/orchestration contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении shared rich-text/locale contracts обновлять также central docs и consumer-module references.
