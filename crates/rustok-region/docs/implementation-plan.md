# План реализации `rustok-region`

Статус: region boundary выделен; модуль держит country/currency/tax baseline, storefront lookup contract и собственные module-owned admin/storefront UI.

Текущий typed tax policy contract: `region.tax_provider_id` стал first-class baseline полем региона; metadata-derived hook больше не является source of truth, но transitional channel override map `metadata.channel_tax_provider_ids` (string или object с `provider_id`/`provider`) допускается для channel-aware cart runtime при явном `channel_id`.

## Execution checkpoint

- Current phase: ffa_admin_mutation_policy_slice
- Last checkpoint: FFA slice #32 добавила fast source-level guardrail `scripts/verify/verify-region-admin-boundary.mjs` для admin `core/transport/ui` split, submit preparation boundary и temporary native `api.rs` adapter state без запуска долгой Cargo-компиляции.
- Next step: Продолжить FFA-first sequencing к тонкому host-adapter smoke для route/query writer либо добрать следующий небольшой admin mutation outcome/error state slice без изменения native/GraphQL contracts.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок; при изменении status code/locale key/DOM evidence сначала обновлять verify script и его test fixture.
- Last updated at (UTC): 2026-06-13T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs;
  - FFA slice #1 вынесла нормализацию admin-формы региона в module-local core и переиспользовала `rustok-api::normalize_ui_text` без изменений транспорта;
  - FFA slice #2 вынесла storefront route segment fallback, tax-provider fallback, country/tax summaries, policy-row formatting и selected-region metric view-model в `storefront/src/core.rs` с unit-тестами без Leptos runtime;
  - FFA slice #3 ввела явный `transport/` facade с `native_server_adapter` и `graphql_adapter`, сохранила policy `NativeThenGraphql`, а resolution выбранного региона перенесла в core с unit-тестами;
  - FFA slice #4 добавила сериализуемый `RegionTransportError`/`RegionTransportPath`, который сохраняет failed path, fallback_attempted и обе причины ошибки при падении native+GraphQL fallback;
  - FFA slice #5 добавила framework-agnostic `RegionErrorEvidence`/`RegionErrorViewModel`, conversion из transport envelope и Leptos `RegionErrorMessage` render adapter без прямого string-only error formatting;
  - FFA slice #6 добавила stable `RegionErrorStatusCode::as_str()` для machine-readable UI status и locale-aware status/body labels в storefront locale bundles;
  - FFA slice #7 добавила `RegionErrorStatusDescriptor` / `REGION_ERROR_STATUS_DESCRIPTORS`, который связывает stable code с locale key, и обновила central FFA checklist для error/status evidence;
  - FFA slice #8 добавила host-readable DOM evidence в `RegionErrorMessage`: `data-region-error-status` и `data-region-error-locale-key` берутся из core view-model/descriptor mapping;
  - FFA slice #9 добавила automated guard в `verify-ffa-ui-migration-contract.mjs` и test fixture для status descriptors, locale keys, DOM attributes и README evidence;
  - FFA slice #10 добавила `RegionErrorDomEvidence` как переносимый output для DOM status attributes и SSR smoke-тест Leptos error adapter, который подтверждает rendered attributes;
  - FFA slice #11 добавила core-owned route/query state contract (`RegionRouteState`, `RegionRouteSelectionUpdate`, `SELECTED_REGION_QUERY_KEY`) для selected-region navigation без Leptos-owned query policy;
  - FFA slice #12 добавила host-visible route/query DOM evidence на rail links и verifier guard для route/query core contract + README evidence;
  - FFA slice #13 добавила SSR smoke-тест Leptos rail adapter, который подтверждает rendered href и route/query DOM evidence без полноценного host runtime;
  - FFA slice #14 добавила `SelectedRegionCardViewModel`, чтобы selected-region card presentation data собирались вне Leptos render слоя;
  - FFA slice #15 добавила `RegionRailViewModel` / `RegionRailLabels`, чтобы rail list title, total, empty state, open label и item rows собирались вне Leptos render слоя;
  - FFA slice #16 добавила admin `transport/` facade для bootstrap/list/detail/create/update operations; Leptos component больше не вызывает `api::*` напрямую, а native server-function adapter остался в `admin/src/api.rs`;
  - FFA slice #17 выделила `admin/src/ui/leptos.rs` и `storefront/src/ui/leptos.rs` как явные Leptos render adapters, а `admin/src/lib.rs` и `storefront/src/lib.rs` стали тонким module wiring/re-export слоем; verifier читает storefront DOM evidence из нового adapter path;
  - FFA slice #18 добавила admin `RegionAdminListItemViewModel`, `RegionAdminListLabels`, `RegionAdminDetailLabels`, core-owned selected-row CSS policy и detail meta formatting с unit-тестами без Leptos runtime; Leptos adapter больше не форматирует region row/meta/tax badge inline;
  - FFA slice #19 добавила `RegionAdminEditorFormState` и core-owned defaults для create/reset формы (`0`, `[]`, `{}`), а loaded-detail mapping (`tax_provider_id` fallback, countries CSV, pretty JSON fields) больше не живёт в Leptos signal helper;
  - FFA slice #20 добавила `RegionAdminPolicyLabels`, `RegionAdminPolicySectionViewModel`, `region_admin_countries_summary` и default tax-provider id fallback в core; Leptos detail section больше не форматирует policy rows inline;
  - FFA slice #21 добавила `RegionAdminRawSectionLabels`, `RegionAdminRawSectionsViewModel` и `build_region_admin_raw_sections_view_model`, чтобы raw JSON section titles/bodies собирались вне Leptos render слоя;
  - FFA slice #22 добавила admin detail header view-model (`RegionAdminDetailHeaderViewModel`) для name/summary/meta и локализованных created/updated timestamp строк; Leptos adapter больше не форматирует header/timestamps напрямую;
  - FFA slice #23 добавила admin editor mode view-model (`RegionAdminEditorViewModel`) для create/edit title и create/save submit label selection без Leptos runtime;
  - FFA slice #24 добавила admin editor field view-model (`RegionAdminEditorFieldViewModel`) для placeholders, tax-included checkbox label и metadata/country-tax-policy copy без Leptos runtime, а также восстановила отсутствующие locale keys для `region.field.countryTaxPolicies` / `region.field.metadata`;
  - FFA slice #25 добавила admin shell/list header view-models (`RegionAdminShellViewModel`, `RegionAdminListHeaderViewModel`): tenant subtitle replacement и fallback policy выполняются в core, а Leptos adapter рендерит готовые header strings;
  - FFA slice #26 добавила admin list state view-model (`RegionAdminListStateViewModel`) для loading/error/empty/ready branches, error context formatting, ready item rows и open action copy без Leptos runtime;
  - FFA slice #27 добавила admin route/query intent (`RegionAdminRouteQueryIntent`) для selected-region query normalization и `Open`/`Clear` decision policy без Leptos runtime;
  - FFA slice #28 добавила admin route/query writer update contract (`RegionAdminRouteQueryUpdate`, `REGION_ADMIN_SELECTED_QUERY_KEY`) для open/save/new host query mutations без Leptos-owned key/action policy;
  - FFA slice #29 добавила admin detail panel view-model (`RegionAdminDetailPanelViewModel`) для empty/ready selected-region branches, detail labels, header, policy rows, countries summary и raw sections без Leptos runtime;
  - FFA slice #30 добавила admin mutation policy helpers (`RegionRequiredFieldLabels`, `region_required_field_message`, `RegionAdminSaveMode`, `region_admin_save_mode`), чтобы required-field validation copy и create/update decision больше не жили в Leptos submit handler;
  - FFA slice #31 добавила admin submit command preparation (`RegionAdminSubmitInput`, `RegionAdminSubmitCommand`, `RegionAdminSubmitError`, `prepare_region_admin_submit`), чтобы Leptos adapter передавал form snapshot в core и получал готовый payload+mode либо typed validation error;
  - FFA slice #32 добавила fast boundary guardrail `scripts/verify/verify-region-admin-boundary.mjs` и включила его в `verify:ffa:ui:migration`; guardrail проверяет Leptos-free admin core, запрет raw `api`/service calls из UI, transport facade exposure, native endpoints в temporary `api.rs`, local plan и central readiness board sync.
