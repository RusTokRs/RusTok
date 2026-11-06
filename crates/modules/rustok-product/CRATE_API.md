# Product Crate API Specification

## Public surface

## Public Modules

- `dto`: Product, variant, translation, and catalog data transfer objects.
- `entities`: SeaORM entity models for products, variants, and translations.
- `error`: Strongly typed commerce error variants (`CommerceError`, `CommerceResult`).
- `fulfillment`: Product shipping and fulfillment requirement definitions.
- `migrations`: SeaORM database migrations for product schema and variant definitions.
- `module`: Lifecycle descriptor `ProductModule` implementing `RusToKModule`.
- `ports`: Owner-defined cross-boundary read ports and projection request types.
- `services`: Domain services for catalog operations, schema handling, and media references.

## Platform Capability Bindings

- **RusToKModule Implementation**: `ProductModule` in `src/module.rs`
- **Module Slug**: `product`
- **Module Name**: `Product`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["taxonomy"]`
- **Permissions**:
  - `PRODUCTS_CREATE`
  - `PRODUCTS_READ`
  - `PRODUCTS_UPDATE`
  - `PRODUCTS_DELETE`
  - `PRODUCTS_LIST`
  - `PRODUCTS_MANAGE`
- **Runtime Extensions**:
  - `ProductRuntimeSelected` typed marker registered into `ModuleRuntimeExtensions`
  - `ProductSeoTargetProvider` registered via `rustok_seo_targets::register_seo_target_provider`
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` with dependency on `taxonomy` migration sequence.

### Selected identity capability

`ProductCatalogReadRuntime` retains optional `ProductVariantIdentityReadPort` alongside the
selected catalog/schema capabilities. `with_variant_identity_read_port` binds the host's
selected owner explicitly; `variant_identity_read_port` does not reconstruct a local service.
`in_process` registers the same embedded CatalogService; `external` does not invent identity.
`ProductVariantIdentitySnapshot.tenant_id` is required (no serde default or historical backfill).
`validate_variant_identities_response` admits only complete exact ordered tenant/SKU/parent
responses; `validate_product_variant_identity_list_response` checks the bounded selected page
and unique per-parent SKU limits. These are projection validators, not another catalog resolver,
publication/eligibility authorization or distributed reservation. Pricing consumes this selected
capability before tenant-less price-row reads. Remote adapter execution remains unverified.

## Domain Collaborations

- **Taxonomy**: Product categorization, hierarchical taxonomy tree bindings, and storefront breadcrumbs.
- **Pricing Collaboration**: Transaction-aware initial price creation, replacement, cleanup, and catalog projections via `ProductPricingPort` during product and variant lifecycle.
- **Inventory**: Transaction-aware initial inventory item provisioning during variant creation.
- **Flex**: Custom fields and dynamic attribute schemas attached to product aggregates.
- **Brand**: Cross-boundary collaboration with `rustok-brand`. Product owns the relational `product_brands` table mapping products to primary manufacturers and collaborative partner brands.
- **Outbox & Events**: Publishes catalog lifecycle domain events through `rustok_outbox::TransactionalEventBus`.
- **SEO & Translation Targets**: Implements targets for search engine optimization and multi-locale translation engines.

## Minimum Contract Set

- `ProductCatalogCommandPort`: Core mutating command execution (create, update, publish, archive, delete products and variants) and selected-owner axis planning.
- `ProductCatalogReadPort`: Read projections for admin and storefront consumers (`read_product_projection`, `read_variant_product_projection`, `list_published_products`).
- `ProductCatalogSchemaReadPort` & `ProductCatalogSchemaWritePort`: Custom attribute schemas, faceted search metadata, and dynamic forms.
- `ProductMediaReferencePort`: Media lifecycle tracking, release workflows, and reference audit.
- `ProductShippingProfilePort`: Fulfillment requirements and shipping classification.

### Input DTOs/Commands

- `CreateProduct`: Product name, slug/handle, category assignments, description, initial variant specifications.
- `UpdateProduct`: Partial product metadata updates, status changes, publication controls.
- `CreateVariant`: SKU, option combinations, weight, dimensions, initial pricing and inventory attributes.
- `UpdateVariant`: Variant updates including barcode, status, and attribute assignments.
- `ProductBrandInput` / `ProductBrandResponse`: Product brand assignment DTOs specifying `brand_id`, `is_primary`, `role` (e.g. `manufacturer`, `collaboration`), and `position`.
- `ProductProjectionRequest`: Request for single product projection by ID, slug, or locale.
- `VariantProductProjectionRequest`: Request for product projection resolved via variant ID.
- `PublishedProductsRequest`: Storefront query for published catalog items with pagination and sorting.

