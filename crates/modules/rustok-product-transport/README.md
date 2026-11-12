# rustok-product-transport

> This is the current Product gRPC adapter package. The [FBA conversion target](../../../docs/backend/module-backend-implementation.md#remote-fba-module-layout) places the adapters and published contracts in `rustok-product-ports` and the standalone binary in the Product owner.

Typed tonic gRPC framing for the Product-owned `ProductCatalogReadPort`,
`ProductVariantIdentityReadPort` and `ProductCatalogCommandPort`.

## Public surface

- `GrpcProductCatalogReadProvider` implements the owner port for consumers.
- `GrpcProductCatalogReadConnectionConfig` validates and opens production client connections.
- `ProductCatalogGrpcBearerToken` stores a prevalidated, debug-redacted service credential.
- `ProductCatalogGrpcBearerInterceptor` authenticates bearer and tenant metadata before installing trusted authority.
- `ProductCatalogGrpcService<P>` exposes an owner-port provider over gRPC.
- `TrustedProductCatalogAuthority` carries interceptor-authenticated tenant and principal authority.
- `ProductCatalogGrpcOperation` declares distinct read and command grants. The bearer authenticator grants only reads by default.
- Generated read and command protobuf services define RPC framing only.

## Known Limitations / Pending Implementation

The remote provider implements the three base catalog projection methods, the storefront
variant projection used by Cart, both SKU identity methods, filtered storefront lists,
facets and storefront Product detail.
Fifteen operations are available on the separate opt-in command service. Schema, storefront
HTTP and tag capabilities still need their own bindings. The package depends on the full
Product owner crate.

Upgrade the provider to a version with the SKU identity and storefront variant RPCs
before selecting it for Cart and Pricing. A provider without these methods returns a nonretryable
`product.grpc_operation_unsupported` error. See the
[Product review](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).

## Boundary

This crate owns protobuf/tonic framing, transport deadlines, gRPC status mapping, connection validation, service authentication, and trusted-authority adaptation. It does not own Product DTOs, catalog policy, persistence, locale/channel semantics, fallback decisions, or consumer composition.

Protobuf frames JSON-serialized Product-owned request/response types and `rustok_api::PortContext`. An authenticated client sends `Authorization: Bearer ...` and `x-rustok-tenant-id` metadata for each RPC. The server compares the complete authorization value in constant time, validates the tenant as a UUID, installs a configured trusted service actor, verifies the claimed tenant against that authority, and replaces untrusted actor, claims, and roles before invoking the owner port. Structured `PortError` values are carried in gRPC status details.

The bearer credential is a deployment secret. Its `Debug` representation is redacted, authentication failures never echo it, and production deployments must deliver it through secret management rather than source control or endpoint URLs. TLS and authentication solve separate problems: production connections still require HTTPS, while the token establishes caller identity.

Production connections require an absolute HTTPS service endpoint without credentials, path, query, or fragment. Plain HTTP is accepted only for an explicitly enabled loopback address. Connect timeout is limited to 1–30000 milliseconds.

## Server deployment

The RusToK server selects the Product catalog provider once at startup:

- `RUSTOK_PRODUCT_DEPLOYMENT=embedded` is the default for both reads and commands. `grpc` selects both remote providers;
- `RUSTOK_PRODUCT_CATALOG_PROVIDER` and `RUSTOK_PRODUCT_COMMAND_PROVIDER` override the deployment default independently;
- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` requires both `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT` and `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`;
- Remote commands use the same endpoint and require a distinct `RUSTOK_PRODUCT_CATALOG_COMMAND_GRPC_BEARER_TOKEN`. The [standalone service](../rustok-product-catalog-service/README.md#required-configuration) also needs a command actor UUID;
- `RUSTOK_PRODUCT_CATALOG_GRPC_TLS_DOMAIN` optionally overrides TLS SNI/domain validation;
- `RUSTOK_PRODUCT_CATALOG_GRPC_CONNECT_TIMEOUT_MS` defaults to `5000`;
- `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true` permits only loopback HTTP for local execution.

Remote connection variables are rejected when both providers are embedded. A command token
requires a remote command provider. Invalid configuration or connection failure stops startup.
Bearer authorization is checked on each RPC.

The standalone Product catalog service host in `crates/modules/rustok-product-catalog-service` must use `ProductCatalogGrpcBearerInterceptor` (or an equivalent stronger authenticator that installs `TrustedProductCatalogAuthority`) and configure its trusted service actor server-side. The caller-supplied actor in `PortContext` is never authoritative.

## Evidence

The loopback harness covers selected reads and commands, bearer authentication, typed not-found details, deadline-required semantics, and trusted actor replacement:

```bash
cargo test -p rustok-product-transport --test port_conformance
```

Loopback conformance covers transport and authority handling against a test owner. Retain mounted provider and consumer evidence separately. Historical maintainer-reported results remain bounded to their attestation.

Authentication source and mutation guards are available separately:

```bash
node scripts/verify/verify-product-catalog-grpc-authentication.mjs
node scripts/verify/verify-product-catalog-grpc-authentication.test.mjs
```

`examples/product_catalog_runtime_probe.rs` is the allowlisted authenticated client used by the separate-process runtime capture. It reads operator-provided tenant, product, and variant fixture identifiers from environment variables, invokes the six original read RPCs, checks returned projections and SKU identities, and emits one bounded success marker without printing identifiers or credentials. Outside the locked read capture, `RUSTOK_PRODUCT_CATALOG_EVIDENCE_ENABLE_COMMANDS=true` also runs `preview_variant_axes` with the distinct command credential. The preview does not verify persisted writes.

The source contract and mutation guard are checked with:

```bash
node scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs
node scripts/verify/verify-product-catalog-separate-process-runtime-contract.test.mjs
```

The operator-only capture command is:

```bash
node scripts/evidence/capture-product-catalog-separate-process-runtime.mjs
```

The capture starts the standalone provider and `rustok-server` as separate processes under an explicit local-loopback profile, runs the authenticated probe, and retains only source hashes, readiness markers, output hashes, byte counts, toolchain versions, and command metadata. Raw database URLs, bearer credentials, fixture identifiers, and process output are not persisted. The capture covers all six owner RPCs and consumer startup. Mounted Commerce and AI business requests require separate evidence.

The implementation agent does not claim the runtime capture command was executed. Product remains `boundary_ready` until retained provider/consumer runtime evidence and separate-process Commerce/AI business-request evidence exist.

## Documentation

- [CRATE_API.md](CRATE_API.md) — Public API contract, transport framing, capability bindings, and domain invariants.
