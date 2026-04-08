# План реализации `rustok-iggy-connector`

Статус: connector abstraction уже отделена от transport crate; дальнейшая
работа связана с hardening реального SDK/lifecycle path и удержанием чистой
границы ответственности.

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
- [ ] удерживать sync между connector contracts, `rustok-iggy` expectations и local docs.

### 2. Lifecycle hardening

- [ ] довести full SDK integration path, reconnection и pooling semantics;
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

## Правила обновления

1. При изменении connector contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении transport boundary обновлять связанные docs в `rustok-iggy`.
