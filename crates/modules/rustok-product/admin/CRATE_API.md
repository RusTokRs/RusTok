# rustok-product-admin / CRATE_API

## Public surface

### Entry Point & Pages
- `ProductAdmin`: mounted root admin component rendered into host admin applications.
- `ProductGridPage`: product catalog grid listing, search, and bulk operations page.
- `ProductEditorPage`: detailed product authoring and variant management editor page.
- `AttributesPage`: typed attribute schema and category binding page.
- `CategoriesPage`: category hierarchy management page.

### Core View Models & Helpers
- `ProductKind`: enumerated product classification (`Physical`, `Digital`, `Service`).
- `product_grid_columns`: standard column definitions for the product catalog grid.
- `admin_catalog_facets_to_grid`: facet view transformer for grid filter panels.
- `fetch_catalog_search_options`: transport query helper retrieving search and filter options.

## Platform Capability Bindings
- **Mounted Admin Surface**: Registers `ProductAdmin` as the canonical admin entrypoint via manifest-driven code generation.
- **Variant Axes Configuration**: Exposes dynamic variant axes management (`SetVariantAxesDraft`, `VariantAxisConfig`, `VariantAxisDraft`).
- **Facet Panels & Filtering**: Exposes facet aggregation models (`ProductAdminFacetPanelView`, `AdminCatalogFacet`).

## Domain Collaborations
- **`rustok-product`**: Core domain module providing product entities, variants, and validation logic.
- **`rustok-pricing`**: Deep-links directly to pricing admin panels and renders authoritative pricing previews.
- **`rustok-seo-panel`**: Integrates product-level SEO metadata editing within the product form.
- **`rustok-grid-leptos`**: Standard data grid component rendering the product catalog with sorting and pagination.

## Minimum Contract Set

### Input DTOs/Commands
- `SetVariantAxesDraft`: DTO defining the complete axis draft for multi-variant products.
- `VariantAxisDraft`: draft representation of a single variant axis.
- `VariantAxisValueDraft`: draft value option within a variant axis.
- `ProductCatalogSearchOptions`: query parameters for product searching and filtering.

### Domain Invariants
- **Non-Swallowed Errors**: Catalog mutations (status changes, deletions, variant updates) must inspect and handle operation results instead of silently ignoring errors.
- **Pricing Authority**: Catalog variant prices are compatibility snapshots; authoritative pricing controls and validations belong to `rustok-pricing`.
- **Locale Parity**: Admin interface strings use package-owned Fluent translations (`en.ftl`, `ru.ftl`) synchronized with host route context.

### Events / Outbox Side Effects
- Triggers product lifecycle mutations (publish, archive, draft) producing outbox domain events.
- Dispatches category hierarchy and variant axis modifications through native server or GraphQL adapters.

### Errors / Failure Codes
- `UiError`: form validation and presentation error states.
- `CatalogSearchOptionsErrorContext`: diagnostic error context for catalog search failures.


## Initial sparse selection and policy patches

`ProductDraft.variant_axes` and optional `initial_variants` describe explicit initial selection.
The existing create mapper forwards it through the canonical Product create command;
Product create still delegates to `product_lifecycle_graphql`. Standalone SKU creation uses
its own selected native/GraphQL adapter and the shared durable journal described below; this
is not a general native CRUD cutover.
`BindSchemaAttributeDraft` / `BindCategoryAttributeDraft` preserve omitted policy/default fields.
Attribute/effective-form models expose definition scope so both transports enforce the same
eligibility. `core/initial_catalog.rs` owns required/default suggestions, bounds, tuple uniqueness
and scalar/standalone-assignment validation; `ui/initial_variants.rs` is a render/binding section
mounted by `ProductEditorPage`. Its standalone create/recovery section is mounted outside the
document fieldset; the shared Variant panel edits existing rows only.

The 128-SKU initial draft bound is a client bound, not a replacement owner import limit.
Category/value changes retain invalid drafts for correction. Axis writes are guarded during
unsaved category edits. An independent Variant/axis acknowledgement refreshes only its confirmed
projection and does not hydrate/rebase document controls. The newly mounted panel does not
expose destructive Variant retirement.

