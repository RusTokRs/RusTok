# Implementation plan for `rustok-product`

Status: product boundary is defined; the module owns the catalog and typed product data,
while transport and part of orchestration remain with umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: product_ui_native_loco_free_transport
- Last checkpoint: product FBA remains `boundary_ready` on no-compile runtime fallback evidence, and product admin plus storefront native server functions now build owner services from `HostRuntimeContext` DB plus typed `TransactionalEventBus` host handles. The admin and storefront packages no longer depend on `loco-rs` or `rustok-outbox/loco-adapter`; GraphQL operation documents remain parallel in `admin/src/transport/graphql_adapter.rs` and `storefront/src/transport/graphql_adapter.rs`, and the product admin/storefront boundary verifiers lock the Loco-free native boundaries against current core/UI helper names.
- Dependency evidence: product storefront locale matching uses `rustok_api::locale_tags_match`; product admin and storefront native SSR use `HostRuntimeContext` and no longer contain package-local `loco-rs` or `loco-adapter` dependencies; no-feature/hydrate profiles no longer contain `rustok-core`.
- Next step: Gather live provider execution evidence before promoting product FBA to `transport_verified`.
- Open blockers: None.
- Hand-off notes for next agent: Continue Loco-exit slices at module-native boundaries; do not touch parallel UI/i18n rewrites unless their guardrails are stale against current code.
- Last updated at (UTC): 2026-07-08T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`: read policy executes before owner `CatalogService`, then typed `PortError` mapping is applied; fallback profiles/degraded modes are checked against registry;
  - no-compile runtime fallback smoke `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json` + `scripts/verify/verify-product-runtime-fallback-smoke.mjs` source-locks product catalog read fallback behavior, bounded pagination validation, locale fallback, tenant scope, typed `PortError` mapping, `package.json` product/aggregate script wiring, `modules.toml` / `rustok-module.toml` metadata sync, central commerce-domain batch summary and the prepared `crates/rustok-product/src/ports.rs` unit-test harness without Rust compilation. Fixture regression test `scripts/verify/verify-product-runtime-fallback-smoke.test.mjs` is wired into `test:verify:ecommerce:fba` and covers package aggregate drift, module metadata drift, premature `transport_verified` registry/local-plan drift and stale central batch-summary drift. FBA status raised to `boundary_ready`; `transport_verified` still requires live provider execution evidence;
  - module plan synchronized with central FFA/FBA readiness board; UI surface already published and managed in migration/backlog rhythm;
  - FBA slice: `crates/rustok-product/src/ports.rs` declares `ProductCatalogReadPort`/`product.catalog_read.v1` for catalog read projections consumed by commerce checkout/storefront compatibility paths, pricing enrichment and `ai-product` generation context; `crates/rustok-product/contracts/product-fba-registry.json`, `contracts/evidence/product-contract-test-static-matrix.json`, `contracts/evidence/product-runtime-contract-smoke.json` and `contracts/evidence/product-runtime-fallback-smoke.json` lock provider metadata, fallback profiles and no-compile runtime fallback behavior under `npm run verify:ecommerce:fba`; status remains below `transport_verified` until live runtime execution/fallback evidence lands;
  - umbrella facade `rustok_commerce::{services::catalog, CatalogService}` is removed; commerce/server/AI consumers import `CatalogService` from `rustok-product` directly, so product owner service is no longer masked by the ecommerce umbrella;
  - product DTOs are exposed through the owner surface `rustok_product::dto`; `rustok-commerce` no longer publicly re-exports product DTO aliases, and commerce/server/AI/test callers import product DTO contracts from the product owner crate;
  - product entities are exposed through `rustok_product::entities` for product, variant, translation, option and image tables; `rustok-commerce` no longer publicly re-exports these product entity aliases;
  - FFA slice: storefront catalog rail title/total/empty/open labels, item fallback labels, seller boundary text, published timestamp fallback and handle links now live in framework-agnostic `ProductCatalogRailViewModel` with unit-test evidence;
  - FFA slice: selected-product card empty state, pricing context label, ownership note, metric labels and pricing action label now live in `SelectedProductEmptyViewModel` / `SelectedProductViewModel` with unit-test evidence;
  - FFA slice: storefront shell badge/title/subtitle/load-error copy and typed fetch request shape now live in `ShellViewModel` / `FetchRequest` with unit-test evidence;
  - FFA slice: storefront pricing-context sanitization/defaulting moved into core, native/GraphQL fetch adapters now sit behind `storefront/src/transport/`, legacy `storefront/src/api.rs` is removed, native server functions use `HostRuntimeContext` plus a typed `TransactionalEventBus` host handle without `loco-rs` / `rustok-outbox/loco-adapter`, and Leptos rendering is isolated in `storefront/src/ui/leptos.rs`; evidence: `cargo check -p rustok-product-storefront --features ssr`, `cargo test -p rustok-product-storefront --lib`;
  - FFA slice: storefront transport errors now keep serializable native/GraphQL selected-path evidence (`ProductTransportError`, `ProductTransportPath`), core composes `ProductTransportErrorDomEvidence`, and the Leptos error adapter exposes stable `data-product-transport-*` attributes for host/parity smoke checks;
  - FFA slice: product admin list/status/filter, shipping-profile, pricing-preview and pricing deep-link helpers moved into `admin/src/core.rs`; Leptos admin remains the render/effect adapter while GraphQL transport stays unchanged for this slice;
  - FFA slice: product admin GraphQL operations now route through `admin/src/transport.rs`, with `admin/src/transport/graphql_adapter.rs` as the GraphQL adapter; legacy `admin/src/api.rs` is removed and forbidden by `verify-product-admin-boundary.mjs`, preserving the existing `rustok-commerce` GraphQL contract;
  - FFA/FBA slice: category-bound admin transport DTOs now live behind `admin/src/transport.rs`; native Leptos `#[server]` functions in `admin/src/transport/native_server_adapter.rs` are the default internal path and use `HostRuntimeContext` plus a typed `TransactionalEventBus` host handle without `loco-rs` / `rustok-outbox/loco-adapter`, GraphQL operation documents remain parallel in `admin/src/transport/graphql_adapter.rs`, and server-side GraphQL query/mutation/type bindings in `rustok-commerce/src/graphql/*` call owner-owned `ProductCatalogSchemaService` for attributes, categories, reusable schemas, schema mode assignment, schema/category bindings and effective form preview; the fast guardrail rejects optional/fallback locale for these new localized catalog operations and checks native, GraphQL and server GraphQL markers;
  - FFA/FBA slice: product CRUD exposes nullable `primary_category_id`; the Leptos product editor loads only structural categories through the module transport facade, persists the selection through GraphQL create/update, and resolves category-first effective-form preview through the build-profile-selected native/GraphQL contract using `UiRouteContext.locale`;
  - FFA/FBA slice: typed product attribute value reads and transactional patches are owner-owned by `ProductCatalogSchemaService`; native Leptos server functions remain the default admin path, GraphQL remains parallel, localized text uses only the explicit host locale, and the fast product boundary guardrail locks both surfaces;
  - FFA/FBA slice: publish validation is owner-owned by `ProductCatalogSchemaService`; `CatalogService::publish_product` rejects missing required effective attributes before status changes, text-like localized required values require an explicit non-empty translation row, option values require at least one option relation, detached values do not satisfy requirements outside the effective schema, and create-with-publish is rejected for categories with required typed attributes;
  - FFA/FBA slice: detached values are listed through the typed value read contract and cleared through owner-owned `clear_detached_product_attribute_values`; the service rejects non-detached ids, native `#[server]` remains primary, GraphQL remains parallel, and product admin renders review rows plus explicit clear-all action;
  - FFA/FBA slice: effective form options are loaded in one bounded query for effective attribute ids and localized by the host locale; effective group labels are resolved from schema/category group translations through the same explicit locale; schema/category group creation and `group_code` bindings are available through native/GraphQL admin contracts; `ProductAttributeEditorState` owns dirty tracking, typed parsing and explicit clear semantics outside Leptos, while `TypedProductAttributeField` renders grouped controls and submit persists values only after the product category is committed;
  - FFA/FBA slice: Next admin product package exposes owner-owned `listCatalogCategorySearchOptions` and `listCatalogAttributeSearchOptions` helpers for search host composition. They query `catalogCategories` / `productAttributes` through product GraphQL with the host effective locale, return category `id` values and filterable/sortable attribute `code` values, and keep search UI from importing product internals directly;
  - FFA/FBA slice: `rustok-product-admin` exposes owner-owned `fetch_catalog_search_options` plus neutral option DTOs for future Leptos host composition. The helper requires the host effective locale and reuses the product build-profile-selected native/GraphQL transport facade; search packages remain consumers of host-provided metadata only;
  - FFA/FBA slice: `apps/admin` now composes product-owned catalog metadata into `SearchAdmin` through `SearchAdminComposition`; the product helper resolves the current tenant through its native `#[server]` endpoint first and keeps GraphQL in parallel, while the host supplies locale/auth/tenant context and checks product enablement;
  - FFA/FBA slice: `rustok-product-storefront` exposes a public-safe catalog search option contract through native `#[server]` for monolith/hydrate builds and `storefrontCatalogSearchOptions(locale: String!)` for the GraphQL selected path; the native path is Loco-free and uses `HostRuntimeContext` DB/event-bus handles; `apps/storefront::SearchStorefrontComposition` supplies host locale, checks product enablement and maps only public owner DTOs into search props;
  - FFA/FBA slice: `apps/next-frontend/packages/rustok-product` exposes public-safe `fetchCatalogSearchOptions` over `storefrontCatalogSearchOptions(locale: String!)`; `apps/next-frontend/src/features/search` composes those owner DTOs into `SearchStorefrontPage` with route locale, tenant slug and product enablement from the host registry context, while the search package still imports no product internals;
  - server artifact cleanup: the unused host-local `apps/server/src/services/product_search.rs` duplicate is removed; product translation title search predicates remain owner/foundation-owned and `apps/server` must not re-export `services::product_search`;
  - i18n evidence: `verify-ui-i18n-parity.mjs` no longer excludes `rustok-product`; admin/storefront EN/RU bundles are part of the common `npm run verify:i18n:ui` gate;
  - FFA slice: product admin Leptos rendering moved under `admin/src/ui/leptos.rs`, and `admin/src/lib.rs` now acts as the module/re-export boundary for `ProductAdmin`;
  - FFA slice: selected product admin summary labels, pricing preview state and pricing deep-link are composed by `SelectedProductSummaryViewModel` in `admin/src/core.rs`, keeping Leptos summary rendering as markup-only;
  - FFA slice: product admin list-card display state (status label/badge, type fallback, meta label, shipping profile chip and published/created timestamp) is composed by `ProductAdminListItemViewModel` in `admin/src/core.rs`, keeping Leptos list rendering as markup/action binding only;
  - FFA slice: product admin editor shell state (create/edit mode, title, subtitle and submit label) is composed by `ProductAdminEditorViewModel` in `admin/src/core.rs`, keeping Leptos editor rendering as markup/action binding only;
  - FFA slice: product admin submit validation, locale/bootstrap guardrails, create/update mode selection and `ProductDraft` command preparation are composed by `SaveCommand` / `DraftForm` in `admin/src/core.rs`; Leptos submit handling remains a thin signal/effect adapter over `admin/src/transport.rs`;
  - FFA slice: product admin editor reset/apply signal values are composed by `ProductAdminEditorFormState` in `admin/src/core.rs`, keeping product-to-form mapping and default form policy outside Leptos;
  - FFA slice: product admin publish/draft/archive command preparation is composed by `StatusCommand` / `StatusTarget` in `admin/src/core.rs`; Leptos status actions dispatch typed core commands over `admin/src/transport.rs`;
  - FFA slice: product admin delete command preparation is composed by `DeleteCommand` in `admin/src/core.rs`; Leptos delete action dispatches a typed core command and clears the editor through the shared core-owned empty form state;
  - FFA slice: product admin delete-result view policy (clear-selection intent, refresh intent, no-op/error copy) is composed by `DeleteResultViewModel` / `DeleteOutcome` in `admin/src/core.rs`; Leptos delete action only applies those intents;
  - FFA slice: product admin list action labels and busy-state availability are composed by `ProductAdminListActionLabels` / `product_admin_list_actions_disabled` in `admin/src/core.rs`; Leptos list actions bind prepared labels and use the core disabled predicate;
  - FFA slice: product admin list loading/empty/error state copy is composed by semantic `ProductAdminListStateViewModel` helpers in `admin/src/core.rs`; Leptos list rendering maps semantic state kind to framework-specific classes;
  - FFA slice: product admin list controls copy/search placeholder/status filter options are composed by `ProductAdminListControlsViewModel` in `admin/src/core.rs`; Leptos list controls only bind prepared labels/options;
  - FFA slice: product admin shell copy and shipping-profile panel loading/error/ready messages are composed by `ProductAdminShellViewModel` and `ProductAdminProfilePanelViewModel` in `admin/src/core.rs`; Leptos renders prepared strings without owning this copy/state policy;
  - FFA slice: product admin editor field placeholders, new action label, shipping-profile empty option and keep-published checkbox copy are composed by `ProductAdminEditorCopy` in `admin/src/core.rs`; Leptos editor rendering consumes prepared strings only;
  - FFA slice: product admin transport/error base copy and load/save/status failure message composition are owned by `ProductAdminErrorCopy` in `admin/src/core.rs`; Leptos effects reuse prepared messages without owning those error bindings;
  - FFA slice: product admin status mutation refresh/error outcome policy is composed by `StatusOutcome` / `StatusResultViewModel` in `admin/src/core.rs`; Leptos status action effects only dispatch transport and apply prepared intents;
  - FFA slice: product admin route/query selection writes are composed by `ProductAdminRouteQueryIntent` helpers in `admin/src/core.rs`; Leptos applies typed push/replace/clear intents without owning the product selection query policy;
  - FFA slice: product admin selected-product query normalization is composed by `ProductAdminSelectedProductQueryState` / `product_admin_selected_product_query_state` in `admin/src/core.rs`; Leptos applies the prepared open/clear state without owning `product_id.trim().is_empty()` policy;
  - FFA slice: product admin products-list async result normalization is composed by `ProductAdminProductsLoadViewModel` / `product_admin_products_load_view_from_result` in `admin/src/core.rs`; Leptos renders prepared loading/error/empty state or ready items without unpacking `ProductList` or owning empty-result classification;
  - FFA slice: product admin shipping-profile async result normalization is composed once by `ShippingProfilesLoadViewModel` / `shipping_profiles_load_view_from_result` in `admin/src/core.rs`; the editor select and registry status panel consume the same prepared options/panel envelope instead of maintaining duplicate Leptos branches;
  - FFA slice: product admin SEO panel title/subtitle/empty-message copy is composed by `ProductAdminSeoPanelCopy` in `admin/src/core.rs`; Leptos passes prepared copy into `SeoEntityPanel` without owning product SEO copy policy;
  - FFA slice: product admin inventory quantity input normalization is composed by `parse_product_admin_inventory_quantity_input` in `admin/src/core.rs`; Leptos forwards raw input text and no longer owns invalid-number fallback policy;
  - FFA slice: product admin open-product result policy is composed by `ProductAdminOpenProductViewModel` / `build_product_admin_open_product_view_model` in `admin/src/core.rs`; Leptos applies prepared selected-product/form-state/error outcomes without owning not-found/load-error reset policy;
  - FFA slice: product admin pricing preview async-resource state mapping is composed by `pricing_preview_state_from_result` in `admin/src/core.rs`; Leptos selected-summary rendering no longer owns loading/error/unavailable/ready classification for pricing preview results;
  - FFA slice: product admin pricing-preview request construction and primary-currency/default fallback are composed by `PricingPreviewRequest` / `pricing_preview_request_from_product` in `admin/src/core.rs`; Leptos selected-pricing resource only forwards the prepared request to transport;
  - FFA slice: product admin list row status badge container class is composed by `ProductAdminListItemViewModel.status_badge_class` / `product_admin_status_badge_container_class` in `admin/src/core.rs`; Leptos row rendering no longer joins base badge classes with status-specific CSS policy;
  - FFA cleanup: product admin status badge policy no longer exposes a separate suffix helper/base-class split; tests and row view-models assert the full core-owned container class contract directly;
  - FFA slice: product admin shipping-profile select options are composed by `ShippingProfileOption` / `build_shipping_profile_options` in `admin/src/core.rs`; Leptos select rendering consumes prepared option value/label pairs instead of mapping raw profile DTOs;
  - FFA slice: product admin list loading/empty/error container class policy is composed by `ProductAdminListStateViewModel.container_class` in `admin/src/core.rs`; Leptos list rendering consumes the prepared class without owning state-to-CSS branching;
  - FFA slice: product admin selected-summary panel title copy is composed by `ProductAdminSummaryPanelCopy` / `build_product_admin_summary_panel_copy` in `admin/src/core.rs`; Leptos summary panel rendering consumes prepared copy and the fast boundary guardrail rejects direct `product.summary.title` / `Selected product` copy in the UI adapter;
  - FFA slice: product admin list row shipping-profile chip display policy is composed by `ProductAdminListItemViewModel.show_shipping_profile` plus prepared `shipping_profile_label` in `admin/src/core.rs`; Leptos row rendering consumes the ready flag/string and the fast boundary guardrail rejects `item_shipping_profile_label.is_some()` / `unwrap_or_default()` policy in the UI adapter;
  - FFA slice: product storefront catalog rail title/total/empty/open/fallback labels are composed by `build_product_catalog_rail_labels` in `storefront/src/core.rs`; Leptos `CatalogRail` consumes prepared labels and no longer imports `crate::i18n::t` for rail copy construction;
  - FFA slice: product storefront selected-product metadata row is composed by `SelectedProductViewModel.metadata_items` in `storefront/src/core.rs`; Leptos `SelectedProductCard` renders prepared metadata items without owning separator/display composition for product type, vendor and publication timestamp;
  - FFA slice: product storefront catalog route segment fallback is composed by `resolve_route_segment` / `DEFAULT_ROUTE_SEGMENT` in `storefront/src/core.rs`; Leptos `CatalogRail` forwards the host route segment and no longer owns the `"products"` default policy;
  - FFA slice: product storefront catalog rail empty-state policy is composed by `ProductCatalogRailViewModel.show_empty_state` in `storefront/src/core.rs`; Leptos `CatalogRail` renders the prepared branch without owning the `items.is_empty()` policy;
  - FFA guardrail: `scripts/verify/verify-product-admin-boundary.mjs` added to the aggregate `verify:ffa:ui:migration` pipeline, with fixture coverage wired through `test:verify:ffa:ui:migration` via `scripts/verify/verify-product-admin-boundary.test.mjs`; it checks product admin core/transport/ui split without long Cargo compilation;
  - FFA guardrail: `scripts/verify/verify-product-storefront-boundary.mjs` added to the aggregate `verify:ffa:ui:migration` pipeline, with fixture coverage wired through `test:verify:ffa:ui:migration` via `scripts/verify/verify-product-storefront-boundary.test.mjs`; it checks product storefront core/transport/ui split plus catalog rail label and selected metadata ownership without long Cargo compilation;
  - further status promotion is done only together with verification evidence and local+central docs update.
