# План реализации `rustok-iggy`

Статус: transport baseline уже существует; основная работа дальше — не в
создании абстракции с нуля, а в доведении реального Iggy integration path до
production-grade уровня.

## Execution checkpoint

- Current phase: real_integration_hardening
- Last checkpoint: no-compile инкремент: transport consume path переключён на connector `recv_with_metadata()`, `ConsumedEvent` переносит offset/opaque ack metadata; fake-connector tests обновлены под metadata assertions.
- Next step: связать offset/ack metadata с DLQ/replay movement и real SDK ack override.
- Open blockers: compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Следующий инкремент должен связать metadata-bearing consume path с retry_from_dlq/replay и real SDK ack semantics.
- Last updated at (UTC): 2026-06-15T00:00:00Z

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
- [x] удерживать sync между transport contracts, connector expectations и local docs.

### 2. Real integration hardening

- [ ] довести full Iggy SDK integration path;
- [ ] закрыть реальные consumption, offset management, DLQ movement и replay flows;
  - [x] добавить первый transport-owned consume path поверх connector `subscribe` и serializer deserialize;
  - [ ] добавить offset/ack metadata и wire-up для DLQ/replay movement;
    - [x] consume path переносит connector offset/opaque ack metadata в `ConsumedEvent`;
- [ ] покрывать performance/recovery/security edge-cases targeted tests и drills.

### 3. Operability

- [ ] развивать metrics, health checks и runbooks для production transport usage;
- [ ] удерживать local docs синхронизированными с connector docs и event-system guidance;
- [ ] документировать transport guarantees одновременно с изменением runtime surface.

## Проверка

контрактные тесты покрывают все публичные use-case

- [ ] контрактные тесты покрывают все публичные use-case orchestration и surface contracts.
- targeted compile/tests для configuration, serialization, topology, consumer groups и replay/DLQ contracts (текущий no-compile инкремент добавил fake-connector unit coverage, запуск отложен);
- integration tests для реального Iggy backend path;
- docs sync между transport и connector layers.

## Правила обновления

1. При изменении transport contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении connector boundary обновлять связанные docs в `rustok-iggy-connector`.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля: добавлены roundtrip deserialize и consume_next fake-connector tests.
- [ ] Добавить DLQ/replay tests поверх offset/ack metadata после real SDK ack override.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