### Domain Invariants

- **Non-Empty Variants**: Every product aggregate must contain at least one variant (`NoVariants`).
- **Variant Preservation**: Deleting the only remaining variant of a product is prohibited (`CannotDeleteOnlyVariant`).
- **Publication Safety**: A published product cannot be hard-deleted directly (`CannotDeletePublished`); it must first be unpublished or archived.
- **Handle Uniqueness**: Product handle/slug must be unique per locale across the tenant (`DuplicateHandle`).
- **SKU Uniqueness**: Variant SKU must be unique across the tenant catalog (`DuplicateSku`).
- **Monetary & Measurement Bounds**: Non-negative decimal validation for prices, compare-at amounts, weights, and dimensions.
- **Brand Associations & Primary Brand Invariant**: Products can be associated with multiple brands. When multiple brands are assigned, exactly one must be designated as primary (`is_primary = true`). If a single brand is assigned without explicit designation, it defaults to primary. Enforced by unique constraint `uq_product_brands` and partial unique index `uq_product_brands_primary` at the database level.

### Events / Outbox Side Effects

- `DomainEvent::ProductCreated { product_id }`
- `DomainEvent::ProductUpdated { product_id }`
- `DomainEvent::ProductPublished { product_id }`
- `DomainEvent::ProductDeleted { product_id }`
- `DomainEvent::VariantCreated { product_id, variant_id }`
- `DomainEvent::VariantUpdated { product_id, variant_id }`
- `DomainEvent::VariantDeleted { product_id, variant_id }`
- `DomainEvent::ProductAttributeValuesChanged { product_id }`

### Errors / Failure Codes

- `CommerceError::ProductNotFound(Uuid)`: Target product does not exist.
- `CommerceError::DuplicateHandle { handle, locale }`: Slug conflict within the specified locale.
- `CommerceError::DuplicateSku(String)`: SKU conflict across products/variants.
- `CommerceError::Validation(String)`: Input validation failure.
- `CommerceError::Conflict(String)`: Concurrency revision conflict or invalid state transition.
- `CommerceError::NoVariants`: Attempted to save or finalize a product without variants.
- `CommerceError::VariantNotFound(Uuid)`: Target variant does not exist.
- `CommerceError::ImageNotFound(Uuid)`: Target product image does not exist.
- `CommerceError::CannotDeleteOnlyVariant`: Guard against leaving an aggregate variant-less.
- `CommerceError::CannotDeletePublished`: Guard against deleting active storefront items.
- `CommerceError::Database(DbErr)`: Underlying database error.
- `CommerceError::Core(CoreError)`: Platform core failure.
- `ProductPublicError`: Transport-safe error mapping for external consumers.

## Variant-axis policy and initial selection

`BindSchemaAttributeInput` and `BindCategoryAttributeInput` accept optional typed
`variant_axis_policy` (`forbidden`, `allowed`, `required`) and `default_variant_axis` patches.
Omission preserves an existing binding; a new binding defaults to `forbidden`/`false`. Allowed
and required policy requires an active `select` definition with `variant`/`both` scope. A
forbidden policy cannot retain a true default hint. Effective form reads expose the definition
scope and effective policy/default/source; malformed policy/clone state is an error.

Policy-affecting schema/category writes validate effective inherited descendants under the
schema transaction fence before committing. `CommerceError::VariantAxisPolicyConflict` carries
bounded typed UUID violations and `has_more`; the schema-write port uses
`product.variant_axis_policy_conflict` with bounded safe diagnostics, mapped by Commerce GraphQL
to `PRODUCT_VARIANT_AXIS_POLICY_CONFLICT`. Refusal does not reconfigure or delete existing SKUs.
The current general-purpose port envelope conveys diagnostics in its curated message; this is
not yet the structured preview/reassignment contract for existing-axis reconfiguration.

`CreateProductInput.variant_axes` and assignment-bearing `variants` use the existing atomic
create/receipt boundary. Sparse selected tuples are valid; there is no implicit Cartesian
expansion or temporary axis-less default SKU. Variant UUIDs remain cross-owner identities.
Product typed EAV writes accept only `product`/`both` scope, not variant-only definitions.