Compilation/mounted execution, remaining copy localization and durable browser pending-request
recovery for lifecycle commands other than axes and standalone creation remain outstanding; see [Known Limitations / Pending Implementation](README.md#known-limitations--pending-implementation).


## Existing-axis preview and recovery

`SetVariantAxesDraft` adds optional `expected_state` and full `reassignments` through
`VariantAxisReassignmentDraft`. `VariantAxesPreview` carries complete bounded SKU tuples and
closed conflict kinds; `VariantAxesApplyOutcome` distinguishes confirmed axes, durable owner
refusal and unknown outcome. Native SSR/hydrate JSON endpoints and the GraphQL adapter use the
selected Product command runtime; no post-failure fallback is allowed. The mounted workflow and
axes-specific immutable browser recovery contract are documented
[here](../docs/variant-axis-reconfiguration.md); source checks do not certify runtime behavior.


## Local bounded combination browser

The internal `core/combinations.rs` projects effective/saved configured axes and complete saved
membership into candidate pages (32 axes, 256 values per axis, 4096 candidates, 12 per page).
Full-tuple membership uses actual Variant IDs, not SKU/display labels; invalid/duplicate/oversized
saved membership refuses, while incomplete initial drafts remain explicitly unindexed.
`build_variant_row_view_models(product, locale)` now renders named saved assignments and localized
missing/default tuple copy, not opaque `combination_identity` text.

`ui/combinations.rs` only renders filters/pages and dispatches explicit local selections. Initial
selection appends one unpriced `VariantDraft` or fills the manual pending tuple for separate
explicit authoring/addition; saved selection prefills only the standalone form's tuple. No new server
function, GraphQL mutation, command port or persisted key is introduced. Blank initial/standalone
base amount means no price; explicit zero is preserved, orphan compare-at remains invalid.
See the [workflow, bounds and unexecuted gates](../docs/variant-combinations.md). This adds no
runtime certification; standalone recovery is the separate bounded cutover below.


## Standalone SKU recovery

`core/variant_create.rs` prepares full inputs and specializes the shared `core/write_journal.rs`
state machine; `transport/write_journal.rs` is the single scoped sessionStorage/URL adapter for
axes and creation. `VariantCreateOutcome` distinguishes a confirmed Variant UUID from a closed
durable refusal and unknown outcome. It supplies **no Product revision**.

`ProductVariantCreateSection` mounts stable recovery outside the document fieldset, with
immutable request/key/original locale and authenticated identity before IO. Native JSON and
GraphQL project the existing selected command/receipt; the deployed caller namespace remains
`create_product_variant`. Only explicit terminal correction/fresh-document actions unlock the
next intent. Other CRUD helpers outside axes/SKU/definition recovery remain memory-only. The matching Next action/journal and
all bounds/failure semantics are documented in the
[owner recovery contract](../docs/variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery).
Tests/compilation/mounted execution remain unverified; syntax checks do not certify behavior.


## Attribute/option definition lifecycle

The mounted `ui/attribute_lifecycle.rs` panel owns no business mutation: neutral DTOs live in
`model/attribute_lifecycle.rs`, preparation/refusal/workspace/control validation in
`core/attribute_lifecycle.rs`, and selected native/GraphQL adapters in `transport/attribute_lifecycle*`.
`JournalScope` specializes the existing `core/write_journal.rs` / `transport/write_journal.rs`
mechanism with an attribute resource, leaving Product envelopes/prefixes unchanged.

The dictionary namespace is `rustok.product.attribute_definition_attempt:{tenant}:{actor}:{attribute}`
with URL marker `attribute_recovery`. Full request/key/original locale is persisted before IO;
Unknown replays exactly, a terminal record precedes explicit Fresh, and the original selected
`option_id` survives Fresh even when GET cannot find it. The frozen target is published before
committing Ready, so failed full navigation cannot retarget a reload. `attribute_id`/`option_id` are shared typed
selection keys. Missing target has no editable snapshot or fabricated parent impact. Recovery and
saved-request inspection are independent of dictionary reads; sibling writers are disabled while
unresolved. Replies/cached reads cannot publish another authenticated scope's data.

Native JSON endpoints `/api/fn/product/admin/attribute-lifecycle` and
`/api/fn/product/admin/update-attribute-definition` use authenticated selected schema capabilities;
non-native builds use the same GraphQL command/result, never a post-failure fallback. See
[ADR §10c](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10c-attribute-and-option-definition-lifecycle).
Source tests are unrun; formatting is not Rust/macro/SQL/mounted-client certification.