- Last verified at (UTC): 2026-07-05T09:44:03Z
- Owner: `rustok-product` module team

## Scope of work

- maintain `rustok-product` as owner of product/variant/catalog domain;
- lock product-owned admin UI as the first UI slice of the ecommerce family split;
- synchronize product tags, shipping profile bindings and local docs;
- do not mix catalog runtime with pricing/inventory/order orchestration.

## Current state

- product catalog, variants, options, translations and publication contract already live in the module;
- taxonomy-backed `product_tags` already serve as a first-class product tag surface;
- typed `shipping_profile_slug` is already locked in product/variant persistence and DTO;
- module-owned admin UI package `rustok-product/admin` is already up and connected in manifest-driven admin composition as the first UI split step; admin list/status/filter, shipping-profile, pricing-preview and pricing deep-link helpers are extracted to framework-agnostic `admin/src/core.rs`, GraphQL operations pass through `admin/src/transport.rs` and `admin/src/transport/graphql_adapter.rs`, selected-product summary is assembled through `SelectedProductSummaryViewModel`, list-card display state through `ProductAdminListItemViewModel`, editor shell state through `ProductAdminEditorViewModel`, submit command/validation state through `SaveCommand` / `DraftForm`, editor reset/apply mapping through `ProductAdminEditorFormState`, publish/draft/archive command mapping through `StatusCommand` / `StatusTarget`, status mutation result policy through `StatusOutcome` / `StatusResultViewModel`, delete command mapping through `DeleteCommand`, delete-result policy through `DeleteResultViewModel` / `DeleteOutcome`, list action labels/availability through `ProductAdminListActionLabels` / `product_admin_list_actions_disabled`, loading/empty/error list state through `ProductAdminListStateViewModel` helpers, list controls/search/status options through `ProductAdminListControlsViewModel`, shell/profile-panel copy through `ProductAdminShellViewModel` / `ProductAdminProfilePanelViewModel`, selected-summary panel copy through `ProductAdminSummaryPanelCopy`, shipping-profile select options through `ShippingProfileOption`, editor field/action copy through `ProductAdminEditorCopy`, transport/error base copy and failure formatting through `ProductAdminErrorCopy`, product SEO panel copy through `ProductAdminSeoPanelCopy`, inventory quantity input normalization through `parse_product_admin_inventory_quantity_input`, open-product result policy through `ProductAdminOpenProductViewModel`, pricing preview state mapping through `pricing_preview_state_from_result`, pricing-preview request construction through `PricingPreviewRequest`, list-state container class policy through `ProductAdminListStateViewModel.container_class`, row status badge container class policy through `ProductAdminListItemViewModel.status_badge_class`, row shipping-profile chip display policy through `ProductAdminListItemViewModel.show_shipping_profile`, product selection route/query writes through `ProductAdminRouteQueryIntent` helpers, and selected-product query normalization through `ProductAdminSelectedProductQueryState` helpers in `admin/src/core.rs`; the Leptos layer is isolated in `admin/src/ui/leptos.rs` as a render/effect adapter;
- module-owned storefront UI package `rustok-product/storefront` is already up and connected in manifest-driven storefront composition for published catalog discovery through Loco-free native Leptos server functions and the parallel GraphQL selected path;
- storefront UI continues FFA decomposition: route/query normalization, typed fetch request shape, shell copy, selected-product view-model composition, selected-card labels/empty state, catalog rail view-model, pricing/seller labels, pricing deep-link state and pricing-context sanitization/defaulting are extracted to framework-agnostic `storefront/src/core.rs`, catalog rail label construction moved into `build_product_catalog_rail_labels`, selected-product metadata row construction moved into `SelectedProductViewModel.metadata_items`, catalog route segment fallback moved into `resolve_route_segment`, catalog rail empty-state branching moved into `ProductCatalogRailViewModel.show_empty_state`, native/GraphQL storefront fetch paths are structured as `storefront/src/transport/` adapters with serializable fallback error evidence, `ProductTransportErrorDomEvidence` composes host-visible failure attributes in core, and the Leptos layer is isolated in `storefront/src/ui/leptos.rs` as a thin render/host-context adapter;
- transport-level validation and public transport are still published through the `rustok-commerce` facade.

