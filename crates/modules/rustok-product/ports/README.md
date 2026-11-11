# rustok-product-ports

Canonical FBA contract crate for `rustok-product`.

Publishes:
- `ProductCatalogReadPort`: Storefront, administrative, and checkout catalog projection reads.
- `ProductVariantIdentityReadPort`: Tenant-scoped variant identity lookup and enumeration.
- `ProductCatalogCommandPort`: Mutating lifecycle and catalog commands.
- Canonical DTOs, requests, and domain models.
- Optional `grpc-client` and `grpc-server` tonic adapters.
