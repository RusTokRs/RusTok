# План реализации `rustok-iggy-connector`

Статус: connector abstraction уже отделена от transport crate; дальнейшая
работа связана с hardening реального SDK/lifecycle path и удержанием чистой
границы ответственности.

## Execution checkpoint

- Current phase: lifecycle_hardening
- Last checkpoint: no-compile инкремент: добавлен `ConnectorAckToken` как единый simulated/real Iggy SDK ack seam; remote/embedded subscribers теперь source-level валидируют stream/topic/partition scope перед ack, а `verify-iggy-connector-source.mjs` фиксирует guardrail без компиляции.
- Next step: подключить `ConnectorAckToken::iggy_sdk` к фактическому SDK subscriber receive/commit path и заменить source-level evidence targeted cargo tests при разрешённых компиляциях.
- Open blockers: compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Сохранить opaque-token contract для transport consumers; при wiring real SDK извлекать offset/consumer cursor в `ConnectorAckToken::iggy_sdk`, не протаскивая retry/DLQ/replay policy в connector crate.
- Last updated at (UTC): 2026-06-20T14:30:00Z

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
  - [x] добавить subscriber metadata для offset/ack/retry без transport policy;
  - [x] добавить explicit ack override seam для remote/embedded subscriber adapters;
  - [x] централизовать simulated ack token builder для remote/embedded metadata;
  - [x] добавить `ConnectorAckToken` seam для simulated и real Iggy SDK ack cursor с source-level scope validation;
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

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля: добавлены unit assertions для subscriber metadata/message builders (запуск отложен без компиляций).
- [x] Проверить полноту и актуальность `README.md` и локальных docs: README/docs/CRATE_API описывают metadata surface.
- [x] Зафиксировать source-level assertions для canonical simulated ack tokens (запуск отложен без компиляций).
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля: `node scripts/verify/verify-iggy-connector-source.mjs` (no-compile) и `cargo test -p rustok-iggy-connector --lib` при разрешённых компиляциях.
- [ ] Подключить real SDK subscriber receive/ack path к `ConnectorAckToken::iggy_sdk` и заменить source-level guardrail фактическими targeted tests.