## Stages

### Native catalog attributes and category-bound forms

- [x] Add write-side storage for `product_attributes`, translations, options, channel settings, `catalog_categories`, closure table, reusable `product_attribute_schemas`, category schema assignments, category-local bindings/groups, `product_categories`, virtual category materialization and typed product/variant attribute values.
- [x] Lock `products.primary_category_id` as the structural category defining the effective product form.
- [x] Add framework-independent schema resolver for modes `inherit`, `use_schema`, `clone_from_category` and `custom`; `clone_from_category` is a snapshot, `inherit` is live inheritance.
- [x] Add owner-owned `ProductCatalogSchemaService` for create attribute/category/schema, schema mode assignment, schema/category bindings and effective form loading from storage.
- [x] Add product/catalog-specific domain events for attribute/schema/category/value changes and connect them to product indexer reindex flow.
- [x] Add read-side projection storage for highload category assignments and normalized attribute facet/search/sort values.
- [x] Add category-bound admin transport DTOs and GraphQL operation contracts for CRUD attributes/categories/schemas, schema/category bindings and effective product form preview; localized operations consume host-provided effective locale without module-local fallback.
- [x] Connect server-side GraphQL resolvers to `ProductCatalogSchemaService` for CRUD attributes/categories/schemas, schema/category bindings and effective product form preview.
- [x] Connect native Leptos `#[server]` functions to `ProductCatalogSchemaService` as the default internal data layer in product admin, keeping GraphQL as a parallel contract.
- [x] Connect `primary_category_id` to owner DTO/entity/service, GraphQL create/update/read and product admin category-first selector; load effective form and detached markers by selected structural category.
- [x] Add typed read/patch contract for product attribute values with explicit clear, empty multiselect clear, effective-schema/option validation, localized translation storage, detached read markers, transactional outbox event and native/GraphQL parity.
- [x] Transition product admin form from metadata/custom-field input to grouped typed effective schema values with dirty patch semantics, localized option dictionaries and locale-aware group labels.
- [x] Add owner-owned publish validation for required effective attributes without module-local locale fallback.
- [x] Add detached-value review/clear API and product admin controls with native/GraphQL parity.
- [x] Materialize effective category assignments and normalized attribute facet/search/sort rows in runtime indexer.
- [x] Materialize bounded V1 virtual category rules in `virtual_category_product_assignments`.
- [x] Apply schema/category visibility overrides and channel settings in runtime facet/search/sort projections.
- [x] Connect `rustok-search` to channel-scoped normalized facets/sorts and materialized virtual category assignments.
- [x] Lock projection-search contract with fast source/schema guardrail.
- [x] Connect storefront/admin UI controls to optional catalog filters/sorts. Evidence: product-owned admin/storefront search metadata helpers expose category options plus filterable/sortable attribute options, Leptos/Next hosts pass them into search UI composition, and `verify-search-ui-boundary` source-locks both host-provided controls and the no module-local locale fallback rule.

