# План реализации `rustok-content`

Статус: content/domain separation завершён; модуль работает как shared
orchestration и rich-text/locale contract layer.

## Execution checkpoint

- Current phase: orchestration_hardening
- Last checkpoint: Добавлены targeted integration сценарии для canonical URL collision и alias-shadow rollback/no-outbox guarantees; compile-free guardrail теперь source-locks эти runtime evidence cases без запуска компиляции.
- Next step: Закрыть reindex drift evidence и расширить conversion bridge contract coverage без расширения shared storage ownership.
- Open blockers: Compile/runtime execution evidence still pending because this iteration intentionally avoided compilation.
- Hand-off notes for next agent: Поддерживать `npm run verify:content:orchestration` вместе с любым изменением `ContentOrchestrationService`, `CanonicalUrlService`, collision tests, local docs или registry row.
- Last updated at (UTC): 2026-06-21T00:00:00Z

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
- [x] удерживать sync между orchestration contracts, event flows и module metadata через compile-free guardrail `npm run verify:content:orchestration`.

### 2. Orchestration hardening

- [x] держать canonical URL и alias semantics атомарными вместе с outbox/reindex flows в статическом contract guardrail-е;
- [x] явно блокировать canonical URL collision и alias shadowing между разными targets до изменения mapping/outbox state;
- [x] добавить targeted integration evidence для canonical URL collision и alias shadowing rollback/no-outbox сценариев;
- [ ] расширять conversion coverage только через явные bridge contracts;
- [ ] удерживать rich-text и locale invariants синхронизированными с доменными модулями.

### 3. Operability

- [x] развивать runbooks и observability для orchestration incidents, partial failures и reindex drift: runbook теперь фиксирует verification gate `npm run verify:content:orchestration`;
- [x] покрыть canonical URL collision/alias shadowing guarantees targeted integration tests (source-locked без компиляции в этой итерации);
- [ ] покрывать следующие orchestration guarantees targeted integration tests;
- [ ] документировать изменения conversion policy одновременно с изменением runtime surface.

## Проверка

- [x] compile-free guardrail покрывает public orchestration use-case contracts, route resolution, canonical/alias collision guards, rollback/no-outbox evidence markers и docs/registry sync: `npm run verify:content:orchestration`
- [ ] контрактные тесты покрывают все публичные use-case orchestration и surface contracts
- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests для orchestration lifecycle, canonical URL policy, fallback chain и sanitize contracts

## Правила обновления

1. При изменении content/orchestration contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении shared rich-text/locale contracts обновлять также central docs и consumer-module references.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля (`npm run verify:content:orchestration`).
