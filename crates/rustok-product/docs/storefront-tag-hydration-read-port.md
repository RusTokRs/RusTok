# Product Storefront tag hydration read capability

Status: `embedded_optional_source_complete_external_transport_open`.

`ProductStorefrontTagReadPort` is an optional Product-owned post-page read capability for consumers that have
already fixed a Product page and need exact Product public tag semantics.

The capability is intentionally separate from `ProductCatalogReadPort` / `product.catalog_read.v1`.
Existing Product catalog gRPC clients/servers and external adapters therefore remain source-compatible; the
current external transport contract is not widened by this slice.

`ProductCatalogReadRuntime::in_process` selects the same `CatalogService` instance as both the canonical
catalog read provider and the optional Storefront tag provider. `ProductCatalogReadRuntime::external` does not
install an embedded fallback. A consumer that requires tag hydration must treat an absent external capability
as unavailable/fail-closed until a concrete external transport is defined.

The request is bounded to 48 unique, non-nil Product IDs and uses `PortContext` for tenant/requested locale plus
an explicit fallback locale. The implementation tenant-scopes all Product identities and delegates to the
existing `CatalogService::load_product_tag_map` owner helper, preserving relation ordering, Taxonomy
requested->fallback/canonical-key resolution, and legacy normalized `metadata.tags` fallback when no
relation-backed tags exist.

This capability is post-page projection only. It does not select Products, change Product Index schema, or
make legacy metadata tags into Index identities.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