### 1. Contract stability

- [x] lock product-owned catalog boundary;
- [x] transition tags to taxonomy-backed first-class contract;
- [x] lock typed `shipping_profile_slug` for product/variant;
- [x] maintain sync between product runtime contract, commerce transport and module metadata. Evidence: `verify-product-runtime-fallback-smoke` locks `ProductCatalogReadPort` / `product.catalog_read.v1` across `product-fba-registry.json`, `rustok-module.toml`, `modules.toml`, README/docs, central FFA/FBA board and ecommerce aggregate package scripts without promoting the provider beyond `boundary_ready`.

### 2. Catalog hardening

- [x] cover publication, tags and shipping-profile edge-cases with targeted tests. Evidence: existing targeted Rust tests cover taxonomy-backed product tags without `metadata.tags` mirrors, legacy metadata read fallback, unknown shipping profile rejection, incompatible storefront shipping profile filtering/rejection and `ProductPublished` event emission; `verify-product-catalog-schema` now source-locks those test cases and the publish/shipping helper call sites without Cargo compilation.
- [x] evolve product-specific semantics without reverting to metadata-only contract. Evidence: product docs and `rustok-commerce/CRATE_API.md` describe first-class `tags`, `product_tags`, typed `shipping_profile_slug`, nullable `seller_id`, native typed attribute values and the pricing-authoritative split; `verify-product-catalog-schema` source-locks these docs plus tag/shipping helper call sites.
- [x] keep deliverability-facing bindings compatible with fulfillment/pricing flows. Evidence: commerce docs lock `variant -> product -> default` effective shipping profile resolution, cart/order line-item snapshots, seller-aware delivery groups, active shipping-profile validation and the catalog-vs-pricing authority split; `verify-product-catalog-schema` source-locks those markers and the helper call sites used by storefront cart/shipping flows.
- [x] Close DB-level tenant consistency audit for native catalog tables: composite tenant-aware FK/unique guardrails must prevent cross-tenant relationships between attributes/options/categories/schemas/groups/values at DB level, not just service-level validation. Evidence: `m20260701_000002_add_product_catalog_tenant_consistency_constraints` backfills variant/join tenant ids, adds tenant-aware uniqueness/FKs for product/category/attribute/schema/value relationships, and `verify-product-catalog-schema` source-locks the constraints and tenant-aware value-option writes.
- [x] Normalize remaining legacy product locale columns to `VARCHAR(32)` per platform i18n contract: `product_translations`, `product_image_translations`, `product_option_translations`, `product_option_value_translations`, `product_variant_translations`. Fresh create-table migrations now use `string_len(32)`, and `m20260405_000007_expand_product_locale_storage_columns` widens existing Postgres columns without a destructive down migration.
- [x] Lock detached-value marker contract: current behavior computes detached read-time from effective schema and does not require setting `detached_at` when changing `primary_category_id`; if a persisted marker is needed, add a separate migration/handler `ProductPrimaryCategoryChanged`. Evidence: `verify-product-catalog-schema` source-locks resolver-side `detached_attribute_ids`, read-side `record.detached = detached_attribute_ids.contains(...)`, and write-side `detached_at = NULL` only when an effective value is saved.
- [x] Add fast no-compile schema guardrail for product catalog attribute migration and index projection invariants: key tables, `VARCHAR(32)` locale in new tables, closure/materialized virtual tables, typed value/options tables, partial indexes for facet/search/sort. Guardrail: `scripts/verify/verify-product-catalog-schema.mjs` + fixture coverage `scripts/verify/verify-product-catalog-schema.test.mjs`, wired into `verify:ecommerce:fba` / `test:verify:ecommerce:fba`.
- [x] Lock product-owned catalog search metadata and UI composition guardrails: admin/storefront product packages publish category and filterable/sortable attribute options, Leptos and Next hosts inject them into search controls, and `verify-search-ui-boundary` covers GraphQL/native parity, public-safe storefront metadata and host effective locale usage.

