# rustok-product-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-product` module.

For full public interface and contract definitions, see [CRATE_API.md](CRATE_API.md).

## Responsibilities

- Exposes the product catalog admin root view used by `apps/admin`.
- Keeps product list/create/edit/publish/archive workflow inside the product-owned package.
- Keeps admin shell copy, profile-panel state, list/status/filter, list-card view-model, editor shell view-model, shipping-profile, selected-summary, pricing-preview and pricing deep-link presentation helpers in framework-agnostic `src/core.rs`, leaving Leptos as the render/effect adapter.
- Isolates Leptos rendering in `src/ui/*`: `src/ui/root.rs` owns the mounted router shell, `src/ui/product_grid.rs`, `src/ui/product_editor.rs`, `src/ui/attributes.rs` and `src/ui/categories.rs` own the routed pages, and `src/ui/leptos.rs` owns the shared sections those pages mount (typed attribute values, variant axes, category-schema authoring). The crate root re-exports the mounted `ui::root::ProductAdmin`.
- Routes admin data operations through `src/transport.rs`, with GraphQL operations in `src/transport/graphql_adapter.rs` and native server functions in `src/transport/native_server_adapter.rs`.
- Builds native catalog schema services from `HostRuntimeContext` DB and typed `TransactionalEventBus` host handles without a package-local framework runtime or framework-specific outbox adapter.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses registry-backed shipping-profile selection so catalog operators work with typed product bindings instead of raw slug text.
- Ships package-owned `admin/locales/en.ftl` and `admin/locales/ru.ftl` bundles declared through `[provides.admin_ui.i18n]`.
- Embeds owner-side product SEO editing through `rustok-seo-panel` so product metadata stays inside the product screen.

## Entry Points

- `ProductAdmin` - mounted root admin view re-exported from `ui::root` and rendered from the host admin registry.
- `ProductGridPage`, `ProductEditorPage`, `AttributesPage`, `CategoriesPage` - routed pages of the mounted surface.
- `core::*` helpers for product admin shell copy, profile-panel state, product list/status/filter labels, list-card view-models, editor shell view-models, selected-summary view-models, pricing previews and pricing deep links.
- `transport::*` facade functions for product admin native and GraphQL operations.

### Non-mounted reference compositions

`ui::leptos::ProductAdmin` and `ui::catalog_admin::ProductAdmin` (the same composition behind
the catalog-controls query shell) are **not mounted**: the host code generator mounts
`ui::root::ProductAdmin`. They remain the catalog-controls reference pinned by the catalog
verification suite, not alternate advertised entry points. A selected `product_id` now mounts
the same keyed `ProductEditorPage` as the routed surface, independently of Product GET. The
unselected catalog/create reference does not duplicate the saved-document recovery boundary.
Every capability they expose is reachable on the mounted surface: list controls and search in `ui/product_grid.rs` + `ui/catalog_admin.rs`, editor shell and media in `ui/product_editor.rs`, typed attribute values through `ProductAttributeValuesSection`, variant axes through `ProductVariantAxesSection`, standalone SKU creation/recovery through `ProductVariantCreateSection` and existing-row editing through `ProductVariantsPanel`, attribute/schema authoring in `ui/attributes.rs`, and category work in `ui/categories.rs`. New feature work belongs on the mounted pages and the shared sections; the reference compositions must not grow independent write workflows.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract for product CRUD while ownership moves to module-owned UI.
- Treats `product -> variants.prices` as a catalog compatibility snapshot and now
  renders pricing-authoritative preview through a separate `adminPricingProduct`
  hook instead of presenting catalog snapshot rows as resolved prices.
