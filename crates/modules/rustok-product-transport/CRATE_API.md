# rustok-product-transport / CRATE_API

## Public surface

Public modules are `auth`, `client`, `connection`, `server` and generated `proto`.

- `GrpcProductCatalogReadProvider` implements the three declared remote reads of
  `ProductCatalogReadPort`. Other optional trait methods are not implemented here.
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
- `ProductCatalogGrpcOperation` has `ReadProductProjection`,
  `ReadVariantProductProjection` and `ListPublishedProducts` variants.
- `ProductCatalogGrpcService<P>` exposes the canonical provider over the generated
  `ProductCatalogReadService` with those three RPC names.

## Ownership and framing

Product owns its request/response DTOs, catalog policy and persistence. Pricing remains
the pricing authority; Product may return owner-composed price projections. This adapter
owns protobuf framing, JSON serialization of owner DTOs/context, service authentication,
deadline propagation and gRPC status/detail mapping. It owns no Product storage or
alternate business implementation and emits no domain events.

The existing package depends on `rustok-product` with its implementation dependencies.
A contracts-only owner build is not currently published. This packaging fact does not
mean the remote client executes local Product services.

## Invariants and errors

- Production connection configuration requires HTTPS; explicitly permitted loopback HTTP
  is limited to local use. Connect timeout is bounded to 1..=30000 milliseconds.
- The bearer comparison uses constant-time equality for the complete authorization value.
- The server verifies trusted tenant and operation authority and replaces untrusted
  actor/claims/roles before invoking the owner port.
- Port deadlines become gRPC timeouts; structured `PortError` details are preserved.
- `ProductCatalogGrpcAuthenticationError` and `GrpcProductCatalogReadConnectionError`
  describe local configuration/authentication failures. Owner domain codes are mapped
  through the canonical port envelope, not redefined as a separate transport vocabulary.

## Coverage and evidence

This is a three-read capability adapter, not whole-module Product extraction. Missing
identity/schema/storefront capabilities and local command composition are listed in the
[Product coverage inventory](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
The [README](README.md) distinguishes historical maintainer conformance from current
fixture repair, compilation and separate-process business evidence still required.
