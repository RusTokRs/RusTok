# План реализации `rustok-media`

Статус: базовый media runtime уже работает; локальная документация выровнена и
модуль удерживается в scoped audit path.

## Execution checkpoint

- Current phase: владелец GraphQL-поля media в `rustok-media`
- Last checkpoint: REST upload/list/get/delete/translation handlers теперь принимают узкий `MediaHttpRuntime` с явными DB/storage handles; текущий Loco `AppContext` изолирован в route-state adapter до полного Axum cutover. GraphQL-поле `mediaUsage` и DTO `MediaUsageStats` перенесены из `apps/server::SystemQuery` в `rustok-media::graphql::MediaQuery`; сервер оставлен только точкой композиции схемы. Граница закреплена `apps/server/tests/module_surface_boundary_guard.rs` без компиляции.
- Next step: продолжить вынос оставшихся GraphQL-артефактов модулей из сервера; для Flex нужен отдельный runtime-handle поверх `FieldDefinitionCachePort`, `FlexStandaloneService` и публикации событий до удаления `apps/server/src/graphql/flex`.
- Open blockers: compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: держать `MediaImageDescriptor` единственным image payload для cross-module SEO/runtime интеграций; admin UI должен идти через `core` + `transport`, Leptos-only код оставлять в `ui/leptos.rs`, а transport-specific код — в dedicated adapter files.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный owner gate `scripts/verify/verify-owner-fba-runtime-order.mjs` проверяет `crates/rustok-media/contracts/evidence/media-provider-runtime-order-smoke.json`: shared read policy helper, tenant/list validation order, owner `MediaService` invocation, typed error mapping, descriptor materialization и parity пяти degraded modes; статус остаётся `in_progress` до live provider execution;
  - module plan синхронизирован с central FFA/FBA readiness board; media admin surface уже опубликован и ведётся в migration/backlog ритме;
  - FFA admin slice: `admin/src/core.rs` владеет Leptos-free form/presentation/state helpers (`non_empty_option`, dimensions label, pagination label, translation form state, usage stat cards, upload success state, busy-key policy, detail-line/list-card view-models и context-error message policy) с unit tests;
  - `admin/src/transport/` владеет текущим native-first + GraphQL fallback + REST upload transport facade без изменения внешних GraphQL/REST contracts; facade split зафиксирован через `graphql_adapter.rs`, `rest_adapter.rs` и `native_server_adapter.rs`;
  - `admin/src/ui/leptos.rs` является явным Leptos render adapter, а crate root только связывает модули и реэкспортирует `MediaAdmin`;
  - runtime hardening slice добавил service-level cleanup report/decision helpers и targeted unit coverage для upload policy + storage cleanup classification без transport changes;
  - граница владения GraphQL: `MediaQuery::media_usage` и `MediaUsageStats` живут в `crates/rustok-media/src/graphql`; `apps/server::SystemQuery` больше не импортирует `rustok_media`; server boundary guard проверяет это без компиляции;
  - FBA provider metadata now exposes the media asset read boundary through `MediaAssetReadPort` / `media.asset_read.v1`: `crates/rustok-media/contracts/media-fba-registry.json`, `crates/rustok-media/contracts/evidence/media-contract-test-static-matrix.json`, source-locked fallback smoke `crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json`, source-locked typed error matrix `crates/rustok-media/contracts/evidence/media-port-error-matrix.json` and `scripts/verify/verify-media-fba.mjs` lock shared `PortCallPolicy::read()` deadline semantics, tenant UUID context validation, typed `PortError` retryability, SEO descriptor fallback/degraded profiles, storage-relative proxy policy and consumer metadata without promoting beyond `in_progress` before executable runtime smoke.

## Область работ

- удерживать `rustok-media` как domain-owned media module поверх `rustok-storage`;
- синхронизировать upload/translation/storage contracts и local docs;
- развивать admin/runtime surfaces без размывания ownership между модулем и host wiring.

## Текущее состояние

- `MediaService`, entities, DTOs и transport adapters уже реализованы;
- `load_media_usage_snapshot` используется owner-owned полем `MediaQuery::media_usage`, а
  `apps/server::SystemQuery` больше не содержит media resolver/DTO/imports;
- media metadata хранится в module-owned tables, а бинарные файлы остаются в `rustok-storage`;
- upload остаётся REST-first path, GraphQL покрывает read/write flows без multipart semantics;
- module-owned admin UI и observability surface уже входят в модульный contract;
- `MediaAssetSummary` введён для kind/usage классификации без raw blob coupling; typed `MediaImageDescriptor` введён как cross-module boundary для SEO image payload (`url/alt/size/mime` + derived helpers), дополнен delivery profile policy (`absolute/root-relative public URL`, `storage-relative path`, `opaque reference`) и public URL policy (`direct public`, `proxy required`, `not addressable`), покрыт edge-case normalization tests для explicit MIME, invalid dimensions, query/fragment cleanup и proxy-required storage paths.

## Этапы

### 1. Contract stability

- [x] зафиксировать upload/list/delete/translation runtime contract;
- [x] удерживать tenant isolation и MIME/size validation внутри модуля;
- [x] держать media storage metadata и physical storage boundary явными;
- [~] удерживать sync между runtime contracts, admin UI и module metadata; текущий FFA admin slice вынес Leptos-free helpers в `admin/src/core.rs`, включая upload/detail/list-card/error state policy, transport facade в `admin/src/transport/`, явный render adapter в `admin/src/ui/leptos.rs` и fast boundary guardrail `scripts/verify/verify-media-admin-boundary.mjs`; FBA contract sync дополнительно закреплён `media-port-error-matrix.json` и проверкой `verify-media-fba.mjs`.

### 2. Runtime hardening

- [~] покрыть cleanup task, storage failures и translation edge-cases targeted integration tests; translation boundary имеет unit coverage для locale/text normalization, upload policy и cleanup probe classification покрыты service-level unit tests, DB-backed cleanup integration остаётся открытым;
- [ ] развивать richer metadata/use-case surfaces только через module-owned service layer; текущий no-compile slice добавил `get_asset_summary`/`list_asset_summaries` и DTO-level `MediaAssetSummary`;
- [ ] уточнить long-term policy для public URLs и storage-driver-specific guarantees.

### 3. Operability

- [ ] удерживать Prometheus metrics и storage health semantics production-ready;
- [ ] документировать cleanup/invalidation/runbook guarantees вместе с runtime changes;
- [ ] синхронизировать local docs, README и manifest metadata при изменении module surface.

## Проверка

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests для upload policy, translation normalization/persistence, cleanup task classification и storage error handling

## Правила обновления

1. При изменении media runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении storage contract или admin UI ожиданий обновлять связанные docs в `rustok-storage` и host docs.


## Quality backlog

- [~] Актуализировать покрытие тестами по ключевым сценариям модуля: FBA static matrix, source-locked fallback smoke, public URL policy / asset summary static evidence и source-locked port error matrix закрыты; executable runtime smoke и DB-backed cleanup integration остаются открытыми.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