See [Known Limitations / Pending Implementation](README.md#known-limitations-pending-implementation)
and the [implementation plan](docs/implementation-plan.md#variant-axis-continuation-2026-10-29)
for source-only verification, unsupported/unverified writers and remaining rollout work.

## Existing-axis planning/application

- `ProductCatalogCommandPort::preview_variant_axes(context, product_id, input)` returns
  `VariantAxesPreviewResponse` under read call policy, without a receipt or persisted changes.
- `SetVariantAxesInput` adds optional `expected_state` and full target `reassignments`
  (`VariantAxisReassignmentInput` / `VariantAxisValueInput`); absent/empty fields retain old
  serialization. The existing `set_variant_axes` still returns `Vec<VariantAxisConfigResponse>`.
- `VariantAxesConflictKind` and bounded conflict/variant response DTOs expose missing/duplicate/
  unexpected assignments, disallowed values, duplicate combinations and multiple defaults.
- `product_catalog_command_error_is_terminal` classifies only known nonretryable owner business
  decisions; admission/key/context/infrastructure failures are not settlement proof.

The [owner contract](docs/variant-axis-reconfiguration.md) specifies fingerprints, locks,
nonnegative staging, stable IDs, transport mapping, client recovery and unexecuted runtime gates.

## Admin combination projection

The [bounded combination browser](docs/variant-combinations.md) reuses configured allowed values,
complete saved SKU membership and the existing Product/Variant create commands. It adds no
backend command/receipt schema, inventory operation or automatic Cartesian materialization.
Local selection is not owner admission or stock availability. Both admin clients have source
implementations; compilation, mounted behavior and all added test execution remain open.

## Standalone SKU caller recovery

The existing `create_variant` command and stored response remain authoritative. Native/GraphQL
admin adapters keep deployed `create_product_variant` caller framing and complete frozen input
under the existing receipt. No new Product revision acknowledgement or receipt schema is added.
Next and Leptos caller journals are recovery state, not a second catalog source of truth. See
[scope, full-request replay and unexecuted gates](docs/variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery).

## Attribute and option definitions

`ProductCatalogSchemaReadPort::read_attribute_lifecycle(PortContext, AttributeLifecycleRequest)`
returns a bounded `AttributeLifecycleWorkspace` with optional archived definitions, exact-locale
copy/options and advisory `AttributeReferenceImpact`. Missing parent returns an empty selected
projection; missing option retains its actual parent/options but has null impact, never the
parent's impact. Overflow/unsupported capability is unavailable, not an empty/truncated list.

`ProductCatalogSchemaWritePort::update_attribute_definition(PortContext, UpdateAttributeDefinitionInput)`
uses the existing schema receipt ledger. Actions `update`/`archive`/`restore` retain UUID/code/type/
scope/localization and all assignments/links. `expected_state` is a lowercase SHA-256 projection
fingerprint, not a monotonic or Product revision; `locale` must equal the frozen context locale.
`AttributeDefinitionResult` records acknowledgement or `stale`/`in_use`/`inspection_limit`/
`not_found`/`parent_archived` refusal. Only presentation fields and complete exact-locale copy are
mutable; archival never prunes references and restore never cascades. Detailed semantics, resource
bounds and unexecuted gates: [ADR §10c](../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10c-attribute-and-option-definition-lifecycle).

### Lightweight Pricing identity projection (2026-10-29)

- `ProductVariantIdentityReadPort`, implemented by `CatalogService` and called by
  Pricing's in-process read provider; this is separate from public Product visibility.
- `ProductVariantIdentitiesRequest`: optional expected Product UUID, 1–256 unique
  non-nil Variant UUIDs; complete ordered results or a typed refusal, never omission.
- `ProductVariantIdentityListRequest`: at most 48 previously selected Product UUIDs;
  at most 256 SKU identities per Product, with an overflow probe.
- `ProductVariantIdentitySnapshot`: canonical Product/Variant UUIDs only. Tenant
  filters and Product membership remain Product-owned; no mutable SKU string is a key.
- Price, stock, attribute tuples, localized copy and customer eligibility do not move
  into this projection. Reads require the caller's typed context and deadline.

## Inventory source caller continuation (2026-11-02; source only)

Both fresh Bootstrap calls and both guarded Product/Variant deletion calls explicitly forward
current `actor_id` to Inventory. Inventory alone publishes retained UUID/positive source revision
and retained tombstone on that same caller transaction. Product revisions, causal roots and
refresh ledger remain independent; they are not Inventory authority. Bootstrap/deletion APIs
now require non-nil current actor, never a saved actor fallback. Generic Inventory Index and full
operator/client recovery remain pending; [owner packet](../rustok-inventory/contracts/evidence/source-delivery-source.json).
