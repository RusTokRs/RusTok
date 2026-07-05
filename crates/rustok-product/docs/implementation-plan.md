# План реализации `rustok-product`

Статус: product boundary выделен; модуль владеет каталогом и typed product data,
а transport и часть orchestration остаются у umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: product_fba_fixture_locked_no_compile
- Last checkpoint: product FBA remains `boundary_ready` on no-compile runtime fallback evidence. `product-runtime-fallback-smoke.json` and `verify-product-runtime-fallback-smoke.mjs` now lock read policy ordering, tenant scope, locale fallback, bounded storefront pagination, fallback profiles, typed `PortError` mapping, the prepared `ports::tests` harness, README/docs FBA boundary markers, product verification command docs, `package.json` aggregate wiring, `modules.toml`/`rustok-module.toml` metadata sync and central commerce-domain batch summary without Rust compilation; `verify-product-runtime-fallback-smoke.test.mjs` covers README/docs/source-marker drift, package aggregate drift, module metadata drift, premature `transport_verified` status drift and stale central batch-summary drift before live evidence. Next storefront host composition remains connected through `apps/next-frontend/src/features/search` and product-owned `fetchCatalogSearchOptions`.
- Dependency evidence: product storefront locale matching uses `rustok_api::locale_tags_match`; no-feature/hydrate profiles no longer contain `rustok-core`.
- Next step: Собрать live provider execution evidence перед повышением product FBA до `transport_verified`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-07-05T09:44:03Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` и fixture-regression suite проверяют `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`: read policy выполняется до owner `CatalogService`, затем применяется typed `PortError` mapping; fallback profiles/degraded modes сверяются с registry;
  - no-compile runtime fallback smoke `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json` + `scripts/verify/verify-product-runtime-fallback-smoke.mjs` source-locks product catalog read fallback behavior, bounded pagination validation, locale fallback, tenant scope, typed `PortError` mapping, `package.json` product/aggregate script wiring, `modules.toml` / `rustok-module.toml` metadata sync, central commerce-domain batch summary and the prepared `crates/rustok-product/src/ports.rs` unit-test harness without Rust compilation. Fixture regression test `scripts/verify/verify-product-runtime-fallback-smoke.test.mjs` is wired into `test:verify:ecommerce:fba` and covers package aggregate drift, module metadata drift, premature `transport_verified` registry/local-plan drift and stale central batch-summary drift. FBA status raised to `boundary_ready`; `transport_verified` still requires live provider execution evidence;
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - FBA slice: `crates/rustok-product/src/ports.rs` declares `ProductCatalogReadPort`/`product.catalog_read.v1` for catalog read projections consumed by commerce checkout/storefront compatibility paths, pricing enrichment and `ai-product` generation context; `crates/rustok-product/contracts/product-fba-registry.json`, `contracts/evidence/product-contract-test-static-matrix.json`, `contracts/evidence/product-runtime-contract-smoke.json` and `contracts/evidence/product-runtime-fallback-smoke.json` lock provider metadata, fallback profiles and no-compile runtime fallback behavior under `npm run verify:ecommerce:fba`; status remains below `transport_verified` until live runtime execution/fallback evidence lands;
  - umbrella facade `rustok_commerce::{services::catalog, CatalogService}` is removed; commerce/server/AI consumers import `CatalogService` from `rustok-product` directly, so product owner service is no longer masked by the ecommerce umbrella;
  - product DTOs are exposed through the owner surface `rustok_product::dto`; `rustok-commerce` no longer publicly re-exports product DTO aliases, and commerce/server/AI/test callers import product DTO contracts from the product owner crate;
  - product entities are exposed through `rustok_product::entities` for product, variant, translation, option and image tables; `rustok-commerce` no longer publicly re-exports these product entity aliases;
  - FFA slice: storefront catalog rail title/total/empty/open labels, item fallback labels, seller boundary text, published timestamp fallback and handle links now live in framework-agnostic `ProductCatalogRailViewModel` with unit-test evidence;
  - FFA slice: selected-product card empty state, pricing context label, ownership note, metric labels and pricing action label now live in `SelectedProductEmptyViewModel` / `SelectedProductViewModel` with unit-test evidence;
  - FFA slice: storefront shell badge/title/subtitle/load-error copy and typed fetch request shape now live in `ProductStorefrontShellViewModel` / `ProductStorefrontFetchRequest` with unit-test evidence;
  - FFA slice: storefront pricing-context sanitization/defaulting moved into core, native/GraphQL fetch adapters now sit behind `storefront/src/transport/`, legacy `storefront/src/api.rs` is removed, and Leptos rendering is isolated in `storefront/src/ui/leptos.rs`; evidence: `cargo test -p rustok-product-storefront --lib`;
  - FFA slice: storefront transport errors now keep serializable native/GraphQL fallback evidence (`ProductTransportError`, `ProductTransportPath`), core composes `ProductTransportErrorDomEvidence`, and the Leptos error adapter exposes stable `data-product-transport-*` attributes for host/parity smoke checks;
  - FFA slice: product admin list/status/filter, shipping-profile, pricing-preview and pricing deep-link helpers moved into `admin/src/core.rs`; Leptos admin remains the render/effect adapter while GraphQL transport stays unchanged for this slice;
  - FFA slice: product admin GraphQL operations now route through `admin/src/transport.rs`, with `admin/src/transport/graphql_adapter.rs` as the GraphQL adapter; legacy `admin/src/api.rs` is removed and forbidden by `verify-product-admin-boundary.mjs`, preserving the existing `rustok-commerce` GraphQL contract;
  - FFA/FBA slice: category-bound admin transport DTOs now live behind `admin/src/transport.rs`; native Leptos `#[server]` functions in `admin/src/transport/native_server_adapter.rs` are the default internal path, GraphQL operation documents remain parallel in `admin/src/transport/graphql_adapter.rs`, and server-side GraphQL query/mutation/type bindings in `rustok-commerce/src/graphql/*` call owner-owned `ProductCatalogSchemaService` for attributes, categories, reusable schemas, schema mode assignment, schema/category bindings and effective form preview; the fast guardrail rejects optional/fallback locale for these new localized catalog operations and checks native, GraphQL and server GraphQL markers;
  - FFA/FBA slice: product CRUD exposes nullable `primary_category_id`; the Leptos product editor loads only structural categories through the module transport facade, persists the selection through GraphQL create/update, and resolves category-first effective-form preview through the native-first/GraphQL-parallel contract using `UiRouteContext.locale`;
  - FFA/FBA slice: typed product attribute value reads and transactional patches are owner-owned by `ProductCatalogSchemaService`; native Leptos server functions remain the default admin path, GraphQL remains parallel, localized text uses only the explicit host locale, and the fast product boundary guardrail locks both surfaces;
  - FFA/FBA slice: publish validation is owner-owned by `ProductCatalogSchemaService`; `CatalogService::publish_product` rejects missing required effective attributes before status changes, text-like localized required values require an explicit non-empty translation row, option values require at least one option relation, detached values do not satisfy requirements outside the effective schema, and create-with-publish is rejected for categories with required typed attributes;
  - FFA/FBA slice: detached values are listed through the typed value read contract and cleared through owner-owned `clear_detached_product_attribute_values`; the service rejects non-detached ids, native `#[server]` remains primary, GraphQL remains parallel, and product admin renders review rows plus explicit clear-all action;
  - FFA/FBA slice: effective form options are loaded in one bounded query for effective attribute ids and localized by the host locale; effective group labels are resolved from schema/category group translations through the same explicit locale; schema/category group creation and `group_code` bindings are available through native/GraphQL admin contracts; `ProductAttributeEditorState` owns dirty tracking, typed parsing and explicit clear semantics outside Leptos, while `TypedProductAttributeField` renders grouped controls and submit persists values only after the product category is committed;
  - FFA/FBA slice: Next admin product package exposes owner-owned `listCatalogCategorySearchOptions` and `listCatalogAttributeSearchOptions` helpers for search host composition. They query `catalogCategories` / `productAttributes` through product GraphQL with the host effective locale, return category `id` values and filterable/sortable attribute `code` values, and keep search UI from importing product internals directly;
  - FFA/FBA slice: `rustok-product-admin` exposes owner-owned `fetch_catalog_search_options` plus neutral option DTOs for future Leptos host composition. The helper requires the host effective locale and reuses the product native-first/GraphQL-parallel transport facade; search packages remain consumers of host-provided metadata only;
  - FFA/FBA slice: `apps/admin` now composes product-owned catalog metadata into `SearchAdmin` through `SearchAdminComposition`; the product helper resolves the current tenant through its native `#[server]` endpoint first and keeps GraphQL in parallel, while the host supplies locale/auth/tenant context and checks product enablement;
  - FFA/FBA slice: `rustok-product-storefront` exposes a public-safe catalog search option contract through native `#[server]` first and `storefrontCatalogSearchOptions(locale: String!)` GraphQL fallback; `apps/storefront::SearchStorefrontComposition` supplies host locale, checks product enablement and maps only public owner DTOs into search props;
  - FFA/FBA slice: `apps/next-frontend/packages/rustok-product` exposes public-safe `fetchCatalogSearchOptions` over `storefrontCatalogSearchOptions(locale: String!)`; `apps/next-frontend/src/features/search` composes those owner DTOs into `SearchStorefrontPage` with route locale, tenant slug and product enablement from the host registry context, while the search package still imports no product internals;
  - i18n evidence: `verify-ui-i18n-parity.mjs` больше не исключает `rustok-product`; admin/storefront EN/RU bundles входят в общий `npm run verify:i18n:ui` gate;
  - FFA slice: product admin Leptos rendering moved under `admin/src/ui/leptos.rs`, and `admin/src/lib.rs` now acts as the module/re-export boundary for `ProductAdmin`;
  - FFA slice: selected product admin summary labels, pricing preview state and pricing deep-link are composed by `SelectedProductSummaryViewModel` in `admin/src/core.rs`, keeping Leptos summary rendering as markup-only;
  - FFA slice: product admin list-card display state (status label/badge, type fallback, meta label, shipping profile chip and published/created timestamp) is composed by `ProductAdminListItemViewModel` in `admin/src/core.rs`, keeping Leptos list rendering as markup/action binding only;
  - FFA slice: product admin editor shell state (create/edit mode, title, subtitle and submit label) is composed by `ProductAdminEditorViewModel` in `admin/src/core.rs`, keeping Leptos editor rendering as markup/action binding only;
  - FFA slice: product admin submit validation, locale/bootstrap guardrails, create/update mode selection and `ProductDraft` command preparation are composed by `ProductAdminSaveCommand` / `ProductAdminDraftForm` in `admin/src/core.rs`; Leptos submit handling remains a thin signal/effect adapter over `admin/src/transport.rs`;
  - FFA slice: product admin editor reset/apply signal values are composed by `ProductAdminEditorFormState` in `admin/src/core.rs`, keeping product-to-form mapping and default form policy outside Leptos;
  - FFA slice: product admin publish/draft/archive command preparation is composed by `ProductAdminStatusMutationCommand` / `ProductAdminStatusTarget` in `admin/src/core.rs`; Leptos status actions dispatch typed core commands over `admin/src/transport.rs`;
  - FFA slice: product admin delete command preparation is composed by `ProductAdminDeleteCommand` in `admin/src/core.rs`; Leptos delete action dispatches a typed core command and clears the editor through the shared core-owned empty form state;
  - FFA slice: product admin delete-result view policy (clear-selection intent, refresh intent, no-op/error copy) is composed by `ProductAdminDeleteResultViewModel` / `ProductAdminDeleteOutcome` in `admin/src/core.rs`; Leptos delete action only applies those intents;
  - FFA slice: product admin list action labels and busy-state availability are composed by `ProductAdminListActionLabels` / `product_admin_list_actions_disabled` in `admin/src/core.rs`; Leptos list actions bind prepared labels and use the core disabled predicate;
  - FFA slice: product admin list loading/empty/error state copy is composed by semantic `ProductAdminListStateViewModel` helpers in `admin/src/core.rs`; Leptos list rendering maps semantic state kind to framework-specific classes;
  - FFA slice: product admin list controls copy/search placeholder/status filter options are composed by `ProductAdminListControlsViewModel` in `admin/src/core.rs`; Leptos list controls only bind prepared labels/options;
  - FFA slice: product admin shell copy and shipping-profile panel loading/error/ready messages are composed by `ProductAdminShellViewModel` and `ProductAdminProfilePanelViewModel` in `admin/src/core.rs`; Leptos renders prepared strings without owning this copy/state policy;
  - FFA slice: product admin editor field placeholders, new action label, shipping-profile empty option and keep-published checkbox copy are composed by `ProductAdminEditorCopy` in `admin/src/core.rs`; Leptos editor rendering consumes prepared strings only;
  - FFA slice: product admin transport/error base copy and load/save/status failure message composition are owned by `ProductAdminErrorCopy` in `admin/src/core.rs`; Leptos effects reuse prepared messages without owning those error bindings;
  - FFA slice: product admin status mutation refresh/error outcome policy is composed by `ProductAdminStatusMutationOutcome` / `ProductAdminStatusMutationResultViewModel` in `admin/src/core.rs`; Leptos status action effects only dispatch transport and apply prepared intents;
  - FFA slice: product admin route/query selection writes are composed by `ProductAdminRouteQueryIntent` helpers in `admin/src/core.rs`; Leptos applies typed push/replace/clear intents without owning the product selection query policy;
  - FFA slice: product admin selected-product query normalization is composed by `ProductAdminSelectedProductQueryState` / `product_admin_selected_product_query_state` in `admin/src/core.rs`; Leptos applies the prepared open/clear state without owning `product_id.trim().is_empty()` policy;
  - FFA slice: product admin products-list async result normalization is composed by `ProductAdminProductsLoadViewModel` / `product_admin_products_load_view_from_result` in `admin/src/core.rs`; Leptos renders prepared loading/error/empty state or ready items without unpacking `ProductList` or owning empty-result classification;
  - FFA slice: product admin shipping-profile async result normalization is composed once by `ProductAdminShippingProfilesLoadViewModel` / `product_admin_shipping_profiles_load_view_from_result` in `admin/src/core.rs`; the editor select and registry status panel consume the same prepared options/panel envelope instead of maintaining duplicate Leptos branches;
  - FFA slice: product admin SEO panel title/subtitle/empty-message copy is composed by `ProductAdminSeoPanelCopy` in `admin/src/core.rs`; Leptos passes prepared copy into `SeoEntityPanel` without owning product SEO copy policy;
  - FFA slice: product admin inventory quantity input normalization is composed by `parse_product_admin_inventory_quantity_input` in `admin/src/core.rs`; Leptos forwards raw input text and no longer owns invalid-number fallback policy;
  - FFA slice: product admin open-product result policy is composed by `ProductAdminOpenProductViewModel` / `build_product_admin_open_product_view_model` in `admin/src/core.rs`; Leptos applies prepared selected-product/form-state/error outcomes without owning not-found/load-error reset policy;
  - FFA slice: product admin pricing preview async-resource state mapping is composed by `product_admin_pricing_preview_state_from_result` in `admin/src/core.rs`; Leptos selected-summary rendering no longer owns loading/error/unavailable/ready classification for pricing preview results;
  - FFA slice: product admin pricing-preview request construction and primary-currency/default fallback are composed by `ProductAdminPricingPreviewRequest` / `product_admin_pricing_preview_request_from_product` in `admin/src/core.rs`; Leptos selected-pricing resource only forwards the prepared request to transport;
  - FFA slice: product admin list row status badge container class is composed by `ProductAdminListItemViewModel.status_badge_class` / `product_admin_status_badge_container_class` in `admin/src/core.rs`; Leptos row rendering no longer joins base badge classes with status-specific CSS policy;
  - FFA cleanup: product admin status badge policy no longer exposes a separate suffix helper/base-class split; tests and row view-models assert the full core-owned container class contract directly;
  - FFA slice: product admin shipping-profile select options are composed by `ProductAdminShippingProfileOption` / `build_product_admin_shipping_profile_options` in `admin/src/core.rs`; Leptos select rendering consumes prepared option value/label pairs instead of mapping raw profile DTOs;
  - FFA slice: product admin list loading/empty/error container class policy is composed by `ProductAdminListStateViewModel.container_class` in `admin/src/core.rs`; Leptos list rendering consumes the prepared class without owning state-to-CSS branching;
  - FFA slice: product admin selected-summary panel title copy is composed by `ProductAdminSummaryPanelCopy` / `build_product_admin_summary_panel_copy` in `admin/src/core.rs`; Leptos summary panel rendering consumes prepared copy and the fast boundary guardrail rejects direct `product.summary.title` / `Selected product` copy in the UI adapter;
  - FFA slice: product admin list row shipping-profile chip display policy is composed by `ProductAdminListItemViewModel.show_shipping_profile` plus prepared `shipping_profile_label` in `admin/src/core.rs`; Leptos row rendering consumes the ready flag/string and the fast boundary guardrail rejects `item_shipping_profile_label.is_some()` / `unwrap_or_default()` policy in the UI adapter;
  - FFA slice: product storefront catalog rail title/total/empty/open/fallback labels are composed by `build_product_catalog_rail_labels` in `storefront/src/core.rs`; Leptos `CatalogRail` consumes prepared labels and no longer imports `crate::i18n::t` for rail copy construction;
  - FFA slice: product storefront selected-product metadata row is composed by `SelectedProductViewModel.metadata_items` in `storefront/src/core.rs`; Leptos `SelectedProductCard` renders prepared metadata items without owning separator/display composition for product type, vendor and publication timestamp;
  - FFA slice: product storefront catalog route segment fallback is composed by `resolve_product_storefront_route_segment` / `PRODUCT_STOREFRONT_DEFAULT_ROUTE_SEGMENT` in `storefront/src/core.rs`; Leptos `CatalogRail` forwards the host route segment and no longer owns the `"products"` default policy;
  - FFA slice: product storefront catalog rail empty-state policy is composed by `ProductCatalogRailViewModel.show_empty_state` in `storefront/src/core.rs`; Leptos `CatalogRail` renders the prepared branch without owning the `items.is_empty()` policy;
  - FFA guardrail: `scripts/verify/verify-product-admin-boundary.mjs` added to the aggregate `verify:ffa:ui:migration` pipeline, with fixture coverage wired through `test:verify:ffa:ui:migration` via `scripts/verify/verify-product-admin-boundary.test.mjs`; it checks product admin core/transport/ui split without long Cargo compilation;
  - FFA guardrail: `scripts/verify/verify-product-storefront-boundary.mjs` added to the aggregate `verify:ffa:ui:migration` pipeline, with fixture coverage wired through `test:verify:ffa:ui:migration` via `scripts/verify/verify-product-storefront-boundary.test.mjs`; it checks product storefront core/transport/ui split plus catalog rail label and selected metadata ownership without long Cargo compilation;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs.
