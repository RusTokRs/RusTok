# rustok-product-transport

Typed tonic gRPC framing for the Product-owned `ProductCatalogReadPort`.

## Public surface

- `GrpcProductCatalogReadProvider` implements the owner port for consumers.
- `GrpcProductCatalogReadConnectionConfig` validates and opens production client connections.
- `ProductCatalogGrpcService<P>` exposes an owner-port provider over gRPC.
- `TrustedProductCatalogAuthority` carries interceptor-authenticated tenant and principal authority.
- `ProductCatalogGrpcOperation` declares the three authorized read operations.
- Generated `ProductCatalogReadService` protobuf types define RPC framing only.

## Boundary

This crate owns protobuf/tonic framing, transport deadlines, gRPC status mapping, connection validation, and trusted-authority adaptation. It does not own Product DTOs, catalog policy, persistence, locale/channel semantics, fallback decisions, or consumer composition.

Protobuf frames JSON-serialized Product-owned request/response types and `rustok_api::PortContext`. The server verifies the claimed tenant against interceptor authority and replaces untrusted actor, claims, and roles before invoking the owner port. Structured `PortError` values are carried in gRPC status details.

Production connections require an absolute HTTPS service endpoint without credentials, path, query, or fragment. Plain HTTP is accepted only for an explicitly enabled loopback address. Connect timeout is limited to 1–30000 milliseconds.

## Server deployment

The RusToK server selects the Product catalog provider once at startup:

- `RUSTOK_PRODUCT_CATALOG_PROVIDER=embedded` is the default;
- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` requires `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT`;
- `RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN` optionally overrides TLS SNI/domain validation;
- `RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS` defaults to `5000`;
- `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true` permits only loopback HTTP for local execution.

Remote variables are rejected in embedded mode. Invalid remote configuration or connection failure stops startup; the host does not silently fall back to the embedded provider.

## Evidence

The loopback conformance harness covers all three owner operations, typed not-found details, deadline-required semantics, and trusted actor replacement:

```bash
cargo test -p rustok-product-transport --test port_conformance
```

The implementation agent does not claim this command was executed. Until retained execution evidence exists, Product remains `boundary_ready`.