### 3. Operability

- [x] bring up module-owned admin UI package for product catalog surface;
- [x] document new catalog guarantees concurrently with runtime surface changes;
- [x] keep local docs and `README.md` synchronized for native catalog attributes, search metadata, detached values, highload projections and the current no-compile verification gates.
- [x] extract storefront FFA core slice for route/query state, selected-product view-model and pricing/seller helpers;
- [x] extract storefront catalog rail presentation into core view-model without Leptos runtime;
- [x] extract selected-product card labels and empty state into core view-model without Leptos runtime;
- [x] extract storefront shell copy and typed fetch request shape into core without Leptos runtime;
- [x] isolate storefront native/GraphQL transport adapters and explicit Leptos UI adapter over core-owned request/policy state;
- [x] extract product admin list/status/filter, shipping-profile and pricing-preview helpers into framework-agnostic admin core;
- [x] isolate product admin GraphQL operations behind a module-owned transport facade and `admin/src/transport/graphql_adapter.rs` without changing `rustok-commerce` GraphQL contract;
- [x] isolate product admin Leptos rendering under `admin/src/ui/leptos.rs` with crate-root re-export boundary;
- [x] extract selected product admin summary state into `SelectedProductSummaryViewModel` in framework-agnostic admin core;
- [x] extract product admin list-card display state into `ProductAdminListItemViewModel` in framework-agnostic admin core;
- [x] extract product admin editor shell state into `ProductAdminEditorViewModel` in framework-agnostic admin core;
- [x] update consumer-module docs when tag/deliverability integration rules change. Evidence: `crates/rustok-commerce/README.md`, `crates/rustok-commerce/docs/README.md` and `crates/rustok-commerce/CRATE_API.md` are synchronized with first-class product tags, typed shipping profiles, seller-aware delivery groups and the pricing split, and `verify-product-catalog-schema` now checks those consumer-facing markers.

