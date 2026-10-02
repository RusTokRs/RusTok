# rustok-fulfillment

## Purpose

`rustok-fulfillment` is the default fulfillment submodule of the `Ecommerce` family.

## Responsibilities

- Own shipping-option and fulfillment storage.
- Own typed `fulfillment_items` storage inside each fulfillment.
- Track per-item `shipped_quantity` and `delivered_quantity` inside `fulfillment_items` for partial delivery progress.
- Prepare a stable shipping boundary for checkout orchestration.
- Keep shipment lifecycle transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default fulfillment flow for the current stage.
- Expose a fulfillment-owned provider SPI registry with external carrier registration validation and side-effect-free runtime-mode guardrails before adapter invocation.
- Own storefront shipping handoff and seller-aware shipping selection presentation through `rustok-fulfillment/storefront`; commerce composes it through the aggregate checkout workspace and the explicit checkout runtime API.
- Normalize first-class `allowed_shipping_profile_slugs` on shipping-option contracts into the metadata-backed compatibility shape while older stored rows are still read.
- Provide create/update/lifecycle read-side service operations for shipping-option management that the commerce facade exposes over admin REST and GraphQL.
- Return typed fulfillment items from `FulfillmentResponse` instead of forcing post-order flows to reconstruct line-item scope from metadata blobs alone.
- Support partial `ship` / `deliver` adjustments on typed fulfillment items and append language-agnostic audit events to fulfillment/item metadata while keeping `delivered_note` as a typed field.
- Treat `metadata.audit` on fulfillment and fulfillment-item records as owner-generated lifecycle evidence: create inputs cannot seed it, and lifecycle metadata patches cannot replace existing audit history.
- Support explicit `reopen` / `reship` recovery flows on top of typed fulfillment items, so delivered or cancelled fulfillments can return to actionable post-order states without language-dependent metadata hacks.
- Treat `metadata.provider_operation` as a reserved provider commit receipt: provider-backed `ship` / `reship` / `cancel` flows may attach it after journaling, while ordinary `deliver` / `reopen` metadata patches cannot introduce or replace it.
- Provider operations with an unresolved external outcome remain fail-closed during migration rollback: the reconciliation migration refuses to roll back while any `reconciliation_required` operation has no persisted provider result; the operation must be resolved before the older lifecycle contract is restored.
- Checkout `create_label` payment protection is also fail-closed on rollback: the payment-guard migration refuses to roll back while a retryable or executing create-label operation is tied to an unpaid or invalid tenant-scoped order, so removing the guard cannot reopen premature provider execution.
- When an order is cancelled, any in-flight checkout `create_label` operation is moved to `reconciliation_required` rather than left executable; pending label operations are removed. This preserves the provider journal as the canonical fail-closed record for cancellation races.
- The cancellation quarantine is applied both to existing rows during migration upgrade and to live order-cancellation transitions; cancellation becomes the recorded reason for the reconciliation state.
- Direct checkout `create_label` journal insertion as `executing` is permitted only for a tenant-scoped `paid` order; the migration quarantines legacy premature executions and refuses downgrade while any checkout label execution is still in flight.
- Before the typed checkout-identity cutover, the legacy metadata contract requires `checkout.fulfillment_key` and a non-empty `checkout.operation_id` together. PostgreSQL, SQLite, and MySQL now enforce that pair on both insert and metadata update; immutable-key enforcement remains separate.
- The shipping-option translation change journal is durable incremental-sync evidence, not a rebuildable cache: its change sequence and historical resource revisions are consumed by the Translation target cursor. Rollback refuses to drop a non-empty journal rather than silently invalidating that cursor history.
- The typed checkout-identity migration is a clean cutover: MySQL removes the pre-cutover legacy INSERT guard, PostgreSQL/SQLite/MySQL rollback paths restore the current legacy identity contract, and PostgreSQL legacy numeric-index backfill is length-bounded before BIGINT casting.
- The fulfillment migration registry contains only executable canonical migrations; an unregistered historical `m20260713_000111_enforce_order_line_allocation` prototype was removed rather than silently activating a cross-row trigger implementation that is not part of the accepted current contract.
- Support post-order follow-up fulfillments through the commerce facade, where manual create paths validate order-line ownership and remaining quantities before calling `FulfillmentService`.
- Publish a module-owned Leptos admin UI package in `admin/` for shipping-option operations.

## Translation ownership

