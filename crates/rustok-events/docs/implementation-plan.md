# План реализации `rustok-events`

Статус: canonical ownership event contracts уже вынесена в отдельный модуль;
текущая работа — удерживать compatibility path и schema discipline без дрейфа.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Область работ

- удерживать `rustok-events` как единственный canonical source для event contracts;
- синхронизировать schema registry, envelope shape и local docs;
- не возвращать ownership event contracts обратно в `rustok-core`.

## Текущее состояние

- `DomainEvent` и `EventEnvelope` уже живут в `rustok-events`;
- `rustok-core::events` уже работает как compatibility re-export layer;
- внутренние `rustok-*` crates уже должны импортировать event contracts напрямую из `rustok-events`;
- schema coverage, versioning guidance и contract tests уже составляют базовый release gate.

## Этапы

### 1. Contract stability

- [x] вынести canonical ownership event contracts в отдельный crate;
- [x] сохранить compatibility path через `rustok-core::events`;
- [x] покрыть schema registry, validation и roundtrip contract tests;
- [ ] удерживать sync между event types, registry и consumer imports (tenant lifecycle family обновлена: добавлен `tenant.module.toggled`).

### 2. Release discipline

- [ ] довести documented release gate до устойчивого процесса вокруг schema changes;
- [ ] продолжать вычищать остаточные прямые импорты из compatibility path;
- [ ] документировать breaking/deprecating changes вместе с versioning plan.

### 3. Operability

- [ ] удерживать outbox/replay/reindex guidance синхронизированной с event contracts;
- [ ] синхронизировать local docs и `README.md` при изменении schema surface;
- [ ] расширять compatibility checks при появлении новых event families.

## Проверка

<!-- compatibility anchor: РєРѕРЅС‚СЂР°РєС‚РЅС‹Рµ С‚РµСЃС‚С‹ РїРѕРєСЂС‹РІР°СЋС‚ РІСЃРµ РїСѓР±Р»РёС‡РЅС‹Рµ use-case -->
- [ ] Contract tests cover public event-contract use cases.
- `cargo xtask module validate events`
- `cargo xtask module test events`
- targeted tests для schema coverage, validation, compatibility aliases и JSON roundtrip

## Правила обновления

1. При изменении event contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении event versioning policy обновлять связанные architecture/outbox docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
