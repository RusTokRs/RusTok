# План реализации `rustok-iggy`

Статус: transport baseline уже существует; основная работа дальше — не в
создании абстракции с нуля, а в доведении реального Iggy integration path до
production-grade уровня.

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

- targeted compile/tests для configuration, serialization, topology, consumer groups и replay/DLQ contracts;
- integration tests для реального Iggy backend path;
- docs sync между transport и connector layers.

## Правила обновления

1. При изменении transport contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении connector boundary обновлять связанные docs в `rustok-iggy-connector`.