- Links directly into `rustok-pricing/admin` with prefilled product id and
  pricing context so operators can move from catalog editing to pricing control
  without reselecting the product.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-panel`
  for explicit product SEO authoring.
- Accepts Product deep links through typed `product_id`, and dictionary selection through
  `attribute_id` plus optional `option_id`, without generic `id`/display-field aliases.
- Reads the effective UI locale from `UiRouteContext.locale`; product translation edits and edit-form hydration both resolve against that host-owned locale without a package-local locale override.

## Documentation

- See [platform docs](../../../../docs/index.md).

## Editorial save boundary

The mounted editor binds `ProductAttributeValuesSection` to its candidate category and shared
`ProductDraft`; typed fields are available before creation. Saving the Product composes its
localized value batches and status in the same owner transaction. There is no second EAV or status
save. Detached cleanup remains an explicit independent command. Value re-fetches preserve dirty
fields; only acknowledged catalog saves clear their dirty state. The older unkeyed lifecycle
GraphQL implementation has been removed; `product_lifecycle_graphql` is the sole lifecycle gateway.

Existing Variant edits are an explicit **independent action**, not a second command hidden behind
Product Save/Publish. The pricing card identifies the exact Variant hydrated into its controls and
provides `Save only this variant`; ordinary document Save respects the selected status. Each
acknowledgement updates only its confirmed local snapshot without a whole-editor refetch that
would overwrite the other block's unsaved fields. Inputs are disabled while either of these commands runs.

Variant controls are snapshotted before bootstrap/IO. Empty base-price input leaves prices alone,
not an implicit free USD row; zero and decimal precision remain valid. Invalid local scalars and
unsupported SKU/barcode clears are refused before sending. Full nullable Variant patches and
immutable pending-request recovery for existing-row updates remain separate gaps; those
retained-key helpers are not durable snapshot recovery. Five added Rust tests are unrun without Cargo/rustc. Source
guards are not compilation, database or mounted-client evidence. Whole-screen multi-command
atomicity is not claimed: Product document/category/EAV/publication remains the atomic Save scope.


## Admin catalog trusted context (2026-10-18)

Native list and facet reads now propagate canonical RequestContext rejection before owner
queries, even with an explicit locale. Existing authentication, permission and tenant checks
remain mandatory. Successful locale precedence and owner translation fallback are unchanged.
See the [current contract](../docs/product-public-error-diagnostic-safety.md#admin-catalog-context-admission-continuation-2026-10-18).
This is source-only/unintegrated: two new source scenarios are unrun; compilation, host trusted
resolved-locale composition, and mounted profile behavior remain maintainer verification gates.


## Known Limitations / Pending Implementation

Variant-axis policy authoring and atomic sparse Product creation are implemented in source,
including the mounted `ui/product_editor.rs` path. The first untouched category draft seeds
required/default axes; subsequent category/value edits preserve rows and require explicit
correction. `core/initial_catalog.rs` owns selection/assignment/scalar validation, and
`ui/initial_variants.rs` uses the canonical `leptos-ui` controls. The existing Product create
command receives the exact selected axes/SKUs, not a temporary default SKU followed by writes.

Standalone SKU creation/recovery is mounted through `ProductVariantCreateSection` outside the
document fieldset; `ProductVariantsPanel` only edits existing rows. Creation sends explicit axis
assignments and refuses invalid inventory/price text rather than coercing it to zero.
Projection refresh only merges acknowledged variants/axes; it does not rebase dirty document
controls. Persisted-axis writes are blocked while the category is dirty. The newly mounted
section does not offer SKU deletion until stock-safe owner lifecycle semantics are implemented.

`core/variant_axes.rs`, `ui/variant_axes.rs` and the selected native/GraphQL adapters now own
existing-axis preview/full-tuple correction/CAS in source. Localized recovery controls persist
an immutable axes request/key/locale under sessionStorage plus a stable URL marker; unknown
outcomes never fall back to another transport/provider. Recovery remains reachable outside the
locked document fieldset even when Product loading fails. Metadata refresh preserves selections;
oversized dictionaries retain saved selections only and explicitly refuse new-axis authoring.
A terminal axes receipt does not rebase dirty document fields or supply a new Product revision.
See the [owner workflow/recovery contract](../docs/variant-axis-reconfiguration.md).

`core/combinations.rs` and `ui/combinations.rs` now implement bounded candidate browsing and
explicit picking on both mounted create/standalone-SKU forms, matching the Next package's
local projection. Filters do not alter canonical axes; saved membership must be complete and
at most 256 SKUs, candidate spaces at most 4096, pages 12 rows. Invalid source/stale filters
refuse visibly. Initial picks add only one unpriced draft, or fill the existing manual tuple
without adding a row; saved picks replace only tuple fields. The ordinary owner commands remain
the writers. SKU grids show actual named assignments, missing assignments and a localized default
tuple rather than using an opaque combination encoding as display text. See the
[workflow and release gates](../docs/variant-combinations.md).

Standalone creation now shares `core/write_journal.rs` and `transport/write_journal.rs` with
axes, preserving the axes persisted shape. It freezes full input/key/locale/scope before IO,
retains the terminal draft for explicit correction, and replays without a Product GET. Native
SSR/hydrate JSON ingress uses the authenticated host-selected command port; GraphQL preserves
owner metadata. Neither switches provider after failure. Root fresh admission verifies both
journals. Stale bootstrap/owner responses cannot change a new authenticated recovery context.
See [standalone semantics and unexecuted gates](../docs/variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery).

This continuation has no Rust compilation or mounted execution evidence and no tests were run.
Axes, standalone SKU creation and dictionary definition lifecycle have durable Leptos browser
recovery in source; other retained-key CRUD helpers remain memory-only. Product create and remaining CRUD still use
`transport/product_lifecycle_graphql.rs`; this is not a general native CRUD cutover. Unsubmitted
local drafts/filters are not durable pending commands. Localization of other initial/schema
controls, stock-safe retirement, storefront work and runtime verification
remain in the [Product plan](../docs/implementation-plan.md#variant-axis-continuation-2026-10-29).


## Attribute and option definition recovery

The mounted `ui/attributes.rs` page includes `ui/attribute_lifecycle.rs`. Its pure
`core/attribute_lifecycle.rs` and neutral DTOs prepare presentation-only updates/archive/restore;
selected native/GraphQL adapters use the installed schema ports and the existing receipt ledger,
never a direct-service fallback after failure. UUIDs/codes/storage shape and SKU/EAV links remain
stable. Archive rechecks bounded current references, including child IDs when archiving a parent;
incomplete inspection is a refusal, not evidence that a definition is unused.

The existing journal is specialized by an attribute scope and `attribute_recovery` URL marker,
without replacing Product/SKU envelopes. Full request/key/original locale precede IO; Unknown
permits exact replay even when GET fails. Missing selected UUIDs stay visible/uneditable. Fresh
requires terminal proof and publishes the original nullable target before committing Ready, so
failed full navigation cannot select the parent/another option on reload. Raw duplicate, orphan,
noncanonical, sentinel and changed URL identities fail admission. Both admin clients expose the
same owner decisions; saved owner impact outranks advisory reads.

[ADR §10c](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10c-attribute-and-option-definition-lifecycle)
and [API](CRATE_API.md#attributeoption-definition-lifecycle) specify bounds and exact-copy semantics.
Regression sources are written but unrun; this is not Rust/macro/SQL/mounted-client certification
or a general durable CRUD/storefront cutover.