- `fulfillment/shipping_option_copy` is the only current Fulfillment Translation target. It owns exact localized shipping-option `name` rows and their independent revision/change evidence.
- Shipping-option write paths canonicalize locales through `TenantLocale`; the storage-only `und` provenance locale is never admitted as runtime translation input, while legacy persisted `und` rows remain read-only provenance and are excluded from runtime locale resolution.
- Bulk shipping-option translation reads are ordered by owner ID and locale before runtime fallback resolution, so the existing first-available fallback and `available_locales` projection are deterministic across database executions.
- Fulfillment collection reads use explicit ID tie-breakers after timestamp ordering, keeping list pagination, latest-by-order selection, and item projection order deterministic when timestamps are equal.
- Shipping-option currency codes are normalized as uppercase ASCII three-letter codes at the owner-service boundary, matching the provider currency invariant before persistence.
- Malformed `shipping_profiles.allowed_slugs` compatibility metadata fails closed in Fulfillment projections: a present but structurally invalid shipping-profile namespace yields an empty allowed-profile set rather than silently removing the profile restriction; completely absent `shipping_profiles` retains the existing unrestricted semantics.
- Supplying typed shipping-profile restrictions requires object-shaped shipping-option metadata; non-object metadata is rejected rather than discarded when the compatibility projection is materialized.
- Shipping-option translation names are limited to 120 Unicode characters at the owner-service boundary, matching persisted schema and exact-locale mutation validation before storage.
- Fulfillment lifecycle audit append requires object-shaped metadata; malformed scalar/array metadata is rejected instead of being silently discarded when the owner records lifecycle history.
- Lifecycle metadata merge rejects both non-object persisted metadata and non-object metadata patches before merging, preventing scalar/array patches from replacing valid structured metadata before audit recording.
- Checkout fulfillment creation requires object-shaped root and item metadata; the checkout projection rejects a non-object root or malformed `checkout` namespace instead of converting it to an empty object and silently losing data, while preserving the canonical item `cart_line_item_id` projection.
- Fulfillment and fulfillment-item audit history now treats both `audit` and `audit.events` as reserved structured evidence; lifecycle append rejects malformed audit shapes instead of silently replacing them.
- Fulfillment-item metadata treats legacy checkout identity keys (`operation_id`, `order_id`, `order_plan_hash`, `fulfillment_index`, `fulfillment_key`) as owner-reserved and strips them at create; `checkout.cart_line_item_id`, when present, must be a non-nil canonical UUID string.
- Fulfillment-item lifecycle arithmetic revalidates persisted quantity/shipped/delivered counters before subtraction or increment; inconsistent legacy snapshots fail closed instead of reaching unchecked integer arithmetic.
- Fulfillment-item create inputs and checkout projections strip caller-supplied `metadata.audit` before persistence; lifecycle append is the sole owner path that creates item audit history.
- All FulfillmentService entrypoints that accept tenant identity reject the nil UUID before persistence or tenant-scoped reads; tenant identity remains an explicit invariant of the owner service boundary.
- Checkout fulfillment plan hashes are canonical lowercase 64-character hexadecimal values at the owner boundary; validation normalizes both incoming and persisted hashes so legacy typed rows with valid uppercase hex remain readable and adoptable.
- `metadata.provider_operation` is write-reserved: fulfillment creation strips caller-supplied receipt data, while provider-backed lifecycle commands attach the receipt only after the provider operation has been journaled.
- Shipping-option `provider_id` values use the canonical Fulfillment provider-registry identifier grammar at the owner boundary, so persisted options cannot contain provider IDs that the registry would later reject.
- The broad `fulfillment/fulfillment_copy` readiness row is an aggregate classification only and must not be registered as a second Translation provider.
- `carrier` and `tracking_number` are identifiers; provider IDs, shipping-profile slugs, metadata, amounts, currencies, routing and lifecycle state are operational facts rather than translatable copy.
- `delivered_note` and `cancellation_reason` belong to fulfillment history and preserve their original operational context. Translation must not retroactively rewrite those facts.
- Any future mutable Fulfillment presentation surface must be introduced as its own typed owner resource with exact locale storage, CAS/idempotency and change evidence instead of widening the aggregate row or scanning metadata.

## Interactions

- Depends on `rustok-core` for module contracts and fulfillment permission vocabulary.
- Used by `rustok-commerce` as the default fulfillment submodule of the ecommerce family.
- Links to orders and customers by identifier without taking ownership of those domains.
- `apps/admin` consumes `rustok-fulfillment-admin` through manifest-driven `build.rs` composition for shipping-option CRUD and lifecycle work.
- `rustok-commerce-storefront` consumes `rustok-fulfillment-storefront` for delivery-group shipping selection UI while it still orchestrates cross-module checkout transport and delegates shipping-selection fallback policy to the fulfillment-owned transport facade.

## Conditional capability boundary

Fulfillment owns shipping options and shipment lifecycle only for lines carrying a
typed physical-fulfillment requirement. It must not create shipment state for digital
lines or require Commerce/Product to enable Fulfillment for digital-only operation.
Historical fulfillment data remains Fulfillment-owned even when new capability use is
disabled. See
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Entry points

- `FulfillmentModule`
- `FulfillmentService`
- `providers::*`
- `admin::FulfillmentAdmin` (publishable Leptos package)
- `dto::*`
- `entities::*`

See also `docs/README.md`.
