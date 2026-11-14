# rustok-product-ports

Canonical FBA contract crate for `rustok-product`.

Publishes:
- `ProductCatalogReadPort`: Storefront, administrative, and checkout catalog projection reads.
- `ProductVariantIdentityReadPort`: Tenant-scoped variant identity lookup and enumeration.
- `ProductCatalogCommandPort`: Mutating lifecycle and catalog commands.
- Canonical DTOs, requests, and domain models.
- Optional `grpc-client` and `grpc-server` tonic adapters.

## Deployment Configuration

- `RUSTOK_PRODUCT_DEPLOYMENT=embedded` (default): Product runs within the modular monolith.
- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc`: Product catalog reads are delegated to a remote gRPC service.
- `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT`: gRPC endpoint URL (e.g. `https://product.internal:50051`).
- `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`: Shared secret bearer token for authentication.
- `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true`: Allow plaintext HTTP only on loopback interfaces (`127.0.0.1`, `::1`).

Invalid configuration or connection failure stops startup immediately.

## Authentication

- Client authentication: `ProductCatalogGrpcBearerToken` attaches `Authorization: Bearer ...` and `x-rustok-tenant-id`.
- Server authentication: Servers must use `ProductCatalogGrpcBearerInterceptor`, which compares the complete authorization value in constant time.
- Configured via `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`.
- Note: TLS and authentication solve separate problems; TLS protects data in transit while bearer tokens authenticate callers.