- Last verified at (UTC): 2026-07-05T09:44:03Z
- Owner: `rustok-product` module team

## Область работ

- удерживать `rustok-product` как owner product/variant/catalog domain;
- закрепить product-owned admin UI как первый UI slice распила ecommerce family;
- синхронизировать product tags, shipping profile bindings и local docs;
- не смешивать catalog runtime с pricing/inventory/order orchestration.

## Текущее состояние

- product catalog, variants, options, translations и publication contract уже живут в модуле;
- taxonomy-backed `product_tags` уже служат first-class product tag surface;
- typed `shipping_profile_slug` уже закреплён в product/variant persistence и DTO;
- module-owned admin UI пакет `rustok-product/admin` уже поднят и подключён в
  manifest-driven admin composition как первый шаг UI split; admin list/status/filter,
  shipping-profile, pricing-preview и pricing deep-link helpers вынесены в
  framework-agnostic `admin/src/core.rs`, GraphQL операции проходят через
  `admin/src/transport.rs` и `admin/src/transport/graphql_adapter.rs`, selected-product summary собирается через
  `SelectedProductSummaryViewModel`, list-card display state собирается через
  `ProductAdminListItemViewModel`, editor shell state собирается через
  `ProductAdminEditorViewModel`, а submit command/validation state собирается через
  `ProductAdminSaveCommand` / `ProductAdminDraftForm`, а editor reset/apply mapping — через
  `ProductAdminEditorFormState`, а publish/draft/archive command mapping — через
  `ProductAdminStatusMutationCommand` / `ProductAdminStatusTarget`, status mutation result policy — через `ProductAdminStatusMutationOutcome` / `ProductAdminStatusMutationResultViewModel`, а delete command mapping — через
  `ProductAdminDeleteCommand`, а delete-result policy — через
  `ProductAdminDeleteResultViewModel` / `ProductAdminDeleteOutcome`, а list action labels/availability — через
  `ProductAdminListActionLabels` / `product_admin_list_actions_disabled`, loading/empty/error list state — через `ProductAdminListStateViewModel` helpers, а list controls/search/status options — через `ProductAdminListControlsViewModel`, shell/profile-panel copy — через `ProductAdminShellViewModel` / `ProductAdminProfilePanelViewModel`, selected-summary panel copy — через `ProductAdminSummaryPanelCopy`, shipping-profile select options — через `ProductAdminShippingProfileOption`, editor field/action copy — через `ProductAdminEditorCopy`, transport/error base copy and failure formatting — через `ProductAdminErrorCopy`, product SEO panel copy — через `ProductAdminSeoPanelCopy`, inventory quantity input normalization — через `parse_product_admin_inventory_quantity_input`, open-product result policy — через `ProductAdminOpenProductViewModel`, pricing preview state mapping — через `product_admin_pricing_preview_state_from_result`, pricing-preview request construction — через `ProductAdminPricingPreviewRequest`, list-state container class policy — через `ProductAdminListStateViewModel.container_class`, row status badge container class policy — через `ProductAdminListItemViewModel.status_badge_class`, row shipping-profile chip display policy — через `ProductAdminListItemViewModel.show_shipping_profile`, product selection route/query writes — через `ProductAdminRouteQueryIntent` helpers, а selected-product query normalization — через `ProductAdminSelectedProductQueryState` helpers в `admin/src/core.rs`; Leptos слой
  изолирован в `admin/src/ui/leptos.rs` как render/effect adapter;
