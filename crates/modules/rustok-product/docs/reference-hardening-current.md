# rustok-product canonical large-reference hardening cursor

Status: `large_reference_hardening_in_progress_after_public_api_slice`.

Baseline: `ad04a17e8252ae9d23e99af4b7ad7ffaebdfd10e` (Blog canonical reference v1 source certification, #4073).

This document is the current architecture cursor for turning `rustok-product` into the
large-owner companion reference to `rustok-blog`. It does not replace the historical
implementation plan or Product-specific Index, Translation, Category, FBA, or variant-axis
records. It records only the cross-module canonical-reference gap against the accepted Blog v1
contract.

## Reference target

Product must implement the same responsibility and safety vocabulary as the Blog reference while
retaining Product-specific scale, catalog-schema, pricing/inventory collaboration, Index replay,
Translation targets, and external read-port responsibilities.

A large module is not canonical merely because its individual capabilities are mature. Owner API,
mutation semantics, lifecycle, concurrency, tenant authority, public errors, integrations, UI
adapters, persistence visibility, and machine guards must agree as one architecture.

## Slice 1 — native owner context and public-error boundary

Source-hardening in the current branch closes the first bounded gap:

- admin catalog and schema native server functions resolve `HostRuntimeContext` fallibly;
- storefront catalog-list and detail/search-option native functions resolve host context fallibly;
- missing host runtime/event-bus composition uses the Product-owned safe public-error envelope;
- raw extractor/runtime dependency text is not rendered at public native boundaries;
- schema/admin native context fails closed when `AuthContext.tenant_id != TenantContext.id`;
- the existing Product public-error contract now exposes a generic owner-safe internal descriptor;
- existing admin/storefront/error-safety verifiers guard the fallible context and tenant-authority
  pattern instead of requiring `expect_context`;
- storefront catalog native source evidence records the new fallible-host-context invariant.

This slice does not claim compile, runtime, mounted-parity, or transport evidence. Maintainer
execution remains separate.

## Confirmed open canonical-reference gaps

The following gaps are confirmed from current source and block Product from being called the
canonical large-module reference:

1. **Public persistence API / crate layout** — **closed by this slice**
   - `src/lib.rs` no longer exports `entities` or `migrations`.
   - `ProductModule` runtime registration/composition now lives in `src/module.rs`; the crate root is a thin public facade.
   - `ProductStatus` is owned by `src/domain/product_status.rs`, independent of the persistence entity module, and is re-exported through the public DTO/root API.
   - persistence entity modules remain crate-private; public consumers receive DTOs/services/ports/contracts.

2. **Nullable mutation semantics**
   - `UpdateProductInput` uses `Option<T>` for nullable owner fields such as seller, vendor,
     product type, shipping profile, and primary category.
   - Several update branches therefore cannot represent `Keep / Set / Clear` unambiguously.
   Product must adopt explicit patch semantics before it becomes the large mutation reference.

3. **Business optimistic concurrency**
   - the Product aggregate has no edit predecessor `version`/business revision;
   - `CatalogService::update_product` loads the current row and performs an unconstrained
     ActiveModel update;
   - Product/Variant `index_revision` is an Index source watermark and must not be repurposed as
     the editor/business CAS token.
   A dedicated owner revision and predecessor-bound mutation contract are required.

4. **Owner error classification**
   - the Flex-to-Product error adapter still has a catch-all
     `other => CommerceError::Validation(other.to_string())`;
   - dependency/internal classes must be audited so infrastructure or storage failures never
     collapse into client validation or leak implementation details.

## Required audits not yet classified as defects

These areas must be audited against Blog v1 before certification; this cursor deliberately does not
prejudge their result:

- multilingual write provenance and missing-translation storage invariants;
- lifecycle transition ownership and whether transports/services share one policy;
- metadata shadow copies versus typed canonical state;
- Product/Commerce GraphQL and HTTP ownership boundaries;
- Category/Taxonomy canonical projection semantics;
- Search/SEO/Index/Translation/Pricing/Inventory integration ownership;
- migration invariants and source/upgrade compatibility;
- admin/storefront responsibility split and file-size/layout profile;
- event/idempotency semantics across Product, Variant, Index refresh, Translation and FBA;
- complete public error semantics beyond the native context slice.

## FBA status

`ProductCatalogReadPort / product.catalog_read.v1` remains `boundary_ready`.
The substantial gRPC/provider/consumer source work and maintainer attestations do not by themselves
promote Product to `transport_verified`. The registry's retained separate-process/runtime evidence
requirements remain authoritative.

## Ordered continuation

1. Merge the native owner-context/redaction slice after source review.
2. Re-read fresh `main`.
3. Make Product persistence entities private and split the crate root into a thin facade plus
   runtime/module composition without breaking legitimate owner contracts.
4. Introduce explicit `Patch<T>` semantics for nullable Product edits.
5. Add a dedicated Product business revision and mandatory predecessor CAS through owner service,
   GraphQL/native transports, and admin edit state.
6. Audit and harden lifecycle, multilingual/storage provenance, metadata ownership and events.
7. Audit integrations/FBA and finish Product-specific large-reference machine guards.
8. Only then mark Product as the canonical large-module reference and continue to Pages, Forum,
   Taxonomy, and remaining modules/libs.