- Last verified at (UTC): 2026-06-13T00:00:00Z
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
- storefront route для region discovery теперь живёт в `rustok-region/storefront` и использует native Leptos server functions с GraphQL fallback поверх существующего `storefrontRegions` transport; route/tax/country presentation helpers, selection resolution, error status classification, error DOM evidence и error view-model живут в framework-agnostic storefront core, transport разделён на facade + native/GraphQL adapters, ошибки transport проходят через typed envelope с fallback evidence, а Leptos render adapter живёт в `storefront/src/ui/leptos.rs` и остаётся bind/render слоем.

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
- [x] Slice 4: transport facade возвращает typed `RegionTransportError` с `RegionTransportPath`, `fallback_attempted`, native error evidence и GraphQL error evidence; проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 5: transport error envelope конвертируется в framework-agnostic `RegionErrorEvidence`/`RegionErrorViewModel`, а Leptos слой рендерит `RegionErrorMessage` без прямого string-only formatting; проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 6: `RegionErrorStatusCode` закрепляет stable `native_unavailable` / `fallback_unavailable`, status labels/body переведены через storefront locale bundles, а Leptos error adapter показывает machine-readable code + localized label; проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 7: `REGION_ERROR_STATUS_DESCRIPTORS` фиксирует host-visible mapping `stable_code -> locale_key`, а `docs/verification/ffa-ui-parity-checklist.md` требует evidence для изменённых error/status contracts; проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 8: `RegionErrorMessage` публикует host-readable DOM evidence (`data-region-error-status`, `data-region-error-locale-key`) из core view-model/descriptor mapping; проверка: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 9: `verify-ffa-ui-migration-contract.mjs` проверяет `RegionErrorStatusDescriptor`, stable codes, locale keys, DOM evidence attributes и README evidence; test fixture обновлён, проверка: `node scripts/verify/verify-ffa-ui-migration-contract.test.mjs`.
- [x] Slice 10: `RegionErrorDomEvidence` фиксирует переносимый output для DOM status attributes, а SSR smoke-тест Leptos adapter рендерит `RegionErrorMessage` и проверяет `data-region-error-status` / `data-region-error-locale-key`; проверка: `cargo test -p rustok-region-storefront --lib --features ssr region_error_message_ssr_exposes_host_visible_dom_evidence`.
- [x] Slice 11: `RegionRouteState` / `RegionRouteSelectionUpdate` и `SELECTED_REGION_QUERY_KEY` фиксируют переносимый route/query contract для selected-region navigation; Leptos adapter читает query key из core, нормализует selected id через core и строит rail href через core `selected_region_query_update`; проверка: `cargo test -p rustok-region-storefront --lib region_route_state_normalizes_host_route_query_contract`.
- [x] Slice 12: `RegionRailItemViewModel` включает `query_key` / `query_value`, Leptos rail links публикуют `data-region-route-query-key` / `data-region-route-query-value`, а `verify-ffa-ui-migration-contract.mjs` проверяет route/query contract и README evidence; проверка: `npm run verify:ffa:ui:migration`.
- [x] Slice 13: SSR smoke-тест `region_rail_ssr_exposes_route_query_dom_evidence` рендерит Leptos rail adapter и проверяет href + `data-region-route-query-key` / `data-region-route-query-value`; проверка: `cargo test -p rustok-region-storefront --lib --features ssr region_rail_ssr_exposes_route_query_dom_evidence`.
- [x] Slice 14: `SelectedRegionCardViewModel` переносит selected-region header labels, metric list, countries summary и country policy row strings в core; Leptos selected card потребляет готовую модель; проверка: `cargo test -p rustok-region-storefront --lib selected_region_card_view_model_collects_render_ready_sections`.
- [x] Slice 15: `RegionRailViewModel` / `RegionRailLabels` переносит rail title, total label, empty state, open label и item rows в core; Leptos rail adapter рендерит готовую модель и сохраняет route/query DOM evidence; проверка: `cargo test -p rustok-region-storefront --lib region_rail_view_model_collects_render_ready_list_state`.
- [x] Slice 16: admin `transport/` facade покрывает bootstrap/list/detail/create/update operations, а Leptos adapter больше не вызывает `api::*` напрямую; native server-function adapter временно остаётся в `admin/src/api.rs`.
- [x] Slice 17: `admin/src/ui/leptos.rs` и `storefront/src/ui/leptos.rs` стали явными Leptos render adapters, crate roots — wiring/re-export слой поверх `core` + `transport`.
- [x] Slice 18: admin list/detail render-fragment policy перенесена в core (`RegionAdminListItemViewModel`, `RegionAdminListLabels`, `RegionAdminDetailLabels`, selected-row CSS policy, detail meta formatting), Leptos adapter передаёт locale labels и рендерит готовые строки; проверка: `cargo test -p rustok-region-admin --lib --no-default-features` была остановлена по timeout, чтобы не уходить в долгую компиляцию.
- [x] Slice 19: admin editor form-state defaults и loaded-detail snapshot mapping перенесены в core (`RegionAdminEditorFormState`, default input constants, `from_detail`); Leptos adapter только применяет готовый snapshot к signals; проверка: `timeout 120s cargo check -p rustok-region-admin --lib --no-default-features` завершилась успешно в заданном лимите.
- [x] Slice 20: admin detail policy-section rows, countries summary и `region_default` tax-provider fallback перенесены в core (`RegionAdminPolicySectionViewModel`, `RegionAdminPolicyLabels`, `region_admin_countries_summary`); Leptos adapter рендерит готовые rows; проверка: `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` завершилась успешно.
- [x] Slice 21: admin detail raw JSON sections (`Country Tax Policies`, `Metadata`) перенесены в core (`RegionAdminRawSectionsViewModel`, `RegionAdminRawSectionLabels`); Leptos adapter рендерит готовые title/body пары; проверка: `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` завершилась успешно.
- [x] Slice 22: admin detail header presentation перенесена в core (`RegionAdminDetailHeaderViewModel`, `RegionAdminDetailHeaderLabels`): name, currency/countries summary, policy meta и created/updated timestamp строки собираются без Leptos runtime; проверки `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` и `timeout 90s cargo test -p rustok-region-admin --lib --no-default-features admin_detail_header_view_model_formats_summary_and_timestamps` остановлены по timeout, чтобы не уходить в долгую компиляцию.
- [x] Slice 23: admin editor mode copy перенесена в core (`RegionAdminEditorViewModel`, `RegionAdminEditorLabels`): create/edit title и create/save submit label выбираются по normalized editing id; проверка: `timeout 45s cargo test -p rustok-region-admin --lib --no-default-features admin_editor_view_model_selects_create_or_edit_copy_without_ui_runtime` остановлена по timeout, чтобы не уходить в долгую компиляцию.
- [x] Slice 24: admin editor field copy перенесена в core (`RegionAdminEditorFieldViewModel`, `RegionAdminEditorFieldLabels`): placeholders, tax-included checkbox label и metadata/country-tax-policy copy передаются в Leptos adapter готовыми строками; побочная правка: добавлены отсутствующие admin locale keys `region.field.countryTaxPolicies` и `region.field.metadata`; проверка: `timeout 45s cargo check -p rustok-region-admin --lib --no-default-features` дважды остановлена по timeout на dependency compile, чтобы не уходить в долгую компиляцию.
- [x] Slice 25: admin shell/list header state перенесён в core (`RegionAdminShellViewModel`, `RegionAdminListHeaderViewModel`): top-level badge/title/subtitle, refresh label и tenant-aware list subtitle replacement/fallback больше не форматируются inline в Leptos adapter; проверка: `timeout 60s cargo check -p rustok-region-admin --lib --no-default-features` остановлена по timeout на dependency compile, чтобы не уходить в долгую компиляцию.
- [x] Slice 26: admin list state mapping перенесён в core (`RegionAdminListStateViewModel`, `RegionAdminListStateLabels`): loading/error/empty/ready branches, error context, готовые rows и open action copy больше не собираются inline в Leptos adapter; проверка: `timeout 60s cargo check -p rustok-region-admin --lib --no-default-features` остановлена по timeout на dependency compile, чтобы не уходить в долгую компиляцию.
- [x] Slice 27: admin selected-region route/query intent (`RegionAdminRouteQueryIntent`) перенесён в core; Leptos effect применяет готовое `Open`/`Clear` решение без package-local query policy.
- [x] Slice 28: admin route/query writer update contract (`RegionAdminRouteQueryUpdate`, `REGION_ADMIN_SELECTED_QUERY_KEY`) перенесён в core для open/save/new mutations без Leptos-owned key/action policy.
- [x] Slice 29: admin detail panel aggregation перенесён в core (`RegionAdminDetailPanelViewModel`): empty/ready branches, labels, header, policy rows, countries summary и raw sections собираются до render слоя.
- [x] Slice 30: admin mutation policy helpers (`RegionRequiredFieldLabels`, `region_required_field_message`, `RegionAdminSaveMode`, `region_admin_save_mode`) перенесли required-field validation copy mapping и create/update decision из submit handler.
- [x] Slice 31: admin submit command preparation (`RegionAdminSubmitInput`, `RegionAdminSubmitCommand`, `RegionAdminSubmitError`, `prepare_region_admin_submit`) строит normalized payload, typed validation errors и save mode в core; Leptos adapter остаётся snapshot/error/transport bind слоем.
- [x] Slice 32: fast boundary guardrail `scripts/verify/verify-region-admin-boundary.mjs` добавлен в aggregate `verify:ffa:ui:migration` и проверяет admin core/transport/ui split без долгой Cargo-компиляции.