- module-owned storefront UI пакет `rustok-product/storefront` уже поднят и
  подключён в manifest-driven storefront composition для published catalog
  discovery через native Leptos server functions с GraphQL fallback;
- storefront UI продолжает FFA-декомпозицию: route/query normalization, typed fetch
  request shape, shell copy, selected-product view-model composition, selected-card
  labels/empty state, catalog rail view-model, pricing/seller labels, pricing
  deep-link state и pricing-context sanitization/defaulting вынесены в
  framework-agnostic `storefront/src/core.rs`, catalog rail label construction moved into `build_product_catalog_rail_labels`, selected-product metadata row construction moved into `SelectedProductViewModel.metadata_items`, catalog route segment fallback moved into `resolve_product_storefront_route_segment`, catalog rail empty-state branching moved into `ProductCatalogRailViewModel.show_empty_state`, native/GraphQL storefront fetch
  paths оформлены как `storefront/src/transport/` adapters with serializable
  fallback error evidence, `ProductTransportErrorDomEvidence` composes host-visible
  failure attributes в core, а Leptos слой изолирован в `storefront/src/ui/leptos.rs`
  как thin render/host-context adapter;
- transport-level validation и public transport по-прежнему публикуются фасадом `rustok-commerce`.

## Этапы

### Нативные атрибуты каталога и category-bound формы

