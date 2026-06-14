# План реализации `rustok-iggy-connector`

Статус: connector abstraction уже отделена от transport crate; дальнейшая
работа связана с hardening реального SDK/lifecycle path и удержанием чистой
границы ответственности.

## Execution checkpoint

- Current phase: lifecycle_hardening
- Last checkpoint: no-compile cross-module инкремент из rustok-iggy: `is_connected()` для remote/embedded connector теперь читает фактический connection state вместо постоянного `false`, чтобы transport health/debug surface отражал lifecycle.
- Next step: формализовать subscriber metadata contract для offset/ack/retry semantics, нужный rustok-iggy DLQ/replay movement.
- Open blockers: compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Начать с расширения `MessageSubscriber` результата или companion metadata DTO без втягивания transport-level semantics в connector crate.
- Last updated at (UTC): 2026-06-14T00:00:00Z

## Область работ

- удерживать `rustok-iggy-connector` как low-level connector layer;
- синхронизировать mode switching, lifecycle contracts и local docs;
- не допускать втягивания transport-level semantics в connector crate.

## Текущее состояние

- `IggyConnector`, remote/embedded implementations и config model уже существуют;
- optional `iggy` feature уже служит seam для реальной SDK integration;
- request building, mode serialization и error handling уже выделены в отдельный crate;
- `rustok-iggy` использует этот crate как низкоуровневый dependency.

## Этапы

### 1. Contract stability

- [x] закрепить connector boundary отдельно от transport crate;
- [x] удерживать embedded/remote mode abstraction внутри connector crate;
- [x] удерживать sync между connector contracts, `rustok-iggy` expectations и local docs.

### 2. Lifecycle hardening

- [ ] довести full SDK integration path, reconnection и pooling semantics;
  - [x] исправить lifecycle read surface `is_connected()` для remote/embedded connectors;
  - [ ] добавить subscriber metadata для offset/ack/retry без transport policy;
- [ ] покрывать batching, TLS и real connection failure cases targeted tests;
- [ ] удерживать simulation mode как явный documented compatibility path.

### 3. Operability

- [ ] развивать health/metrics/runbook guidance для connector layer;
- [ ] удерживать local docs синхронизированными с transport docs;
- [ ] документировать lifecycle guarantees одновременно с изменением connector surface.

## Проверка

- targeted compile/tests для configuration, mode switching, request building и connector errors;
- integration tests для real embedded/remote paths;
- docs sync между connector и transport crates.
- контрактные тесты покрывают все публичные use-case connector surface.

## Правила обновления

1. При изменении connector contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении transport boundary обновлять связанные docs в `rustok-iggy`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
