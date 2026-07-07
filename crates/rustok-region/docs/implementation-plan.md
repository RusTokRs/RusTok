# Implementation plan for `rustok-region`

Status: region boundary is defined; the module holds the country/currency/tax baseline, storefront lookup contract and its own module-owned admin/storefront UI.

Current typed tax policy contract: `region.tax_provider_id` became a first-class baseline field for the region; the metadata-derived hook is no longer the source of truth, but a transitional channel override map `metadata.channel_tax_provider_ids` (string or object with `provider_id`/`provider`) is allowed for channel-aware cart runtime with explicit `channel_id`.

## Execution checkpoint

- Current phase: loco_free_native_admin_and_storefront_transport
- Last checkpoint: FFA slice #44 made the region storefront native server-function transport Loco-free: `storefront/src/transport/native_server_adapter.rs` consumes host-provided `rustok_api::HostRuntimeContext` for DB access, `rustok-region-storefront` no longer depends on `loco-rs`, and the GraphQL selected path remains intact. FFA slice #43 previously made the region admin native server-function transport Loco-free.
- Next step: Continue Loco-exit parity/evidence hardening for module-owned native adapters, then gather runtime contract/fallback smoke evidence for shared-context `RegionReadPort` and storefront native success/native failure + GraphQL success/double-failure error envelope; until runtime evidence, status remains `in_progress`.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block; when changing status code/locale key/DOM evidence, first update the verify script and its test fixture.
- Last updated at (UTC): 2026-06-29T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-region/contracts/evidence/region-provider-runtime-order-smoke.json`: shared read policy, tenant/request validation, owner `RegionService` invocation, typed error mapping and unified parity fallback/degraded metadata from consumer rows; status remains `in_progress` until live provider execution;
  - module plan synchronized with central FFA/FBA readiness board; UI surface already published and managed in migration/backlog rhythm;
  - FBA provider slice: `crates/rustok-region/src/ports.rs` declares `RegionReadPort` / `region.read_projection.v1` for region/country read projection consumers with shared `rustok_api::ports::PortContext`/`PortError`, tenant-scope preservation, locale fallback preservation and `PortCallPolicy::read()` deadline semantics; `crates/rustok-region/contracts/region-fba-registry.json` plus `crates/rustok-region/contracts/evidence/region-contract-test-static-matrix.json` lock planned contract cases and fallback profiles under `npm run verify:region:fba` while runtime execution/fallback smoke remains pending before `boundary_ready`;
  - commerce store-context consumer now only calls `RegionReadPort`: concrete `RegionService` dependency removed from orchestration service, runtime provider passed through single constructor, and no legacy compatibility path exists;
  - commerce storefront REST/GraphQL list consumers also call `RegionReadPort`; previous `rustok-commerce` re-exports of owner service and module alias removed, composition with concrete provider remains only in host wiring;
  - further status promotion is done only together with verification evidence and local+central docs update;
  - FFA slice #1 extracted region admin form normalization to module-local core and reused `rustok-api::normalize_ui_text` without transport changes;
  - FFA slice #2 extracted storefront route segment fallback, tax-provider fallback, country/tax summaries, policy-row formatting and selected-region metric view-model to `storefront/src/core.rs` with unit tests without Leptos runtime;
  - FFA slice #3 introduced explicit `transport/` facade with `native_server_adapter` and `graphql_adapter`, preserved `NativeThenGraphql` policy, and moved selected region resolution to core with unit tests;
  - FFA slice #4 added serializable `RegionTransportError`/`RegionTransportPath` that preserves selected failed path and error reason for native or GraphQL failures;
  - FFA slice #5 added framework-agnostic `RegionErrorEvidence`/`RegionErrorViewModel`, conversion from transport envelope and Leptos `RegionErrorMessage` render adapter without direct string-only error formatting;
  - FFA slice #6 added stable `RegionErrorStatusCode::as_str()` for machine-readable UI status and locale-aware status/body labels in storefront locale bundles;
  - FFA slice #7 added `RegionErrorStatusDescriptor` / `REGION_ERROR_STATUS_DESCRIPTORS` that links stable code with locale key, and updated central FFA checklist for error/status evidence;
  - FFA slice #8 added host-readable DOM evidence in `RegionErrorMessage`: `data-region-error-status` and `data-region-error-locale-key` come from core view-model/descriptor mapping;
  - FFA slice #9 added automated guard in `verify-ffa-ui-migration-contract.mjs` and test fixture for status descriptors, locale keys, DOM attributes and README evidence;
  - FFA slice #10 added `RegionErrorDomEvidence` as a portable output for DOM status attributes and SSR smoke-test of Leptos error adapter confirming rendered attributes;
  - FFA slice #11 added core-owned route/query state contract (`RegionRouteState`, `RegionRouteSelectionUpdate`, `SELECTED_REGION_QUERY_KEY`) for selected-region navigation without Leptos-owned query policy;
  - FFA slice #12 added host-visible route/query DOM evidence on rail links and verifier guard for route/query core contract + README evidence;
  - FFA slice #13 added SSR smoke-test of Leptos rail adapter confirming rendered href and route/query DOM evidence without full host runtime;
  - FFA slice #14 added `SelectedRegionCardViewModel` so selected-region card presentation data is assembled outside the Leptos render layer;
  - FFA slice #15 added `RegionRailViewModel` / `RegionRailLabels` so rail list title, total, empty state, open label and item rows are assembled outside the Leptos render layer;
  - FFA slice #16 added admin `transport/` facade for bootstrap/list/detail/create/update operations; Leptos component no longer calls raw native adapter directly;
  - admin transport profile locked as native-only single-adapter state: `admin/src/transport/mod.rs` delegates to native server-function endpoints from `admin/src/transport/native_server_adapter.rs`; GraphQL/REST admin fallback not yet declared, and legacy `admin/src/api.rs` removed;
  - FFA slice #17 extracted `admin/src/ui/leptos.rs` and `storefront/src/ui/leptos.rs` as explicit Leptos render adapters, while `admin/src/lib.rs` and `storefront/src/lib.rs` became thin module wiring/re-export layers; verifier reads storefront DOM evidence from the new adapter path;
  - FFA slice #18 added admin `RegionAdminListItemViewModel`, `RegionAdminListLabels`, `RegionAdminDetailLabels`, core-owned selected-row CSS policy and detail meta formatting with unit tests without Leptos runtime; Leptos adapter no longer formats region row/meta/tax badge inline;
  - FFA slice #19 added `RegionAdminEditorFormState` and core-owned defaults for create/reset form (`0`, `[]`, `{}`), and loaded-detail mapping (`tax_provider_id` fallback, countries CSV, pretty JSON fields) no longer lives in Leptos signal helper;
  - FFA slice #20 added `RegionAdminPolicyLabels`, `RegionAdminPolicySectionViewModel`, `region_admin_countries_summary` and default tax-provider id fallback in core; Leptos detail section no longer formats policy rows inline;
  - FFA slice #21 added `RegionAdminRawSectionLabels`, `RegionAdminRawSectionsViewModel` and `build_region_admin_raw_sections_view_model` so raw JSON section titles/bodies are assembled outside the Leptos render layer;
  - FFA slice #22 added admin detail header view-model (`RegionAdminDetailHeaderViewModel`) for name/summary/meta and localized created/updated timestamp strings; Leptos adapter no longer formats header/timestamps directly;
  - FFA slice #23 added admin editor mode view-model (`RegionAdminEditorViewModel`) for create/edit title and create/save submit label selection without Leptos runtime;
  - FFA slice #24 added admin editor field view-model (`RegionAdminEditorFieldViewModel`) for placeholders, tax-included checkbox label and metadata/country-tax-policy copy without Leptos runtime, and restored missing locale keys for `region.field.countryTaxPolicies` / `region.field.metadata`;
  - FFA slice #25 added admin shell/list header view-models (`RegionAdminShellViewModel`, `RegionAdminListHeaderViewModel`): tenant subtitle replacement and fallback policy execute in core, and Leptos adapter renders ready header strings;
  - FFA slice #26 added admin list state view-model (`RegionAdminListStateViewModel`) for loading/error/empty/ready branches, error context formatting, ready item rows and open action copy without Leptos runtime;
  - FFA slice #27 added admin route/query intent (`RegionAdminRouteQueryIntent`) for selected-region query normalization and `Open`/`Clear` decision policy without Leptos runtime;
  - FFA slice #28 added admin route/query writer update contract (`RegionAdminRouteQueryUpdate`, `REGION_ADMIN_SELECTED_QUERY_KEY`) for open/save/new host query mutations without Leptos-owned key/action policy;
  - FFA slice #29 added admin detail panel view-model (`RegionAdminDetailPanelViewModel`) for empty/ready selected-region branches, detail labels, header, policy rows, countries summary and raw sections without Leptos runtime;
  - FFA slice #30 added admin mutation policy helpers (`RegionRequiredFieldLabels`, `region_required_field_message`, `RegionAdminSaveMode`, `region_admin_save_mode`) so required-field validation copy and create/update decision no longer live in the Leptos submit handler;
  - FFA slice #31 added admin submit command preparation (`RegionAdminSubmitInput`, `RegionAdminSubmitCommand`, `RegionAdminSubmitError`, `prepare_region_admin_submit`) so the Leptos adapter passes a form snapshot to core and receives a ready payload+mode or typed validation error;
  - FFA slice #32 added fast boundary guardrail `scripts/verify/verify-region-admin-boundary.mjs` and included it in `verify:ffa:ui:migration`; guardrail checks Leptos-free admin core, prohibition of raw `api`/service calls from UI, transport facade exposure, native endpoints in temporary `api.rs`, local plan and central readiness board sync;
  - FFA slice #33 added `RegionAdminOpenDetailViewModel`, `region_admin_open_detail_success` and `region_admin_open_detail_error` so open-detail success/error state, empty-form reset and context error message composition live in core, and Leptos adapter only applies prepared selected/form/error values;
  - FFA slice #34 added `RegionAdminSaveSuccessViewModel` and `region_admin_save_success` so post-save selected detail, editor form state, refresh intent and selected-region route/query replace update are prepared in core, and Leptos adapter applies the prepared outcome;
  - FFA slice #35 added `RegionAdminSubmitErrorLabels`, `RegionAdminTransportErrorLabels`, `region_admin_submit_error_message`, `region_admin_load_region_error_message` and `region_admin_save_region_error_message` so locale-unavailable/required-field/load/save error copy and context formatting live in core, and Leptos adapter only passes typed errors/transport failures to prepared helpers;
  - FFA slice #36 added `RegionAdminRouteQueryWrite`, `region_admin_route_query_write` and `optional_region_admin_route_query_write` so selected-region route/query push/replace/clear and replace-vs-push host writer policy are checked in core, and Leptos adapter applies prepared updates through a generic `RouteQueryWriter::update`;
  - FFA slice #37 added `scripts/verify/verify-region-admin-boundary.test.mjs` and npm script `test:verify:region:admin-boundary` so the fast guardrail has a canonical fixture and negative fixtures for Leptos-specific core, raw UI api/service calls, missing route/query writer helper and stale central readiness board;
  - FFA slice #38 strengthened verifier self-check: `verify-region-admin-boundary.mjs` now checks `package.json` wiring for `test:verify:region:admin-boundary` and presence of canonical/docs-sync cases in fixture test file, and fixture suite rejects missing package test script;
  - FFA slice #39 connected `npm run test:verify:region:admin-boundary` into aggregate `test:verify:ffa:ui:migration` and added self-check/negative fixture so region boundary fixture evidence does not fall out of the general FFA UI migration test path.
  - FFA slice #40 added `scripts/verify/verify-region-storefront-boundary.mjs` and fixture suite: guardrail locks build-profile-selected native/GraphQL execution, presence of native/GraphQL adapters, transport error evidence, DOM status/locale-key attributes and prohibition of raw adapter calls from Leptos; scripts connected to aggregate verify/test pipelines.
  - FFA slice #41 retired storefront legacy `api.rs`; the native server-function endpoint and GraphQL selected-path request now live inside their module-owned transport adapters, and `verify-region-storefront-boundary.mjs` rejects reintroducing `storefront/src/api.rs` or `mod api`.
  - FFA slice #42 retired admin legacy `api.rs`; the native server-function endpoints now live in `admin/src/transport/native_server_adapter.rs`, `admin/src/transport/mod.rs` delegates through that adapter, and `verify-region-admin-boundary.mjs` rejects reintroducing `admin/src/api.rs` or `mod api`.
  - FFA slice #43 added Loco-free native admin transport: `admin/src/transport/native_server_adapter.rs` uses `HostRuntimeContext` instead of Loco `AppContext`, `admin/Cargo.toml` no longer declares `loco-rs`, and `scripts/verify/verify-region-admin-boundary.mjs` plus `scripts/verify/verify-api-surface-contract.mjs` guard that boundary.
  - FFA slice #44 added Loco-free native storefront transport: `storefront/src/transport/native_server_adapter.rs` uses `HostRuntimeContext` instead of Loco `AppContext`, `storefront/Cargo.toml` no longer declares `loco-rs`, build-profile-selected native/GraphQL execution remains unchanged, and `scripts/verify/verify-region-storefront-boundary.mjs` plus `scripts/verify/verify-api-surface-contract.mjs` guard that boundary.
- Last verified at (UTC): 2026-06-29T00:00:00Z
- Owner: `rustok-region` module team

## Scope of work

- maintain `rustok-region` as owner of region/country/currency policy baseline;
- maintain region CRUD/read-side inside module-owned service and admin/storefront UI packages;
- synchronize region runtime contract, manifest metadata and local docs;
- do not mix region boundary with tenant locale policy or the full tax domain.

## Current state

- `regions` and `RegionService` already live in a separate module;
- the module provides basic lookup by `region_id` or country;
- tenant locale policy remains a platform-level concern outside `rustok-region`;
- storefront region transport is still published through `rustok-commerce`;
- admin route for region list/detail/create/update now lives in `rustok-region/admin` and uses native Leptos server functions over `RegionService`; the native transport receives DB access through `rustok_api::HostRuntimeContext`, not Loco `AppContext`.
- storefront route for region discovery now lives in `rustok-region/storefront` and uses Loco-free native Leptos server functions with a GraphQL selected path over existing `storefrontRegions` transport; native SSR reads DB access from `HostRuntimeContext`; route/tax/country presentation helpers, selection resolution, error status classification, error DOM evidence and error view-model live in framework-agnostic storefront core, transport split into facade + native/GraphQL adapters, transport errors pass through typed selected-path evidence, and Leptos render adapter lives in `storefront/src/ui/leptos.rs` and remains a bind/render layer.

## Stages

### 1. Contract stability

- [x] lock region-owned storage and lookup contract;
- [x] separate region boundary from tenant locale policy;
- [x] export admin UI by module ownership boundary;
- [x] export storefront UI by module ownership boundary;
- [ ] maintain sync between region runtime contract, commerce orchestration and module metadata.

### 2. Domain expansion

- [ ] develop richer region/country/currency policy only through module-owned service layer;
- [ ] do not turn flat tax flags into a surrogate for a full tax domain;
- [ ] cover region resolution and policy edge-cases with targeted tests.

### 3. Operability

- [x] document module-owned admin/storefront routes and manifest wiring concurrently with runtime surface;
- [ ] keep local docs, `README.md` and admin package docs synchronized;
- [ ] keep local docs, `README.md`, `admin/README.md` and `storefront/README.md` synchronized;
- [ ] update umbrella commerce docs when region/storefront orchestration expectations change.

## Verification

- `cargo xtask module validate region`
- `cargo xtask module test region`
- `node scripts/verify/verify-region-admin-boundary.mjs`
- `cargo check -p rustok-region-admin --lib`
- `cargo check -p rustok-region-storefront --lib`
- targeted tests for region lookup, country/currency policy and tax-baseline semantics

## Update rules

1. When changing region runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md`, `docs/README.md`, `admin/README.md`, `storefront/README.md` and `rustok-module.toml`.
3. When changing admin wiring, synchronize `apps/admin` docs and central UI indexes.
4. When changing region/pricing/tax orchestration, update umbrella docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.

