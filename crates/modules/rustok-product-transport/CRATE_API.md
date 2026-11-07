# rustok-product-transport / CRATE_API

## Public surface

Public modules are `auth`, `client`, `connection`, `server` and generated `proto`.

- `GrpcProductCatalogReadProvider` implements the three base methods of
  `ProductCatalogReadPort` and both methods of `ProductVariantIdentityReadPort`.
- `GrpcProductCatalogReadConnectionConfig` validates endpoint, connect timeout, optional
  TLS domain and explicit insecure-loopback permission. `validated()` returns
  `ValidatedGrpcProductCatalogReadConnection`; `connect()` opens the provider channel.
- `ProductCatalogGrpcBearerToken` validates a credential and redacts Debug output.
  Client authentication is attached with `with_authentication(...)`, separately from
  connection configuration. No secure-memory or zeroization guarantee is claimed.
- `ProductCatalogGrpcBearerAuthenticator` and `ProductCatalogGrpcBearerInterceptor`
  establish server-side service authority.
- `TrustedProductCatalogAuthority` carries authenticated tenant/actor/claims/roles and
  allowed operations; client-serialized identity is not authoritative.
- `ProductCatalogGrpcOperation` declares `ReadProductProjection`,
  `ReadVariantProductProjection`, `ListPublishedProducts`, `ReadVariantIdentities`
  and `ListProductVariantIdentities` grants.
- `ProductCatalogGrpcService<P>` exposes those operations over the generated
  `ProductCatalogReadService`. `with_variant_identity_read_port` attaches the owner
  identity implementation before the service is mounted.

## Platform Capability Bindings

The adapter supplies gRPC framing, transport deadlines, service authentication and
trusted-authority adaptation for the selected Product read ports.

## Domain Collaborations

Product owns the DTOs, catalog policy and persistence. Both ports execute through the
configured Product owner. Pricing consumes SKU identity and remains the pricing authority.
The package depends on the full Product owner crate.

## Minimum Contract Set

### Input DTOs/Commands

Protobuf frames a serialized Product request DTO and a typed `PortContext`. The selected
owner returns a typed projection or a bounded list of SKU identities.

### Domain Invariants

- Production connection configuration requires HTTPS. Explicit loopback HTTP is for local use.
  Connect timeout is bounded to 1..=30000 milliseconds.
- The bearer comparison uses constant-time equality for the complete authorization value.
- The server verifies tenant and operation grants and replaces untrusted actor, claims
  and roles before invoking the owner port.
- Port deadlines become gRPC timeouts.

### Events / Outbox Side Effects

The transport executes reads and emits no domain events or outbox writes.

### Errors / Failure Codes

Owner `PortError` details are preserved. `ProductCatalogGrpcAuthenticationError` and
`GrpcProductCatalogReadConnectionError` describe local authentication or connection
configuration failures. An unsupported RPC maps to a nonretryable
`product.grpc_operation_unsupported` error.

## Coverage and evidence

This adapter covers catalog projections and SKU identity. Open schema and storefront
capabilities and local command composition are listed in the
[Product coverage inventory](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
The [README](README.md) distinguishes historical maintainer conformance from current
verification for the extended profile and separate-process business evidence.