- [x] Добавить write-side storage для `product_attributes`, translations, options, channel settings, `catalog_categories`, closure table, reusable `product_attribute_schemas`, category schema assignments, category-local bindings/groups, `product_categories`, virtual category materialization и typed product/variant attribute values.
- [x] Зафиксировать `products.primary_category_id` как structural category, определяющую effective product form.
- [x] Добавить framework-independent schema resolver для режимов `inherit`, `use_schema`, `clone_from_category` и `custom`; `clone_from_category` является snapshot, `inherit` является live inheritance.
- [x] Добавить owner-owned `ProductCatalogSchemaService` для create attribute/category/schema, schema mode assignment, schema/category bindings и загрузки effective form из storage.
- [x] Добавить product/catalog-specific domain events для attribute/schema/category/value изменений и подключить их к product indexer reindex flow.
- [x] Добавить read-side projection storage для highload category assignments и normalized attribute facet/search/sort values.
- [x] Добавить category-bound admin transport DTOs и GraphQL operation contracts для CRUD attributes/categories/schemas, schema/category bindings и effective product form preview; localized operations consume host-provided effective locale without module-local fallback.
- [x] Подключить server-side GraphQL resolvers к `ProductCatalogSchemaService` для CRUD attributes/categories/schemas, schema/category bindings и effective product form preview.
- [x] Подключить native Leptos `#[server]` functions к `ProductCatalogSchemaService` как default internal data layer в product admin, оставив GraphQL как parallel contract.
- [x] Подключить `primary_category_id` к owner DTO/entity/service, GraphQL create/update/read и product admin category-first selector; загружать effective form и detached markers по выбранной structural category.
- [x] Добавить typed read/patch contract для product attribute values с explicit clear, empty multiselect clear, effective-schema/option validation, localized translation storage, detached read markers, transactional outbox event и native/GraphQL parity.
- [x] Перевести product admin form с metadata/custom-field ввода на grouped typed effective schema values с dirty patch semantics, локализованными option dictionaries и locale-aware group labels.
- [x] Добавить owner-owned publish validation для required effective attributes без module-local locale fallback.
- [x] Добавить detached-value review/clear API и product admin controls с native/GraphQL parity.
- [x] Материализовать effective category assignments и normalized attribute facet/search/sort rows в runtime indexer.
- [x] Материализовать bounded V1 virtual category rules в `virtual_category_product_assignments`.
- [x] Применить schema/category visibility overrides и channel settings в runtime facet/search/sort projections.
- [x] Подключить `rustok-search` к channel-scoped normalized facets/sorts и materialized virtual category assignments.
- [x] Закрепить projection-search contract быстрым source/schema guardrail.
- [ ] Подключить storefront/admin UI controls к optional catalog filters/sorts.

