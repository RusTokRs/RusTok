# План реализации `rustok-storage`

Статус: storage abstraction baseline уже работает; дальнейшая работа связана с
удержанием backend boundary и аккуратным расширением backend-support matrix.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Область работ

- удерживать `rustok-storage` как shared storage abstraction layer;
- синхронизировать backend contracts, path-safety guarantees и local docs;
- не допускать размывания domain logic в storage layer.

## Текущее состояние

- `StorageBackend`, `UploadedObject` и `StorageService` уже составляют базовый contract;
- local backend уже реализован и используется платформой;
- path generation, public URL construction и basic health semantics уже являются частью live surface;
- будущие S3-compatible backends рассматриваются как additive extension, а не как повод ломать существующий contract.

## Этапы

### 1. Contract stability

- [x] закрепить единый storage backend contract;
- [x] удерживать path traversal protection и backend abstraction внутри crate;
- [ ] удерживать sync между storage surface, host wiring и local docs.

### 2. Backend expansion

- [ ] добавить production-grade внешние backends как additive feature-based extension;
- [ ] покрывать backend-specific failure semantics и config edge-cases targeted integration tests;
- [ ] удерживать public URL и deletion semantics совместимыми между backends.

### 3. Operability

- [ ] развивать storage health, metrics и runbook guidance вместе с backend expansion;
- [ ] удерживать local docs синхронизированными с `rustok-media` и host/runtime docs;
- [ ] документировать новые guarantees одновременно с изменением storage contract.

## Проверка

- structural verification для docs и storage boundary;
- targeted compile/tests при изменении `StorageBackend`, `StorageService` или config contracts;
- integration checks для backend implementations и health semantics.

## Правила обновления

1. При изменении storage contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `docs/README.md` и связанные consumer docs.
3. При изменении host/storage wiring ожиданий обновлять runtime docs потребителей.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
