# rustok-product-transport

Typed tonic gRPC framing for the Product-owned `ProductCatalogReadPort`.

## Public surface

- `GrpcProductCatalogReadProvider` implements the owner port for consumers.
- `ProductCatalogGrpcService<P>` exposes an owner-port provider over gRPC.
- `TrustedProductCatalogAuthority` carries interceptor-authenticated tenant and principal authority.
- `ProductCatalogGrpcOperation` declares the three authorized read operations.
- Generated `ProductCatalogReadService` protobuf types define RPC framing only.

## Boundary

This crate owns protobuf/tonic framing, transport deadlines, gRPC status mapping, and trusted-authority adaptation. It does not own Product DTOs, catalog policy, persistence, locale/channel semantics, fallback decisions, or consumer composition.

Protobuf frames JSON-serialized Product-owned request/response types and `rustok_api::PortContext`. The server verifies the claimed tenant against interceptor authority and replaces untrusted actor, claims, and roles before invoking the owner port. Structured `PortError` values are carried in gRPC status details.

## Evidence

The loopback conformance harness covers all three owner operations, typed not-found details, deadline-required semantics, and trusted actor replacement:

```bash
cargo test -p rustok-product-transport --test port_conformance
```

The implementation agent does not claim this command was executed. Until retained execution evidence exists and a production external profile is wired, Product remains `boundary_ready`.