## Verification

- `npm.cmd run verify:product:runtime-fallback-smoke`
- `npm.cmd run test:verify:product:runtime-fallback-smoke`
- `npm.cmd run verify:product:catalog-schema`
- `npm.cmd run test:verify:product:catalog-schema`
- `npm.cmd run verify:ecommerce:fba`
- `npm.cmd run test:verify:ecommerce:fba`
- `cargo xtask module validate product`
- `cargo xtask module test product`
- targeted tests for catalog CRUD, tags, publication and shipping-profile bindings
- targeted `cargo test -p rustok-product ports::tests` before live provider execution evidence and FBA promotion to `transport_verified`

## Update rules

1. When changing product runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata or UI wiring, synchronize `rustok-module.toml`.
4. When changing shipping-profile or taxonomy integration, update related commerce docs.


## Quality backlog

- [x] Update test coverage for key module scenarios. Evidence: targeted Rust tests for tags, publication and shipping-profile compatibility are source-locked by the no-compile product catalog verifier; Cargo execution remains the live evidence step.
- [x] Verify completeness and accuracy of `README.md` and local docs. Evidence: root README, local docs README, implementation plan and commerce consumer docs are cross-checked by `verify-product-catalog-schema` / `verify-product-runtime-fallback-smoke`.
- [x] Lock/update verification gates for current module state. Evidence: expanded `verify-product-catalog-schema` and `verify-product-runtime-fallback-smoke` fixture suites cover schema/tenant/i18n/search UI metadata, detached markers, runtime metadata sync, targeted edge-case tests and consumer docs without large compilation.
