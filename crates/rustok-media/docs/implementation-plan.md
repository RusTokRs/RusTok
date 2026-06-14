# План реализации `rustok-media`

Статус: базовый media runtime уже работает; локальная документация выровнена и
модуль удерживается в scoped audit path.

## Execution checkpoint

- Current phase: FFA-разделение admin ui/core/transport
- Last checkpoint: FFA core helpers расширены для upload/detail state: busy-key policy, upload success selection/refresh state и detail-line view-model теперь живут в `admin/src/core.rs`, а Leptos adapter только применяет готовые состояния; descriptor normalization покрыт дополнительными edge-case unit tests.
- Next step: Добрать интеграционные проверки owner-module SEO providers, которые используют descriptor contract, и расширить targeted runtime tests для cleanup/storage/translation edge-cases без изменения transport parity.
- Open blockers: нет.
- Hand-off notes for next agent: держать `MediaImageDescriptor` единственным image payload для cross-module SEO/runtime интеграций; admin UI должен идти через `core` + `transport`, Leptos-only код оставлять в `ui/leptos.rs`, а transport-specific код — в dedicated adapter files.
- Last updated at (UTC): 2026-06-14T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board; media admin surface уже опубликован и ведётся в migration/backlog ритме;
  - FFA admin slice: `admin/src/core.rs` владеет Leptos-free form/presentation/state helpers (`non_empty_option`, dimensions label, pagination label, translation form state, usage stat cards, upload success state, busy-key policy, detail-line view-model) с unit tests;
  - `admin/src/transport/` владеет текущим native-first + GraphQL fallback + REST upload transport facade без изменения внешних GraphQL/REST contracts; facade split зафиксирован через `graphql_adapter.rs`, `rest_adapter.rs` и `native_server_adapter.rs`;
  - `admin/src/ui/leptos.rs` является явным Leptos render adapter, а crate root только связывает модули и реэкспортирует `MediaAdmin`.

## Область работ

- удерживать `rustok-media` как domain-owned media module поверх `rustok-storage`;
- синхронизировать upload/translation/storage contracts и local docs;
- развивать admin/runtime surfaces без размывания ownership между модулем и host wiring.

## Текущее состояние

- `MediaService`, entities, DTOs и transport adapters уже реализованы;
- media metadata хранится в module-owned tables, а бинарные файлы остаются в `rustok-storage`;
- upload остаётся REST-first path, GraphQL покрывает read/write flows без multipart semantics;
- module-owned admin UI и observability surface уже входят в модульный contract;
- typed `MediaImageDescriptor` введён как cross-module boundary для SEO image payload (`url/alt/size/mime` + derived helpers) и покрыт edge-case normalization tests для explicit MIME, invalid dimensions, query/fragment cleanup.

## Этапы

### 1. Contract stability

- [x] зафиксировать upload/list/delete/translation runtime contract;
- [x] удерживать tenant isolation и MIME/size validation внутри модуля;
- [x] держать media storage metadata и physical storage boundary явными;
- [~] удерживать sync между runtime contracts, admin UI и module metadata; текущий FFA admin slice вынес Leptos-free helpers в `admin/src/core.rs`, включая upload/detail state policy, transport facade в `admin/src/transport/` и явный render adapter в `admin/src/ui/leptos.rs`.

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


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