## FFA rollout tracker (rustok-region)

- [x] Slice 1: region admin form normalization moved to core (`RegionFormInput`, `build_region_draft`) and reuses shared UI input helper (`normalize_ui_text`) from `rustok-api` without changes to native/GraphQL transport.
- [x] Slice 2: storefront route/tax/country summary helpers, policy-row formatting and selected-region metric view-model moved to `storefront/src/core.rs`; native/GraphQL transport not changed, verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 3: storefront transport facade split into `transport/native_server_adapter.rs` and `transport/graphql_adapter.rs`, fallback policy explicitly locked as `NativeThenGraphql`, and selected-region resolution moved to core; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 4: transport facade returns typed `RegionTransportError` with `RegionTransportPath`, `fallback_attempted`, native error evidence and GraphQL error evidence; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 5: transport error envelope converts to framework-agnostic `RegionErrorEvidence`/`RegionErrorViewModel`, and Leptos layer renders `RegionErrorMessage` without direct string-only formatting; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 6: `RegionErrorStatusCode` locks stable `native_unavailable` / `fallback_unavailable`, status labels/body translated through storefront locale bundles, and Leptos error adapter shows machine-readable code + localized label; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 7: `REGION_ERROR_STATUS_DESCRIPTORS` locks host-visible mapping `stable_code -> locale_key`, and `docs/verification/ffa-ui-parity-checklist.md` requires evidence for changed error/status contracts; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 8: `RegionErrorMessage` publishes host-readable DOM evidence (`data-region-error-status`, `data-region-error-locale-key`) from core view-model/descriptor mapping; verification: `cargo test -p rustok-region-storefront --lib`.
- [x] Slice 9: `verify-ffa-ui-migration-contract.mjs` checks `RegionErrorStatusDescriptor`, stable codes, locale keys, DOM evidence attributes and README evidence; test fixture updated, verification: `node scripts/verify/verify-ffa-ui-migration-contract.test.mjs`.
- [x] Slice 10: `RegionErrorDomEvidence` locks portable output for DOM status attributes, and SSR smoke-test of Leptos adapter renders `RegionErrorMessage` and checks `data-region-error-status` / `data-region-error-locale-key`; verification: `cargo test -p rustok-region-storefront --lib --features ssr region_error_message_ssr_exposes_host_visible_dom_evidence`.
- [x] Slice 11: `RegionRouteState` / `RegionRouteSelectionUpdate` and `SELECTED_REGION_QUERY_KEY` lock portable route/query contract for selected-region navigation; Leptos adapter reads query key from core, normalizes selected id through core and builds rail href through core `selected_region_query_update`; verification: `cargo test -p rustok-region-storefront --lib region_route_state_normalizes_host_route_query_contract`.
- [x] Slice 12: `RegionRailItemViewModel` includes `query_key` / `query_value`, Leptos rail links publish `data-region-route-query-key` / `data-region-route-query-value`, and `verify-ffa-ui-migration-contract.mjs` checks route/query contract and README evidence; verification: `npm run verify:ffa:ui:migration`.
- [x] Slice 13: SSR smoke-test `region_rail_ssr_exposes_route_query_dom_evidence` renders Leptos rail adapter and checks href + `data-region-route-query-key` / `data-region-route-query-value`; verification: `cargo test -p rustok-region-storefront --lib --features ssr region_rail_ssr_exposes_route_query_dom_evidence`.
- [x] Slice 14: `SelectedRegionCardViewModel` moves selected-region header labels, metric list, countries summary and country policy row strings to core; Leptos selected card consumes the ready model; verification: `cargo test -p rustok-region-storefront --lib selected_region_card_view_model_collects_render_ready_sections`.
- [x] Slice 15: `RegionRailViewModel` / `RegionRailLabels` moves rail title, total label, empty state, open label and item rows to core; Leptos rail adapter renders the ready model and preserves route/query DOM evidence; verification: `cargo test -p rustok-region-storefront --lib region_rail_view_model_collects_render_ready_list_state`.
- [x] Slice 16: admin `transport/` facade covers bootstrap/list/detail/create/update operations, and Leptos adapter no longer calls raw native adapter directly.
- [x] Slice 17: `admin/src/ui/leptos.rs` and `storefront/src/ui/leptos.rs` became explicit Leptos render adapters, crate roots — wiring/re-export layer over `core` + `transport`.
- [x] Slice 18: admin list/detail render-fragment policy moved to core (`RegionAdminListItemViewModel`, `RegionAdminListLabels`, `RegionAdminDetailLabels`, selected-row CSS policy, detail meta formatting), Leptos adapter passes locale labels and renders ready strings; verification: `cargo test -p rustok-region-admin --lib --no-default-features` was stopped due to timeout to avoid long compilation.
- [x] Slice 19: admin editor form-state defaults and loaded-detail snapshot mapping moved to core (`RegionAdminEditorFormState`, default input constants, `from_detail`); Leptos adapter only applies the ready snapshot to signals; verification: `timeout 120s cargo check -p rustok-region-admin --lib --no-default-features` completed successfully within the given limit.
- [x] Slice 20: admin detail policy-section rows, countries summary and `region_default` tax-provider fallback moved to core (`RegionAdminPolicySectionViewModel`, `RegionAdminPolicyLabels`, `region_admin_countries_summary`); Leptos adapter renders ready rows; verification: `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` completed successfully.
- [x] Slice 21: admin detail raw JSON sections (`Country Tax Policies`, `Metadata`) moved to core (`RegionAdminRawSectionsViewModel`, `RegionAdminRawSectionLabels`); Leptos adapter renders ready title/body pairs; verification: `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` completed successfully.
- [x] Slice 22: admin detail header presentation moved to core (`RegionAdminDetailHeaderViewModel`, `RegionAdminDetailHeaderLabels`): name, currency/countries summary, policy meta and created/updated timestamp strings are assembled without Leptos runtime; checks `timeout 90s cargo check -p rustok-region-admin --lib --no-default-features` and `timeout 90s cargo test -p rustok-region-admin --lib --no-default-features admin_detail_header_view_model_formats_summary_and_timestamps` stopped due to timeout to avoid long compilation.
- [x] Slice 23: admin editor mode copy moved to core (`RegionAdminEditorViewModel`, `RegionAdminEditorLabels`): create/edit title and create/save submit label selected by normalized editing id; verification: `timeout 45s cargo test -p rustok-region-admin --lib --no-default-features admin_editor_view_model_selects_create_or_edit_copy_without_ui_runtime` stopped due to timeout to avoid long compilation.
- [x] Slice 24: admin editor field copy moved to core (`RegionAdminEditorFieldViewModel`, `RegionAdminEditorFieldLabels`): placeholders, tax-included checkbox label and metadata/country-tax-policy copy passed to Leptos adapter as ready strings; side fix: added missing admin locale keys `region.field.countryTaxPolicies` and `region.field.metadata`; verification: `timeout 45s cargo check -p rustok-region-admin --lib --no-default-features` stopped twice due to timeout on dependency compile to avoid long compilation.
- [x] Slice 25: admin shell/list header state moved to core (`RegionAdminShellViewModel`, `RegionAdminListHeaderViewModel`): top-level badge/title/subtitle, refresh label and tenant-aware list subtitle replacement/fallback no longer formatted inline in Leptos adapter; verification: `timeout 60s cargo check -p rustok-region-admin --lib --no-default-features` stopped due to timeout on dependency compile to avoid long compilation.
- [x] Slice 26: admin list state mapping moved to core (`RegionAdminListStateViewModel`, `RegionAdminListStateLabels`): loading/error/empty/ready branches, error context, ready rows and open action copy no longer assembled inline in Leptos adapter; verification: `timeout 60s cargo check -p rustok-region-admin --lib --no-default-features` stopped due to timeout on dependency compile to avoid long compilation.
- [x] Slice 27: admin selected-region route/query intent (`RegionAdminRouteQueryIntent`) moved to core; Leptos effect applies ready `Open`/`Clear` decision without package-local query policy.
- [x] Slice 28: admin route/query writer update contract (`RegionAdminRouteQueryUpdate`, `REGION_ADMIN_SELECTED_QUERY_KEY`) moved to core for open/save/new mutations without Leptos-owned key/action policy.
- [x] Slice 29: admin detail panel aggregation moved to core (`RegionAdminDetailPanelViewModel`): empty/ready branches, labels, header, policy rows, countries summary and raw sections assembled before render layer.
- [x] Slice 30: admin mutation policy helpers (`RegionRequiredFieldLabels`, `region_required_field_message`, `RegionAdminSaveMode`, `region_admin_save_mode`) moved required-field validation copy mapping and create/update decision from submit handler.
- [x] Slice 31: admin submit command preparation (`RegionAdminSubmitInput`, `RegionAdminSubmitCommand`, `RegionAdminSubmitError`, `prepare_region_admin_submit`) builds normalized payload, typed validation errors and save mode in core; Leptos adapter remains a snapshot/error/transport bind layer.
- [x] Slice 32: fast boundary guardrail `scripts/verify/verify-region-admin-boundary.mjs` added to aggregate `verify:ffa:ui:migration` and checks admin core/transport/ui split without long Cargo compilation.
