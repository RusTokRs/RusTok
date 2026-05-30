# План реализации `rustok-region`

Статус: region boundary выделен; модуль держит country/currency/tax baseline, storefront lookup contract и собственные module-owned admin/storefront UI.

Текущий typed tax policy contract: `region.tax_provider_id` стал first-class baseline полем региона; metadata-derived hook больше не является source of truth, но transitional channel override map `metadata.channel_tax_provider_ids` (string или object с `provider_id`/`provider`) допускается для channel-aware cart runtime при явном `channel_id`.

## Execution checkpoint

- Current phase: ffa_storefront_transport_slice
- Last checkpoint: FFA slice #3 разделила storefront transport facade на `native_server_adapter` и `graphql_adapter`, а selection resolution перенесла в `storefront/src/core.rs`, чтобы adapters не владели state policy.
- Next step: Продолжить FFA-first sequencing следующим минимальным выделением typed transport error envelope/fallback evidence без нарушения native/GraphQL parity.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок; не возвращать selection/state policy в transport adapters и не менять порядок fallback без parity evidence.
- Last updated at (UTC): 2026-05-30T01:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs;
  - FFA slice #1 вынесла нормализацию admin-формы региона в module-local core и переиспользовала `rustok-api::normalize_ui_text` без изменений транспорта;
  - FFA slice #2 вынесла storefront route segment fallback, tax-provider fallback, country/tax summaries, policy-row formatting и selected-region metric view-model в `storefront/src/core.rs` с unit-тестами без Leptos runtime;
  - FFA slice #3 ввела явный `transport/` facade с `native_server_adapter` и `graphql_adapter`, сохранила policy `NativeThenGraphql`, а resolution выбранного региона перенесла в core с unit-тестами.
- Last verified at (UTC): 2026-05-30T01:00:00Z
- Owner: `rustok-region` module team

## Область работ

- удерживать `rustok-region` как owner region/country/currency policy baseline;
- удерживать region CRUD/read-side внутри module-owned service и admin/storefront UI packages;
- синхронизировать region runtime contract, manifest metadata и local docs;
- не смешивать region boundary с tenant locale policy или полноценным tax domain.

## Текущее состояние

- `regions` и `RegionService` уже живут в отдельном модуле;
- модуль задаёт базовый lookup по `region_id` или стране;
- tenant locale policy остаётся platform-level concern вне `rustok-region`;
- storefront region transport всё ещё публикуется через `rustok-commerce`;
- admin route для region list/detail/create/update теперь живёт в `rustok-region/admin` и использует native Leptos server functions поверх `RegionService`.
- storefront route для region discovery теперь живёт в `rustok-region/storefront` и использует native Leptos server functions с GraphQL fallback поверх существующего `storefrontRegions` transport; route/tax/country presentation helpers и selection resolution живут в framework-agnostic storefront core, transport разделён на facade + native/GraphQL adapters, а Leptos component остаётся bind/render слоем.

## Этапы

### 1. Contract stability

- [x] зафиксировать region-owned storage и lookup contract;
- [x] отделить region boundary от tenant locale policy;
- [x] вывести admin UI по ownership boundary модуля;
- [x] вывести storefront UI по ownership boundary модуля;
- [ ] удерживать sync между region runtime contract, commerce orchestration и module metadata.

### 2. Domain expansion

- [ ] развивать richer region/country/currency policy только через module-owned service layer;
- [ ] не превращать плоские tax flags в суррогат полноценного tax domain;
- [ ] покрывать region resolution и policy edge-cases targeted tests.

### 3. Operability

- [x] документировать module-owned admin/storefront routes и manifest wiring одновременно с runtime surface;
- [ ] удерживать local docs, `README.md` и admin package docs синхронизированными;
- [ ] удерживать local docs, `README.md`, `admin/README.md` и `storefront/README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении region/storefront orchestration expectations.

## Проверка

- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo check -p rustok-region-admin --lib`
- `cargo check -p rustok-region-storefront --lib`
- targeted tests для region lookup, country/currency policy и tax-baseline semantics

## Правила обновления

1. При изменении region runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `docs/README.md`, `admin/README.md`, `storefront/README.md` и `rustok-module.toml`.
3. При изменении admin wiring синхронизировать `apps/admin` docs и central UI indexes.
4. При изменении region/pricing/tax orchestration обновлять umbrella docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.

## FFA rollout tracker (rustok-region)

- [x] Slice 1: нормализация admin-формы региона перенесена в core (`RegionFormInput`, `build_region_draft`) и переиспользует shared UI input helper (`normalize_ui_text`) из `rustok-api` без изменений native/GraphQL транспорта.
- [x] Slice 2: storefront route/tax/country summary helpers, policy-row formatting и selected-region metric view-model перенесены в `storefront/src/core.rs`; native/GraphQL transport не изменён, проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 3: storefront transport facade разделён на `transport/native_server_adapter.rs` и `transport/graphql_adapter.rs`, fallback policy явно закреплена как `NativeThenGraphql`, а selected-region resolution перенесён в core; проверка: `cargo test -p rustok-region-storefront --lib`.
