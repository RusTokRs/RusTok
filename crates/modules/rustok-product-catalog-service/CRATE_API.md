# rustok-product-catalog-service / CRATE_API

## Executable interface

This is a binary package with no public Rust library API. Its supported operator
interface is the [environment/startup contract](README.md#required-configuration),
authenticated gRPC endpoint, telemetry and shutdown behavior.

The private implementation types `ServiceConfig`, `ServiceTransport` and
`RedactedSecret` are not consumer APIs. `RedactedSecret(String)` redacts Debug output;
it does not establish secure-memory storage or zeroization.

## Platform Capability Bindings

The host composes PostgreSQL, Product's `CatalogService`, an Outbox-backed
`TransactionalEventBus` and `ProductCatalogGrpcService` with bearer authentication.
The host exposes the selected catalog projection and SKU identity read ports.
Platform telemetry uses service name `rustok-product-catalog-service`. Ctrl-C and
Unix SIGTERM request graceful tonic and telemetry shutdown.

## Domain Collaborations

Product owns catalog rules and queries. Both ports delegate to the same `CatalogService`.
The transport package owns wire framing and trusted authority adaptation. Commands and
schema administration have separate placement.

## Minimum Contract Set

### Input DTOs/Commands

The generated service accepts Product-owned read DTOs and a typed `PortContext`.
The operations are `ReadProductProjection`, `ReadVariantProductProjection`,
`ListPublishedProducts`, `ReadVariantIdentities`, `ListProductVariantIdentities`
and `ReadStorefrontVariantProductProjection`.

### Domain Invariants

- Required database, credential and trusted-service-actor configuration is documented
  in the README. Credentials and raw database URLs must not appear in diagnostics.
- The private preflight performs entity reads of `products`, `product_variants` and
  `sys_events` before the listener. It is a bounded prerequisite check, not complete
  schema/dependency/business-path verification.
- The host does not run migrations. Deployment must provide a migrated owner schema.
- TLS is required unless explicitly permitted plaintext is bound to loopback.
- Caller claims are admitted through the interceptor; the trusted actor is configured
  server-side. Initialization failure aborts startup.

### Events / Outbox Side Effects

Outbox supplies the production event-bus constructor. The exposed operations are read-only
and perform no domain or outbox writes.

### Errors / Failure Codes

Configuration, database preflight and listener failures abort startup. Authenticated
calls return owner `PortError` values through structured gRPC status details.

## Deployment limits and evidence

The catalog selector in the consumer does not select remote Product commands. Native
Product/Pricing/Inventory atomic participants remain a separate local contract. Sharing
owner storage and independent storage/replication are different deployment choices.

Standalone provider startup and mounted Commerce/AI business evidence remain
open as described by the [Product plan](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
Historical loopback attestation does not certify the current revision or a standalone
production deployment.
