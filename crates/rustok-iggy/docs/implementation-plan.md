# План реализации `rustok-iggy`

Статус: transport baseline уже существует; основная работа дальше — не в
создании абстракции с нуля, а в доведении реального Iggy integration path до
production-grade уровня.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Область работ

- удерживать `rustok-iggy` как transport crate поверх `rustok-iggy-connector`;
- синхронизировать serialization/topology/DLQ/replay contracts и local docs;
- не допускать смешивания transport logic с connector lifecycle.

## Текущее состояние

- `IggyTransport` уже реализует `EventTransport`;
- JSON/Postcard serialization, topology helpers, consumer groups, DLQ и replay abstractions уже выделены;
- connection mode switching и low-level I/O уже вынесены в `rustok-iggy-connector`;
- часть production-grade integration semantics по-прежнему требует углубления реального SDK path.

## Этапы

### 1. Contract stability

- [x] закрепить transport boundary поверх connector crate;
- [x] удерживать transport-facing abstractions внутри `rustok-iggy`;
- [ ] удерживать sync между transport contracts, connector expectations и local docs.

### 2. Real integration hardening

- [ ] довести full Iggy SDK integration path;
- [ ] закрыть реальные consumption, offset management, DLQ movement и replay flows;
- [ ] покрывать performance/recovery/security edge-cases targeted tests и drills.

### 3. Operability

- [ ] развивать metrics, health checks и runbooks для production transport usage;
- [ ] удерживать local docs синхронизированными с connector docs и event-system guidance;
- [ ] документировать transport guarantees одновременно с изменением runtime surface.

## Проверка

контрактные тесты покрывают все публичные use-case

- [ ] контрактные тесты покрывают все публичные use-case orchestration и surface contracts.
- targeted compile/tests для configuration, serialization, topology, consumer groups и replay/DLQ contracts;
- integration tests для реального Iggy backend path;
- docs sync между transport и connector layers.

## Правила обновления

1. При изменении transport contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении connector boundary обновлять связанные docs в `rustok-iggy-connector`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
