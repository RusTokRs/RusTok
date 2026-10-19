# Nullable Variant update contract

Status: **source implemented; compilation and runtime verification open** (2026-10-18).
The authoritative command is `rustok_product::dto::UpdateVariantInput`, consumed by
`ProductCatalogCommandPort::update_variant`. The unused foundation-owned duplicate update
DTO has been removed; no parallel legacy update contract is retained.

## Intent, not a replacement document

| Field presence | Owner intent | Persistence effect |
| --- | --- | --- |
| Omitted | `Patch::Keep` | Do not assign the loaded field |
| Explicit `null` | `Patch::Clear` | Write SQL NULL |
| Concrete value | `Patch::Set(value)` | Write that value |

This applies independently to `sku`, `barcode`, `shipping_profile_slug`, `weight`, and
`weight_unit`. Weight and unit are separate optional columns; clearing either does not implicitly
clear or set the other. Weight uses decimal values, not binary floating-point arithmetic.
GraphQL accepts weight as an exact decimal **string** and rejects invalid, overflowing or
inexact values rather than silently rounding them.

Owner JSON uses snake_case (`shipping_profile_slug`, `weight_unit`); GraphQL/Next/Leptos
variables use camelCase (`shippingProfileSlug`, `weightUnit`). Examples of GraphQL input:

```json
{}
```

keeps all five values; `{"sku": null}` clears only SKU;
`{"barcode": "0123", "weight": "0.000000001", "weightUnit": "kg"}` sets only those fields.
`Keep` is omitted during serialization: it must never turn into a JSON null. Command receipt
payloads therefore distinguish Keep, Clear and Set. Reusing a caller key for a different patch
must conflict; replay returns the original public result and does not reapply a clear.

Other Variant update fields retain their existing contracts. In particular, omitted prices/axes
keep their values; this slice does not introduce nullable collections, whole-screen atomicity,
Variant editorial CAS, or new inventory/pricing ownership rules. Create inputs are not patches
and retain their existing optional-field semantics.

## Owner and adapter rules

- SKU/barcode/Shipping slug remain bounded to 100 characters; weight unit to 16. The owner
  checks only `Set` text values before Shipping provider IO and the write transaction, preserving
  the previous limits. Keep/Clear are valid without text validation.
- Only `Set(shipping_profile_slug)` selects a profile for admission. Existing whitespace/case
  normalization is retained, including blank slug normalization to no override. Keep/Clear
  require no Shipping lookup. The Product parent profile is inherited after clearing an override;
  this command does not erase that parent selection.
- Tenant/actor/permission/module admission and owner receipt ordering remain unchanged. Validation
  stays inside the lazy receipted command; a completed replay does not consult mutable Shipping.
- GraphQL uses `MaybeUndefined` for all five inputs and maps it to owner patches. `GqlVariant`
  exposes nullable `weight`/`weightUnit` so callers can select the persisted values in responses.
- The mounted Commerce HTTP Product routes do **not** expose an independent Variant update route.
  No new REST route is invented here. The canonical owner JSON/ToSchema contract preserves
  absent/null/value for JSON consumers; a mounted REST Variant execution claim remains out of scope.
- Next callbacks and the server action use the **update command input**, not `Partial<ProductVariant>`.
  The action forwards all five nullable fields without null filtering. The existing SKU/barcode
  controls send null for blank/whitespace edits; absent Shipping/weight controls remain omitted.
  This slice does not add new weight or Shipping form widgets.
- Leptos creation continues using `VariantDraft`; updates use `VariantUpdateDraft`. The routed
  editor explicitly patches SKU/barcode, leaving Shipping/weight/unit as Keep. Inline editing
  patches only SKU, leaving barcode and other unedited nullable fields as Keep. Edited blank
  text becomes Clear; the transport must not re-normalize a patch through `Option`.
- Leptos retry intent includes the **complete draft** plus tenant, actor and Variant identity,
  not just SKU. Non-SKU edits rotate identity; an identical retry retains it. This is existing
  in-memory retry retention, not reload/crash recovery or a durable child-command journal.

## Evidence and unrun gates

Standalone source guard: `npm run verify:product:variant-patches`. It inspects DTO, owner,
GraphQL conversion, Leptos model/transport/retry and Next action wiring; it does not execute
mutations or certify authorization, database behavior, or UI runtime.

Added regression sources (not run):

- Product DTO: omitted fields, all explicit nulls, concrete values/decimal roundtrip and distinct
  serialized receipt intents (3 scenarios).
- Commerce GraphQL helper: omitted/null mapping, concrete/exact decimal mapping, invalid or
  inexact decimal rejection (3 scenarios).
- Leptos wire: edited SKU clear while untouched fields stay absent (1 scenario).
- Leptos retry: complete-draft changes affect invocation identity (1 scenario).
- Commerce catalog owner/receipts: Keep/Clear/Set persistence and original-result replay without
  Shipping IO; character bounds before provider/Variant lookup (2 scenarios).
- Existing Leptos scalar-admission sources migrated to update drafts; obsolete rejection of
  explicit SKU/barcode clears replaced with acceptance and untouched-field checks.

No test suite was run in this slice, per the standing instruction. Rust/feature compilation,
SQLite/PostgreSQL execution (including uniqueness reuse after SKU clear, concurrent edits and
rollback on a later price/axis/inventory failure), mounted authenticated GraphQL execution,
Next typechecking and actual Next/Leptos UI behavior remain open. WASM formatting and source
checks are not Rust compilation, runtime certification, or a Product readiness promotion.
