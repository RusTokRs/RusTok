# План реализации `rustok-media`

Статус: базовый media runtime уже работает; локальная документация выровнена и
модуль удерживается в scoped audit path.

## Область работ

- удерживать `rustok-media` как domain-owned media module поверх `rustok-storage`;
- синхронизировать upload/translation/storage contracts и local docs;
- развивать admin/runtime surfaces без размывания ownership между модулем и host wiring.

## Текущее состояние

- `MediaService`, entities, DTOs и transport adapters уже реализованы;
- media metadata хранится в module-owned tables, а бинарные файлы остаются в `rustok-storage`;
- upload остаётся REST-first path, GraphQL покрывает read/write flows без multipart semantics;
- module-owned admin UI и observability surface уже входят в модульный contract.

## Этапы

### 1. Contract stability

- [x] зафиксировать upload/list/delete/translation runtime contract;
- [x] удерживать tenant isolation и MIME/size validation внутри модуля;
- [x] держать media storage metadata и physical storage boundary явными;
- [ ] удерживать sync между runtime contracts, admin UI и module metadata.

### 2. Runtime hardening

- [ ] покрыть cleanup task, storage failures и translation edge-cases targeted integration tests;
- [ ] развивать richer metadata/use-case surfaces только через module-owned service layer;
- [ ] уточнить long-term policy для public URLs и storage-driver-specific guarantees.

### 3. Operability

- [ ] удерживать Prometheus metrics и storage health semantics production-ready;
- [ ] документировать cleanup/invalidation/runbook guarantees вместе с runtime changes;
- [ ] синхронизировать local docs, README и manifest metadata при изменении module surface.

## Проверка

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests для upload policy, translations, cleanup task и storage error handling

## Правила обновления

1. При изменении media runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении storage contract или admin UI ожиданий обновлять связанные docs в `rustok-storage` и host docs.
