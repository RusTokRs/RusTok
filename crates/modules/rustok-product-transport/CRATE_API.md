# rustok-product-transport / CRATE_API

## Public surface

### Public modules
- `auth`: Bearer token authentication and validation errors.
- `client`: Client-side gRPC adapter `GrpcProductCatalogReadProvider`.
- `connection`: Configuration and connection lifecycle for gRPC channels.
- `server`: gRPC server adapter `ProductCatalogGrpcService` and interceptor.
- `proto`: Generated protobuf service definitions (`product_catalog_read_service_client`, `product_catalog_read_service_server`).

### Primary Public Types & Contracts
- `GrpcProductCatalogReadProvider`: Implements `ProductCatalogReadPort` over remote gRPC.
- `GrpcProductCatalogReadConnectionConfig`: Configuration validator for HTTPS/loopback endpoints, timeouts, and TLS domain overrides.
- `ProductCatalogGrpcBearerToken`: Secret-redacted bearer credential wrapper.
- `ProductCatalogGrpcBearerInterceptor`: Ingress interceptor validating bearer tokens and tenant headers.
- `ProductCatalogGrpcService`: Server adapter exposing the catalog read port.
- `TrustedProductCatalogAuthority`: Installed authority holding authenticated tenant and service principal identity.
- `ProductCatalogGrpcOperation`: Authorized operations enum (`GetProductCatalogEntry`, `GetProductVariantCatalogEntry`, `ListProductCatalogEntries`).

## Platform Capability Bindings
- **Microservice Extraction**: Supplies typed remote gRPC transport for whole-module extraction of the Product catalog.
- **Port Framing**: Encapsulates `ProductCatalogReadPort` RPC invocation and status translation.
- **Zero-Storage Adapter**: Contains no catalog persistence, caching, or business calculation logic.

## Domain Collaborations
- **`rustok-product`**: Sovereign domain module owning product catalog entities, variants, prices, and queries.
- **Consumers (`rustok-commerce`, `rustok-cart`, `rustok-order`)**: Access catalog entries remotely without direct database coupling.

## Minimum Contract Set

### Input DTOs/Commands
- `GrpcProductCatalogReadConnectionConfig`: Connection parameters (`endpoint`, `connect_timeout`, `tls_domain`, `bearer_token`, `allow_insecure_loopback`).
- Protobuf RPC payloads serializing `PortContext` and query parameters.

### Domain Invariants
- **Constant-Time Authentication**: Bearer token comparisons use constant-time equality (`subtle::ConstantTimeEq`) to prevent timing side-channels.
- **Trusted Authority Enforcement**: The server overrides caller-supplied untrusted actors with the interceptor-authenticated service authority.
- **Strict HTTPS Default**: Non-loopback endpoints require valid TLS certificates and reject unencrypted HTTP.
- **Deadline Propagation**: Context deadlines are translated into gRPC timeouts and enforced on both client and server.

### Events / Outbox Side Effects
- Read-only transport adapter; emits no domain events and performs no data mutations.

### Errors / Failure Codes
- `ProductCatalogGrpcAuthenticationError`: Missing authorization header, malformed token, or tenant ID mismatch.
- `GrpcProductCatalogReadConnectionError`: Invalid URL, insecure scheme, or connection timeout.
- Mapped `PortError` failure codes:
  - `product.not_found`
  - `product.variant_not_found`
  - `product.validation`
  - `product.unavailable`
  - `port.access_denied`