### 1. Contract stability

- [x] зафиксировать product-owned catalog boundary;
- [x] перевести tags на taxonomy-backed first-class contract;
- [x] зафиксировать typed `shipping_profile_slug` для product/variant;
- [ ] удерживать sync между product runtime contract, commerce transport и module metadata.

### 2. Catalog hardening

- [ ] покрывать publication, tags и shipping-profile edge-cases targeted tests;
- [ ] развивать product-specific semantics без возврата к metadata-only contract;
- [ ] удерживать deliverability-facing bindings совместимыми с fulfillment/pricing flows.
- [ ] Закрыть DB-level tenant consistency audit для native catalog tables: составные tenant-aware FK/unique guardrails должны исключать cross-tenant связи между attributes/options/categories/schemas/groups/values на уровне БД, а не только сервисной валидацией.
- [ ] Нормализовать оставшиеся legacy product locale columns до `VARCHAR(32)` по platform i18n contract: `product_translations`, `product_image_translations`, `product_option_translations`, `product_option_value_translations`, `product_variant_translations`.
- [ ] Зафиксировать detached-value marker contract: текущее поведение вычисляет detached read-time от effective schema и не требует проставлять `detached_at` при смене `primary_category_id`; если нужен persisted marker, добавить отдельную миграцию/обработчик `ProductPrimaryCategoryChanged`.
- [ ] Добавить быстрый no-compile schema guardrail для product catalog attribute migration и index projection invariants: ключевые таблицы, `VARCHAR(32)` locale в новых tables, closure/materialized virtual tables, typed value/options tables, partial indexes для facet/search/sort.

