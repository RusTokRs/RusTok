# rustok-product-transport

Typed tonic gRPC framing for the Product-owned `ProductCatalogReadPort`.

## Public surface

- `GrpcProductCatalogReadProvider` implements the owner port for consumers.
- `GrpcProductCatalogReadConnectionConfig` validates and opens production client connections.
- `ProductCatalogGrpcBearerToken` stores a prevalidated, debug-redacted service credential.
- `ProductCatalogGrpcBearerInterceptor` authenticates bearer and tenant metadata before installing trusted authority.
- `ProductCatalogGrpcService<P>` exposes an owner-port provider over gRPC.
- `TrustedProductCatalogAuthority` carries interceptor-authenticated tenant and principal authority.
- `ProductCatalogGrpcOperation` declares the three authorized read operations.
- Generated `ProductCatalogReadService` protobuf types define RPC framing only.

## Boundary

This crate owns protobuf/tonic framing, transport deadlines, gRPC status mapping, connection validation, service authentication, and trusted-authority adaptation. It does not own Product DTOs, catalog policy, persistence, locale/channel semantics, fallback decisions, or consumer composition.

Protobuf frames JSON-serialized Product-owned request/response types and `rustok_api::PortContext`. An authenticated client sends `Authorization: Bearer ...` and `x-rustok-tenant-id` metadata for each RPC. The server compares the complete authorization value in constant time, validates the tenant as a UUID, installs a configured trusted service actor, verifies the claimed tenant against that authority, and replaces untrusted actor, claims, and roles before invoking the owner port. Structured `PortError` values are carried in gRPC status details.

The bearer credential is a deployment secret. Its `Debug` representation is redacted, authentication failures never echo it, and production deployments must deliver it through secret management rather than source control or endpoint URLs. TLS and authentication solve separate problems: production connections still require HTTPS, while the token establishes caller identity.

Production connections require an absolute HTTPS service endpoint without credentials, path, query, or fragment. Plain HTTP is accepted only for an explicitly enabled loopback address. Connect timeout is limited to 1–30000 milliseconds.

## Server deployment

The RusToK server selects the Product catalog provider once at startup:

- `RUSTOK_PRODUCT_CATALOG_PROVIDER=embedded` is the default;
- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` requires both `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT` and `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`;
- `RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN` optionally overrides TLS SNI/domain validation;
- `RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS` defaults to `5000`;
- `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true` permits only loopback HTTP for local execution.

All remote variables, including the bearer token, are rejected in embedded mode. Invalid authentication configuration, remote connection failure, or invalid transport configuration stops startup; the host does not silently fall back to the embedded provider.

The future standalone Product catalog service host must use `ProductCatalogGrpcBearerInterceptor` (or an equivalent stronger authenticator that installs `TrustedProductCatalogAuthority`) and configure its trusted service actor server-side. The caller-supplied actor in `PortContext` is never authoritative.

## Evidence

The loopback conformance harness covers all three owner operations, typed not-found details, deadline-required semantics, and trusted actor replacement:

```bash
cargo test -p rustok-product-transport --test port_conformance
```

Authentication source and mutation guards are available separately:

```bash
node scripts/verify/verify-product-catalog-grpc-authentication.mjs
node scripts/verify/verify-product-catalog-grpc-authentication.test.mjs
```

The implementation agent does not claim this command was executed, and does not claim the authentication verifier commands were executed. Until retained execution evidence exists, Product remains `boundary_ready`.