### 3. Operability

- [x] поднять module-owned admin UI пакет для product catalog surface;
- [x] документировать новые catalog guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [x] вынести storefront FFA core slice для route/query state, selected-product view-model и pricing/seller helpers;
- [x] вынести storefront catalog rail presentation в core view-model без Leptos runtime;
- [x] вынести selected-product card labels и empty state в core view-model без Leptos runtime;
- [x] вынести storefront shell copy и typed fetch request shape в core без Leptos runtime;
- [x] выделить storefront native/GraphQL transport adapters и явный Leptos UI adapter поверх core-owned request/policy state;
- [x] вынести product admin list/status/filter, shipping-profile и pricing-preview helpers в framework-agnostic admin core;
- [x] выделить product admin GraphQL operations behind a module-owned transport facade and `admin/src/transport/graphql_adapter.rs` without changing `rustok-commerce` GraphQL contract;
- [x] изолировать product admin Leptos rendering under `admin/src/ui/leptos.rs` with crate-root re-export boundary;
- [x] вынести selected product admin summary state into `SelectedProductSummaryViewModel` in framework-agnostic admin core;
- [x] вынести product admin list-card display state into `ProductAdminListItemViewModel` in framework-agnostic admin core;
- [x] вынести product admin editor shell state into `ProductAdminEditorViewModel` in framework-agnostic admin core;
- [ ] обновлять consumer-module docs при изменении tag/deliverability integration rules.

## Проверка

- `npm.cmd run verify:product:runtime-fallback-smoke`
- `npm.cmd run test:verify:product:runtime-fallback-smoke`
- `npm.cmd run verify:ecommerce:fba`
- `npm.cmd run test:verify:ecommerce:fba`
- `cargo xtask module validate product`
- `cargo xtask module test product`
- targeted tests для catalog CRUD, tags, publication и shipping-profile bindings
- targeted `cargo test -p rustok-product ports::tests` перед live provider execution evidence и повышением FBA до `transport_verified`

## Правила обновления

1. При изменении product runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или UI wiring синхронизировать `rustok-module.toml`.
4. При изменении shipping-profile или taxonomy integration обновлять связанные commerce docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
